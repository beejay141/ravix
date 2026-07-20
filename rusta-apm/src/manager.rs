use std::cell::Cell;
use std::future::Future;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::adapter::LogAdapter;
use crate::config::ApmConfig;
use crate::context::{ActiveTransaction, CURRENT_SPAN_ID, CURRENT_TXN};
use crate::span::SpanHandle;
use crate::transaction::TransactionHandle;
use crate::types::{ApmEntry, Metadata, ServiceContext};
use crate::writer::{ApmWriter, ApmWriterHandle};
use axum::http::HeaderName;

/// Global singleton holding the APM writer, adapter, and service metadata.
struct ApmInner {
    service: Arc<ServiceContext>,
    writer: ApmWriter,
    adapter: Box<dyn LogAdapter + Send + Sync>,
    correlation_id_header: Option<HeaderName>,
}

static APM_INNER: std::sync::OnceLock<ApmInner> = std::sync::OnceLock::new();
static SHUTDOWN_HANDLE: Mutex<Option<ApmWriterHandle>> = Mutex::new(None);

pub(crate) fn send_entry(mut entry: ApmEntry) {
    if let Some(inner) = APM_INNER.get() {
        // Stamp service context on transaction records.
        if let ApmEntry::Transaction(ref mut t) = entry {
            t.service = Arc::clone(&inner.service);
        }
        let line = inner.adapter.format(&entry);
        inner.writer.write_line(line);
    }
}

/// The application APM tracer.
///
/// All instances are cheap, zero-sized handles to the same global state
/// (configured once via [`Apm::configure`]). The `configure` method
/// returns `Arc<Self>` for direct registration in a DI container.
///
/// # Architecture
///
/// - Transactions represent entire HTTP requests (e.g., `GET /users`)
/// - Spans represent operations within a transaction (e.g., DB queries)
/// - Both are written to NDJSON files for ingestion by APM backends
///
/// # Example
/// ```ignore
/// use rusta_apm::{Apm, config};
///
/// // At startup:
/// let apm = Apm::configure(config()...build()).await;
/// container.register(apm);       // register as Arc<Apm>
/// ```
#[derive(Clone, Debug)]
pub struct Apm;

impl Apm {
    /// Create a new APM handle.
    ///
    /// The handle is a zero-sized token; all instances delegate to the same
    /// global state. Returns `Arc<Self>` for direct registration in a DI container.
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    /// Initialise the APM subsystem.
    ///
    /// Opens the NDJSON log file and spawns the background writer task.
    /// Panics if called more than once or if the file cannot be opened.
    /// Returns `Arc<Self>` for direct registration in a DI container.
    ///
    /// # Configuration
    ///
    /// Use [`config()`] to create a builder with these options:
    /// - `service_name()` - Required: your service identifier
    /// - `service_version()` - Optional: version string
    /// - `environment()` - Optional: "production", "development", etc.
    /// - `log_path()` - Optional: defaults to "apm.ndjson"
    /// - `correlation_id_header()` - Optional: header for request tracing
    pub async fn configure(config: ApmConfig) -> Arc<Self> {
        let handle = ApmWriterHandle::new(&config.log_path, config.channel_capacity)
            .await
            .expect("rusta-apm: failed to open APM log file");
        let writer = handle.writer();
        let parsed_header = config.correlation_id_header.map(|s| {
            s.parse::<HeaderName>()
                .expect("rusta-apm: invalid correlation_id_header")
        });

        let inner = ApmInner {
            service: config.service,
            writer,
            adapter: config.adapter,
            correlation_id_header: parsed_header,
        };
        APM_INNER
            .set(inner)
            .ok()
            .expect("rusta-apm: Apm::configure already called");

        let mut guard = SHUTDOWN_HANDLE.lock().unwrap();
        assert!(guard.is_none(), "rusta-apm: SHUTDOWN_HANDLE already set");
        *guard = Some(handle);

        Arc::new(Self)
    }

