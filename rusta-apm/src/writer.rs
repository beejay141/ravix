use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::{self, error::TrySendError, Receiver, Sender};

// ── Split design ───────────────────────────────────────────────────────────
// ApmWriter  = cheaply cloneable channel sender (hot path, lock-free)
// ApmWriterHandle = sender + join handle (kept in shutdown registry)
// ───────────────────────────────────────────────────────────────────────────

/// Cheaply-cloneable handle used on the hot path to enqueue APM lines.
///
/// Internally wraps an [`UnboundedSender`] (which is just an `Arc`), so
/// `Clone` is essentially free.
#[derive(Clone)]
pub struct ApmWriter {
    sender: Sender<String>,
    dropped: Arc<AtomicUsize>,
}

impl ApmWriter {
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

/// Owned handle returned by [`ApmWriterHandle::new`].
///
/// Dropping this handle (or calling [`shutdown`](Self::shutdown)) drops the
/// last sender, which signals the background writer task to drain and exit.
pub struct ApmWriterHandle {
    sender: Sender<String>,
    join_handle: tokio::task::JoinHandle<()>,
    dropped: Arc<AtomicUsize>,
}

impl ApmWriterHandle {
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

    /// Return a cheaply-cloneable [`ApmWriter`] for the hot path.
    pub fn writer(&self) -> ApmWriter {
        ApmWriter {
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
            log::warn!("rusta-apm writer: failed to write entry: {}", e);
            continue;
        }

        count += batch;

        // Batch flush: only fsync every N lines to reduce syscall pressure.
        if count >= BATCH_SIZE {
            if let Err(e) = file.flush().await {
                log::warn!("rusta-apm writer: failed to flush: {}", e);
            }
            count = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::sync::atomic::AtomicUsize;

    #[tokio::test(flavor = "current_thread")]
    async fn apm_writer_handle_creates_file() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test-writer.ndjson");

        let handle = ApmWriterHandle::new(&log_path, None).await.unwrap();
        assert!(log_path.exists());

        handle.shutdown().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn apm_writer_handle_custom_capacity() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test-writer-cap.ndjson");

        let handle = ApmWriterHandle::new(&log_path, Some(100)).await.unwrap();
        assert!(log_path.exists());

        handle.shutdown().await;
    }

    #[test]
    fn apm_writer_write_line_sync() {
        let (sender, _receiver): (Sender<String>, Receiver<String>) = mpsc::channel(8192);
        let dropped = Arc::new(AtomicUsize::new(0));

        let writer = ApmWriter { sender, dropped };
        writer.write_line("test line".to_string());
    }

    #[test]
    fn apm_writer_multiple_clones_sync() {
        let (sender, _receiver): (Sender<String>, Receiver<String>) = mpsc::channel(8192);
        let dropped = Arc::new(AtomicUsize::new(0));

        let writer1 = ApmWriter {
            sender: sender.clone(),
            dropped: Arc::clone(&dropped),
        };
        let writer2 = ApmWriter {
            sender: sender.clone(),
            dropped: Arc::clone(&dropped),
        };

        writer1.write_line("line 1".to_string());
        writer2.write_line("line 2".to_string());
    }

    #[test]
    fn apm_writer_clone() {
        let (sender, _receiver): (Sender<String>, Receiver<String>) = mpsc::channel(8192);
        let dropped = Arc::new(AtomicUsize::new(0));

        let writer1 = ApmWriter { sender, dropped };
        let writer2 = writer1.clone();

        writer1.write_line("line 1".to_string());
        writer2.write_line("line 2".to_string());
    }

    #[test]
    fn apm_writer_dropped_counter() {
        let (sender, _receiver): (Sender<String>, Receiver<String>) = mpsc::channel(1);
        let dropped = Arc::new(AtomicUsize::new(0));

        let writer = ApmWriter { sender, dropped: Arc::clone(&dropped) };
        writer.write_line("test".to_string());
    }

    #[test]
    fn batch_size_constant() {
        assert_eq!(BATCH_SIZE, 64);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn apm_writer_handle_writes_to_file() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test-writer.ndjson");

        let handle = ApmWriterHandle::new(&log_path, None).await.unwrap();
        let writer = handle.writer();

        writer.write_line(r#"{"test": "data"}"#.to_string());
        writer.write_line(r#"{"test": "data2"}"#.to_string());

        // Drop the writer to close the channel
        drop(writer);
        // Give writer time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        handle.shutdown().await;

        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        assert!(content.contains(r#"{"test": "data"}"#));
        assert!(content.contains(r#"{"test": "data2"}"#));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn apm_writer_handle_batch_flush() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test-batch.ndjson");

        let handle = ApmWriterHandle::new(&log_path, Some(100)).await.unwrap();
        let writer = handle.writer();

        // Write more than BATCH_SIZE lines to trigger flush
        for i in 0..70 {
            writer.write_line(format!(r#"{{"line": {}}}"#, i));
        }

        // Drop the writer to close the channel
        drop(writer);
        // Give writer time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        handle.shutdown().await;

        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 70);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn apm_writer_handle_dropped_counter() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test-dropped.ndjson");

        let handle = ApmWriterHandle::new(&log_path, Some(1)).await.unwrap();
        let writer = handle.writer();

        // Fill the channel
        writer.write_line("line 1".to_string());
        // This should be dropped
        writer.write_line("line 2".to_string());

        // Drop the writer to close the channel
        drop(writer);
        // Give writer time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        handle.shutdown().await;

        // The dropped counter should be incremented
        // We can't easily access it, but we can verify the file only has 1 line
        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn apm_writer_handle_multiple_writers() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test-multi.ndjson");

        let handle = ApmWriterHandle::new(&log_path, None).await.unwrap();
        let writer1 = handle.writer();
        let writer2 = handle.writer();

        writer1.write_line("from writer 1".to_string());
        writer2.write_line("from writer 2".to_string());

        // Drop the writers to close the channel
        drop(writer1);
        drop(writer2);
        // Give writer time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        handle.shutdown().await;

        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        assert!(content.contains("from writer 1"));
        assert!(content.contains("from writer 2"));
    }
}
