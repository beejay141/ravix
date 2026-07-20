use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::{self, error::TrySendError, Receiver, Sender};

// ── Split design ───────────────────────────────────────────────────────────
// LogWriter  = cheaply cloneable channel sender (hot path, lock-free)
// LogWriterHandle = sender + join handle (kept in shutdown registry)
// ───────────────────────────────────────────────────────────────────────────

/// Cheaply-cloneable handle used on the hot path to enqueue log lines.
///
/// Internally wraps an [`UnboundedSender`] (which is just an `Arc`), so
/// `Clone` is essentially free.
#[derive(Clone)]
pub struct LogWriter {
    sender: Sender<String>,
    dropped: Arc<AtomicUsize>,
}

impl LogWriter {
    /// Enqueue a formatted line for writing (non-blocking).
    pub fn write_line(&self, line: String) {
        match self.sender.try_send(line) {
            Ok(_) => {}
            Err(TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Closed(_)) => {
                // Writer task has shut down; drop silently.
            }
        }
    }
}

/// Owned handle returned by [`LogWriterHandle::new`].
///
/// Dropping this handle (or calling [`shutdown`](Self::shutdown)) drops the
/// last sender, which signals the background writer task to drain and exit.
pub struct LogWriterHandle {
    sender: Sender<String>,
    join_handle: tokio::task::JoinHandle<()>,
    dropped: Arc<AtomicUsize>,
}

impl LogWriterHandle {
    /// Open (or create) the log file in append mode and spawn the writer task.
    pub async fn new(
        path: impl AsRef<Path>,
        capacity: Option<usize>,
    ) -> Result<Self, std::io::Error> {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        // Bounded channel to protect against unbounded memory growth under
        // sustained backpressure. Try-send is used on the hot path and drops
        // entries when the buffer is full. Use configured capacity when
        // supplied, otherwise fall back to the default.
        const DEFAULT_CHANNEL_CAPACITY: usize = 8192;
        let cap = capacity.unwrap_or(DEFAULT_CHANNEL_CAPACITY);
        let (sender, receiver) = mpsc::channel(cap);
        let dropped = Arc::new(AtomicUsize::new(0));
        let join_handle = tokio::spawn(writer_loop(file, receiver));

        Ok(Self {
            sender,
            join_handle,
            dropped,
        })
    }

    /// Return a cheaply-cloneable [`LogWriter`] for the hot path.
    pub fn writer(&self) -> LogWriter {
        LogWriter {
            sender: self.sender.clone(),
            dropped: Arc::clone(&self.dropped),
        }
    }

    /// Drop the sender and wait for all queued entries to flush.
    pub async fn shutdown(self) {
        drop(self.sender);
        let _ = self.join_handle.await;
    }
}

/// Maximum number of lines to buffer before flushing.
/// Under concurrent load this limits how many writes get batched.
const BATCH_SIZE: usize = 64;

async fn writer_loop(mut file: fs::File, mut receiver: Receiver<String>) {
    // Pre-allocated buffer to join line + newline into a single write.
    let mut buf = String::with_capacity(4096);
    let mut count: usize = 0;

    while let Some(line) = receiver.recv().await {
        // Build a batch: start with the first line then drain any
        // immediately-available lines via `try_recv` so we can issue a
        // single write syscall for the burst.
        buf.clear();
        buf.push_str(&line);
        buf.push('\n');

        let mut batch = 1usize;
        while let Ok(next) = receiver.try_recv() {
            buf.push_str(&next);
            buf.push('\n');
            batch += 1;
        }

        if let Err(e) = file.write_all(buf.as_bytes()).await {
            log::warn!("rusta-logger writer: failed to write entry: {}", e);
            continue;
        }

        count += batch;

        // Batch flush: only fsync every N lines to reduce syscall pressure.
        if count >= BATCH_SIZE {
            if let Err(e) = file.flush().await {
                log::warn!("rusta-logger writer: failed to flush: {}", e);
            }
            count = 0;
        }
    }

    // Final flush on channel close — drain any remaining lines.
    let _ = file.flush().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn log_writer_handle_new_creates_file() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let handle = LogWriterHandle::new(temp.path(), None).await.unwrap();

        drop(handle);
    }

    #[tokio::test]
    async fn log_writer_handle_writer_is_clone() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let handle = LogWriterHandle::new(temp.path(), None).await.unwrap();
        let writer = handle.writer();
        let _clone = writer.clone();

        drop(handle);
    }

    #[tokio::test]
    async fn log_writer_handle_custom_capacity() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let handle = LogWriterHandle::new(temp.path(), Some(100)).await.unwrap();

        drop(handle);
    }
}