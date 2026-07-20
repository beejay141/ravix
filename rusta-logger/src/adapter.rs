use std::cell::RefCell;

use crate::types::LogEntry;

/// Formats a [`LogEntry`] into a string (no trailing newline).
pub trait LogAdapter: Send + Sync {
    fn format(&self, entry: &LogEntry) -> String;
}

/// Default adapter: serializes the entry as a compact JSON line.
///
/// Uses a thread-local scratch buffer to avoid allocating a fresh `String`
/// on every call. The buffer grows to the largest entry seen on that thread
/// and is then reused. To avoid an extra copy we swap the underlying
/// `Vec<u8>` out of the TLS slot and convert it directly into a `String`.
pub struct DefaultJsonAdapter;

thread_local! {
    /// Reusable buffer for JSON serialisation. Grows to the high-water mark
    /// and then stabilises — no allocations after the first few entries.
    static JSON_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(2048));
}

impl LogAdapter for DefaultJsonAdapter {
    fn format(&self, entry: &LogEntry) -> String {
        JSON_BUF.with(|cell| {
            let mut buf = cell.borrow_mut();
            buf.clear();

            if let Err(e) = serde_json::to_writer(&mut *buf, entry) {
                // On serialisation failure write an inline error object.
                let err = format!(r#"{{"error":"failed to serialize log entry: {}"}}"#, e);
                buf.extend_from_slice(err.as_bytes());
            }

            // Swap the buffer out so we can return it without copying.
            let cap = buf.capacity();
            let owned = std::mem::replace(&mut *buf, Vec::with_capacity(cap));
            // SAFETY: serde_json and the error path above produce valid UTF-8.
            unsafe { String::from_utf8_unchecked(owned) }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LogLevel, ServiceContext};
    use chrono::{DateTime, Utc};
    use serde_json;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn default_json_adapter_format_basic() {
        let entry = LogEntry {
            timestamp: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            level: LogLevel::Info,
            message: "test message".to_string(),
            classification: Arc::from("PUBLIC"),
            correlation_id: None,
            service: Arc::new(ServiceContext {
                service_name: "svc".to_string(),
                ..Default::default()
            }),
            context: HashMap::new(),
        };

        let adapter = DefaultJsonAdapter;
        let json = adapter.format(&entry);

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["message"], "test message");
        // LogLevel serializes as the variant name ("Info"), not the Display format ("INFO")
        assert_eq!(parsed["level"], "Info");
        assert_eq!(parsed["classification"], "PUBLIC");
    }

    #[test]
    fn default_json_adapter_format_with_correlation_id() {
        let entry = LogEntry {
            timestamp: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            level: LogLevel::Error,
            message: "error with corr".to_string(),
            classification: Arc::from("PRIVATE"),
            correlation_id: Some(Arc::from("corr-abc-123")),
            service: Arc::new(ServiceContext {
                service_name: "svc".to_string(),
                ..Default::default()
            }),
            context: HashMap::new(),
        };

        let adapter = DefaultJsonAdapter;
        let json = adapter.format(&entry);

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["correlation_id"], "corr-abc-123");
        assert_eq!(parsed["classification"], "PRIVATE");
    }

    #[test]
    fn default_json_adapter_format_with_context() {
        let mut ctx = HashMap::new();
        ctx.insert("key1".to_string(), serde_json::json!("value1"));
        ctx.insert("key2".to_string(), serde_json::json!(42));

        let entry = LogEntry {
            timestamp: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            level: LogLevel::Debug,
            message: "context test".to_string(),
            classification: Arc::from("PUBLIC"),
            correlation_id: None,
            service: Arc::new(ServiceContext {
                service_name: "svc".to_string(),
                ..Default::default()
            }),
            context: ctx,
        };

        let adapter = DefaultJsonAdapter;
        let json = adapter.format(&entry);

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["context"]["key1"], "value1");
        assert_eq!(parsed["context"]["key2"], 42);
    }

    #[test]
    fn default_json_adapter_no_trailing_newline() {
        let entry = LogEntry {
            timestamp: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            level: LogLevel::Info,
            message: "no newline".to_string(),
            classification: Arc::from("PUBLIC"),
            correlation_id: None,
            service: Arc::new(ServiceContext {
                service_name: "svc".to_string(),
                ..Default::default()
            }),
            context: HashMap::new(),
        };

        let adapter = DefaultJsonAdapter;
        let json = adapter.format(&entry);

        assert!(!json.ends_with('\n'));
    }
}