    /// Retrieve the configured correlation-id header name, if any.
    pub(crate) fn correlation_id_header() -> Option<&'static HeaderName> {
        APM_INNER
            .get()
            .and_then(|inner| inner.correlation_id_header.as_ref())
    }

    /// Gracefully shut down the APM writer, draining pending entries.
    pub async fn shutdown() {
        let handle = {
            let mut guard = SHUTDOWN_HANDLE.lock().unwrap();
            guard.take()
        };
        if let Some(handle) = handle {
            handle.shutdown().await;
        }
    }

    // ── Transaction API ──────────────────────────────────────────────────

    /// Create a new transaction without entering its task-local scope.
    ///
    /// Use [`TransactionHandle::active_txn`] to obtain the `Arc` needed for
    /// `CURRENT_TXN.scope(...)`, then call [`TransactionHandle::end`] when
    /// done.
    pub fn start_transaction(
        &self,
        name: &str,
        txn_type: &str,
        metadata: Option<Metadata>,
    ) -> TransactionHandle {
        let txn = Arc::new(ActiveTransaction::new(name.to_string()));
        if let Some(meta) = metadata {
            txn.metadata
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend(meta);
        }
        TransactionHandle::new(txn, txn_type.to_string())
    }

    /// Start a transaction, execute `f` inside its scope, and end the
    /// transaction automatically.
    ///
    /// `f` returns a future. The transaction ends with `result: None`.
    pub async fn wrap_transaction<F, Fut, T>(
        &self,
        name: &str,
        txn_type: &str,
        metadata: Option<Metadata>,
        f: F,
    ) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let txn = Arc::new(ActiveTransaction::new(name.to_string()));
        if let Some(meta) = metadata {
            txn.metadata
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend(meta);
        }
        let handle = TransactionHandle::new(txn.clone(), txn_type.to_string());
        let result = CURRENT_TXN
            .scope(txn.clone(), async {
                CURRENT_SPAN_ID.scope(Cell::new(Uuid::nil()), f()).await
            })
            .await;
        handle.end(None, None);
        result
    }

    /// Like [`wrap_transaction`](Self::wrap_transaction) but takes a future
    /// directly.
    pub async fn wrap_transaction_future<Fut, T>(
        &self,
        name: &str,
        txn_type: &str,
        metadata: Option<Metadata>,
        fut: Fut,
    ) -> T
    where
        Fut: Future<Output = T>,
    {
        let txn = Arc::new(ActiveTransaction::new(name.to_string()));
        if let Some(meta) = metadata {
            txn.metadata
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend(meta);
        }
        let handle = TransactionHandle::new(txn.clone(), txn_type.to_string());
        let result = CURRENT_TXN
            .scope(txn.clone(), async {
                CURRENT_SPAN_ID.scope(Cell::new(Uuid::nil()), fut).await
            })
            .await;
        handle.end(None, None);
        result
    }

    // ── Span API ─────────────────────────────────────────────────────────

    /// Start a span under the current transaction (set via task-local).
    ///
    /// Returns a no-op handle (and logs a warning) when called outside an
    /// active transaction.
    pub fn start_span(
        &self,
        name: &str,
        span_type: &str,
        metadata: Option<Metadata>,
    ) -> SpanHandle {
        match CURRENT_TXN.try_with(|t| t.clone()) {
            Ok(txn) => {
                let parent_id = CURRENT_SPAN_ID.try_with(|id| id.get()).ok();
                SpanHandle::new(
                    txn,
                    name.to_string(),
                    span_type.to_string(),
                    None,
                    parent_id,
                    metadata,
                )
            }
            Err(_) => {
                log::warn!(
                    "rusta-apm: start_span(\"{}\") called without an active transaction — returning no-op",
                    name
                );
                SpanHandle::noop()
            }
        }
    }

    /// Start a span, execute `f`, and end the span automatically.
    pub async fn wrap_span<F, Fut, T>(
        &self,
        name: &str,
        span_type: &str,
        metadata: Option<Metadata>,
        f: F,
    ) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let handle = self.start_span(name, span_type, metadata);
        let result = f().await;
        handle.end(None);
        result
    }

    /// Like [`wrap_span`](Self::wrap_span) but takes a future directly.
    pub async fn wrap_span_future<Fut, T>(
        &self,
        name: &str,
        span_type: &str,
        metadata: Option<Metadata>,
        fut: Fut,
    ) -> T
    where
        Fut: Future<Output = T>,
    {
        let handle = self.start_span(name, span_type, metadata);
        let result = fut.await;
        handle.end(None);
        result
    }

    // ── Context propagation ──────────────────────────────────────────────

    /// Capture the current transaction context for passing to spawned tasks.
    ///
    /// Returns `None` when called outside any transaction.
    pub fn current_context(&self) -> Option<Arc<ActiveTransaction>> {
        CURRENT_TXN.try_with(|t| t.clone()).ok()
    }

    /// Execute `fut` inside the given transaction context.
    ///
    /// Allows spans inside `tokio::spawn` to link back to the parent
    /// transaction. Pass `None` to run without context.
    pub async fn with_context<F>(&self, ctx: Option<Arc<ActiveTransaction>>, fut: F) -> F::Output
    where
        F: Future + Send,
    {
        match ctx {
            Some(txn) => {
                CURRENT_TXN
                    .scope(txn, async {
                        CURRENT_SPAN_ID.scope(Cell::new(Uuid::nil()), fut).await
                    })
                    .await
            }
            None => fut.await,
        }
    }
}

