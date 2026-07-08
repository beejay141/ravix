use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;

use crate::adapter::LogAdapter;
use crate::config::LoggerConfig;
use crate::context::CURRENT_LOG_CONTEXT;
use crate::types::{LogEntry, LogLevel, LogOptions, ServiceContext};
use crate::writer::{LogWriter, LogWriterHandle};

// ── Static state ───────────────────────────────────────────────────────────
//
// Design goals:
//   1. Zero lock contention on the hot path (every log call).
//   2. Once-only initialisation enforced at compile / init time.
//   3. Graceful shutdown with full drain of in-flight entries.
//
// ── Layout ─────────────────────────────────────────────────────────────────
//
//  LOGGER_INNER      – OnceLock – immutable after init, read lock-free.
//                      Holds the cheaply-cloneable LogWriter senders.
//  SHUTDOWN_HANDLES  – Mutex    – only touched during configure() and
//                      shutdown().  Never accessed on the hot path.
//  CORRELATION_ID_HEADER – OnceLock – static header string.
// ───────────────────────────────────────────────────────────────────────────

/// Immutable runtime state accessed lock-free on every log call.
///
/// All fields use `Arc` so clones on the hot path are ref-count bumps.
struct LoggerInner {
    service: Arc<ServiceContext>,
    min_level: LogLevel,
    default_classification: Arc<str>,
    writers: HashMap<Arc<str>, LogWriter>,
    adapter: Box<dyn LogAdapter + Send + Sync>,
}

static LOGGER_INNER: std::sync::OnceLock<LoggerInner> = std::sync::OnceLock::new();
static SHUTDOWN_HANDLES: Mutex<Option<Vec<LogWriterHandle>>> = Mutex::new(None);
static CORRELATION_ID_HEADER: std::sync::OnceLock<String> = std::sync::OnceLock::new();

// ── Helpers ────────────────────────────────────────────────────────────────

/// Lock-free read of the inner config (panics if not yet configured).
fn logger_inner() -> &'static LoggerInner {
    LOGGER_INNER
        .get()
        .expect("ravix-logger: Logger::configure must be called before any log call")
}

/// Dispatch a formatted log entry to the correct writer (lock-free).
pub(crate) fn dispatch(entry: &LogEntry) {
    let inner = logger_inner();
    match inner.writers.get(&entry.classification) {
        Some(writer) => {
            let line = inner.adapter.format(entry);
            writer.write_line(line);
        }
        None => {
            log::warn!(
                "ravix-logger: no writer configured for classification '{}' — dropping entry",
                entry.classification
            );
        }
    }
}

// ── Public API ─────────────────────────────────────────────────────────────

/// The application logger.
///
/// All instances are cheap, zero-sized handles to the same global state
/// (configured once via [`Logger::configure`]).  Create an instance with
/// [`Logger::new`] and share it via `Arc<Logger>` in your DI container.
///
/// ```ignore
/// use ravix_logger::{Logger, config};
///
/// // At startup:
/// Logger::configure(config()...build()).await;
///
/// // For DI:
/// let logger = Logger::new();       // returns Arc<Logger>
/// container.register(logger);       // register as Arc<Logger>
/// ```
#[derive(Clone, Debug)]
pub struct Logger;

impl Logger {
    /// Create a new logger handle.
    ///
    /// The handle is a zero-sized token; all instances delegate to the same
    /// global state.  Returns `Arc<Self>` for direct registration in a DI
    /// container.
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    /// Initialise the logger subsystem.
    ///
    /// Opens all classification files and spawns background writer tasks.
    /// Panics if called more than once or if any file cannot be opened.
    pub async fn configure(config: LoggerConfig) {
        let mut writers: HashMap<Arc<str>, LogWriter> = HashMap::new();
        let mut handles: Vec<LogWriterHandle> = Vec::with_capacity(config.classifications.len());

        for c in &config.classifications {
            let handle = LogWriterHandle::new(&c.log_path)
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "ravix-logger: failed to open log file '{}' for classification '{}': {}",
                        c.log_path.display(),
                        c.name,
                        e
                    )
                });
            writers.insert(Arc::from(c.name.as_str()), handle.writer());
            handles.push(handle);
        }

        let inner = LoggerInner {
            service: Arc::new(config.service),
            min_level: config.min_level,
            default_classification: Arc::from(config.default_classification.as_str()),
            writers,
            adapter: config.adapter,
        };

        CORRELATION_ID_HEADER
            .set(config.correlation_id_header)
            .ok()
            .expect("ravix-logger: CORRELATION_ID_HEADER already set");

        LOGGER_INNER
            .set(inner)
            .ok()
            .expect("ravix-logger: LOGGER_INNER already set");

        let mut guard = SHUTDOWN_HANDLES.lock().unwrap();
        assert!(
            guard.is_none(),
            "ravix-logger: SHUTDOWN_HANDLES already set"
        );
        *guard = Some(handles);
    }

    // ── Logging methods ───────────────────────────────────────────────────

    /// Emit a log entry at the given level (lock-free, allocation-minimal).
    pub fn log(&self, level: LogLevel, message: impl Into<String>, options: Option<LogOptions>) {
        let inner = logger_inner();

        if level < inner.min_level {
            return;
        }

        let opts = options.unwrap_or_default();

        // Classification: use caller override or default — Arc<str> clone is cheap.
        let classification: Arc<str> = match opts.classification {
            Some(s) => Arc::from(s.as_str()),
            None => Arc::clone(&inner.default_classification),
        };

        // Correlation ID: Arc<str> clone — ref-count bump, no alloc.
        let correlation_id: Option<Arc<str>> = CURRENT_LOG_CONTEXT
            .try_with(|ctx| Arc::clone(&ctx.correlation_id))
            .ok();

        let context = opts.context.unwrap_or_default();

        let entry = LogEntry {
            timestamp: Utc::now(),
            level,
            message: message.into(),
            classification,
            correlation_id,
            service: Arc::clone(&inner.service),
            context,
        };

        dispatch(&entry);
    }

    pub fn trace(&self, message: impl Into<String>, options: Option<LogOptions>) {
        self.log(LogLevel::Trace, message, options);
    }

    pub fn debug(&self, message: impl Into<String>, options: Option<LogOptions>) {
        self.log(LogLevel::Debug, message, options);
    }

    pub fn info(&self, message: impl Into<String>, options: Option<LogOptions>) {
        self.log(LogLevel::Info, message, options);
    }

    pub fn warn(&self, message: impl Into<String>, options: Option<LogOptions>) {
        self.log(LogLevel::Warn, message, options);
    }

    pub fn error(&self, message: impl Into<String>, options: Option<LogOptions>) {
        self.log(LogLevel::Error, message, options);
    }

    /// Returns the configured correlation ID header name.
    pub fn correlation_id_header() -> &'static str {
        CORRELATION_ID_HEADER
            .get()
            .map(|s| s.as_str())
            .unwrap_or("X-Correlation-ID")
    }

    /// Gracefully shut down all writers, draining pending entries.
    pub async fn shutdown() {
        let handles = {
            let mut guard = SHUTDOWN_HANDLES.lock().unwrap();
            guard.take()
        };
        if let Some(handles) = handles {
            for handle in handles {
                handle.shutdown().await;
            }
        }
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self
    }
}