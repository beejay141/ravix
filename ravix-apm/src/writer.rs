use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

/// A dedicated async writer that receives NDJSON lines and appends them to a file.
///
/// The writer spawns a background task that holds an append-mode file handle.
/// Callers send completed JSON lines through an unbounded channel so request
/// handlers are never blocked on disk I/O.
pub struct ApmWriter {
    sender: UnboundedSender<String>,
}

impl ApmWriter {
    /// Open (or create) the log file in append mode and spawn the writer task.
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        let (sender, receiver) = mpsc::unbounded_channel();
        tokio::spawn(writer_loop(file, receiver));

        Ok(Self { sender })
    }

    /// Enqueue a formatted NDJSON line for writing.
    pub fn write_line(&self, line: String) {
        // Unbounded send never fails in practice unless the receiver is dropped.
        let _ = self.sender.send(line);
    }
}

async fn writer_loop(
    mut file: fs::File,
    mut receiver: UnboundedReceiver<String>,
) {
    while let Some(line) = receiver.recv().await {
        if let Err(e) = file.write_all(line.as_bytes()).await {
            log::warn!("ravix-apm writer: failed to write entry: {}", e);
            continue;
        }
        if let Err(e) = file.write_all(b"\n").await {
            log::warn!("ravix-apm writer: failed to write newline: {}", e);
            continue;
        }
        if let Err(e) = file.flush().await {
            log::warn!("ravix-apm writer: failed to flush: {}", e);
        }
    }
}