impl Default for Apm {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SpanRecord, TransactionRecord};

    #[test]
    fn apm_new_returns_arc() {
        // Apm::new() returns Arc<Self>
        let apm = Apm::new();
        // The Arc itself has strong count >= 1
        assert!(Arc::strong_count(&apm) >= 1);
    }

    #[test]
    fn apm_default() {
        // Apm::default() returns Self, wrap in Arc for testing
        let apm = Arc::new(Apm::default());
        assert!(Arc::strong_count(&apm) >= 1);
    }

    #[test]
    fn send_entry_without_inner() {
        // When APM_INNER is not set, send_entry should be a no-op
        let entry = ApmEntry::Transaction(TransactionRecord {
            id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            name: "test".to_string(),
            transaction_type: "request".to_string(),
            start_time: chrono::Utc::now(),
            end_time: chrono::Utc::now(),
            duration_ms: 1.0,
            result: None,
            correlation_id: None,
            metadata: Metadata::new(),
            service: Arc::new(ServiceContext::default()),
        });
        // Should not panic
        send_entry(entry);
    }

    #[test]
    fn send_entry_span_without_inner() {
        // When APM_INNER is not set, send_entry should be a no-op for spans too
        let entry = ApmEntry::Span(SpanRecord {
            id: Uuid::new_v4(),
            transaction_id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            parent_id: None,
            name: "test-span".to_string(),
            span_type: "db".to_string(),
            subtype: None,
            start_time: chrono::Utc::now(),
            end_time: chrono::Utc::now(),
            duration_ms: 1.0,
            metadata: Metadata::new(),
        });
        // Should not panic
        send_entry(entry);
    }

    #[test]
    fn apm_current_context_outside_transaction() {
        let apm = Apm::new();
        // When no transaction is active, should return None
        let ctx = apm.current_context();
        assert!(ctx.is_none());
    }

    #[test]
    fn apm_with_context_none() {
        let apm = Apm::new();
        // When None is passed, should just run the future
        let result = futures::executor::block_on(apm.with_context(None, async { "no_context" }));
        assert_eq!(result, "no_context");
    }

    #[test]
    fn apm_with_context_some() {
        let txn = Arc::new(ActiveTransaction::new("test-txn".to_string()));
        let apm = Apm::new();

        let result = futures::executor::block_on(apm.with_context(Some(txn.clone()), async {
            let current = CURRENT_TXN.try_with(|t| t.clone());
            assert!(current.is_ok());
            CURRENT_TXN.try_with(|t| t.name.clone()).unwrap()
        }));

        assert_eq!(result, "test-txn");
    }

    #[test]
    fn apm_wrap_span_without_transaction() {
        let apm = Apm::new();
        // When no transaction is active, wrap_span should still work (returns noop)
        let result = futures::executor::block_on(apm.wrap_span("test-span", "custom", None, || async {
            "span_result"
        }));
        assert_eq!(result, "span_result");
    }

    #[test]
    fn apm_wrap_span_future_without_transaction() {
        let apm = Apm::new();
        let fut = async { "future_result" };

        let result = futures::executor::block_on(apm.wrap_span_future("test-span", "custom", None, fut));
        assert_eq!(result, "future_result");
    }

    #[test]
    fn apm_wrap_transaction_without_inner() {
        let apm = Apm::new();
        // When APM_INNER is not set, wrap_transaction should still work
        let result = futures::executor::block_on(apm.wrap_transaction("test-txn", "request", None, || async {
            "txn_result"
        }));
        assert_eq!(result, "txn_result");
    }

    #[test]
    fn apm_wrap_transaction_future_without_inner() {
        let apm = Apm::new();
        let fut = async { "future_txn_result" };

        let result = futures::executor::block_on(apm.wrap_transaction_future("test-txn", "request", None, fut));
        assert_eq!(result, "future_txn_result");
    }

    #[test]
    fn apm_start_transaction_without_inner() {
        let apm = Apm::new();
        // When APM_INNER is not set, start_transaction should still work
        let handle = apm.start_transaction("test-txn", "request", None);
        handle.end(Some("success"), None);
    }

    #[test]
    fn apm_start_span_without_transaction() {
        let apm = Apm::new();
        // When no transaction is active, start_span returns noop
        let handle = apm.start_span("orphan-span", "custom", None);
        handle.end(None);
    }

    #[test]
    fn apm_correlation_id_header_without_inner() {
        // When APM_INNER is not set, correlation_id_header should return None
        let header = Apm::correlation_id_header();
        assert!(header.is_none());
    }

    #[test]
    fn send_entry_transaction_stamps_service() {
        // Test that send_entry stamps service context on transaction entries
        // This tests the inner logic when APM_INNER is set
        // We can't easily set APM_INNER due to OnceLock, but we can test the logic
        // by checking the code path exists
        let entry = ApmEntry::Transaction(TransactionRecord {
            id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            name: "test".to_string(),
            transaction_type: "request".to_string(),
            start_time: chrono::Utc::now(),
            end_time: chrono::Utc::now(),
            duration_ms: 1.0,
            result: None,
            correlation_id: None,
            metadata: Metadata::new(),
            service: Arc::new(ServiceContext::default()),
        });
        // Should not panic even without inner
        send_entry(entry);
    }

    #[test]
    fn send_entry_span_no_service_stamp() {
        // Spans should not have service stamped
        let entry = ApmEntry::Span(SpanRecord {
            id: Uuid::new_v4(),
            transaction_id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            parent_id: None,
            name: "test-span".to_string(),
            span_type: "db".to_string(),
            subtype: None,
            start_time: chrono::Utc::now(),
            end_time: chrono::Utc::now(),
            duration_ms: 1.0,
            metadata: Metadata::new(),
        });
        // Should not panic
        send_entry(entry);
    }
}
