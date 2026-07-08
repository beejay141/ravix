use std::cell::RefCell;
use std::io::Write;

use crate::types::LogEntry;

/// Formats a [`LogEntry`] into a string (no trailing newline).
pub trait LogAdapter: Send + Sync {
    fn format(&self, entry: &LogEntry) -> String;
}

/// Default adapter: serializes the entry as a compact JSON line.
///
/// Uses a thread-local scratch buffer to avoid allocating a fresh `String`
/// on every call.  The buffer grows to the largest entry seen on that thread
/// and is then reused.
pub struct DefaultJsonAdapter;

thread_local! {
    /// Reusable buffer for JSON serialisation.  Grows to the high-water mark
    /// and then stabilises — no allocations after the first few entries.
    static JSON_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(2048));
}

impl LogAdapter for DefaultJsonAdapter {
    fn format(&self, entry: &LogEntry) -> String {
        JSON_BUF.with(|cell| {
            let mut buf = cell.borrow_mut();
            buf.clear();

            serde_json::to_writer(&mut *buf, entry).unwrap_or_else(|e| {
                // On serialisation failure write an inline error object.
                write!(
                    &mut *buf,
                    r#"{{"error":"failed to serialize log entry: {}"}}"#,
                    e
                )
                .unwrap();
            });

            // SAFETY: serde_json always produces valid UTF-8.
            // The error path above also only emits ASCII-safe JSON.
            unsafe { String::from_utf8_unchecked(buf.clone()) }
        })
    }
}