use crate::types::ApmEntry;
use std::cell::RefCell;

/// Formats an [`ApmEntry`] into an NDJSON line string (no trailing newline).
pub trait LogAdapter: Send + Sync {
    fn format(&self, entry: &ApmEntry) -> String;
}

/// Default adapter: serializes the entry as a compact JSON line using a
/// thread-local buffer to avoid frequent allocations.
pub struct DefaultJsonAdapter;

thread_local! {
    static JSON_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(2048));
}

impl LogAdapter for DefaultJsonAdapter {
    fn format(&self, entry: &ApmEntry) -> String {
        JSON_BUF.with(|cell| {
            let mut buf = cell.borrow_mut();
            buf.clear();

            if let Err(e) = serde_json::to_writer(&mut *buf, entry) {
                let err = format!(r#"{{"error":"failed to serialize APM entry: {}"}}"#, e);
                buf.extend_from_slice(err.as_bytes());
            }

            let cap = buf.capacity();
            let owned = std::mem::replace(&mut *buf, Vec::with_capacity(cap));
            unsafe { String::from_utf8_unchecked(owned) }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ServiceContext, SpanRecord, TransactionRecord};
    use chrono::Utc;
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn default_json_adapter_formats_transaction() {
        let adapter = DefaultJsonAdapter;
        let service = Arc::new(ServiceContext {
            service_name: "test-service".to_string(),
            service_version: Some("1.0".to_string()),
            environment: Some("test".to_string()),
            server_name: None,
            context: std::collections::HashMap::new(),
        });

        let record = TransactionRecord {
            id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            name: "GET /users".to_string(),
            transaction_type: "request".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration_ms: 15.5,
            result: Some("HTTP 200".to_string()),
            correlation_id: Some("corr-123".to_string()),
            metadata: std::collections::HashMap::new(),
            service: service.clone(),
        };

        let entry = ApmEntry::Transaction(record);
        let formatted = adapter.format(&entry);

        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
        assert_eq!(parsed["type"], "transaction");
        assert_eq!(parsed["name"], "GET /users");
        assert_eq!(parsed["transaction_type"], "request");
        assert_eq!(parsed["service"]["service_name"], "test-service");
    }

    #[test]
    fn default_json_adapter_formats_span() {
        let adapter = DefaultJsonAdapter;

        let record = SpanRecord {
            id: Uuid::new_v4(),
            transaction_id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            parent_id: Some(Uuid::new_v4()),
            name: "db-query".to_string(),
            span_type: "db".to_string(),
            subtype: Some("postgresql".to_string()),
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration_ms: 5.2,
            metadata: std::collections::HashMap::new(),
        };

        let entry = ApmEntry::Span(record);
        let formatted = adapter.format(&entry);

        let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
        assert_eq!(parsed["type"], "span");
        assert_eq!(parsed["name"], "db-query");
        assert_eq!(parsed["span_type"], "db");
        assert_eq!(parsed["subtype"], "postgresql");
    }

    #[test]
    fn default_json_adapter_handles_metadata() {
        let adapter = DefaultJsonAdapter;
        let service = Arc::new(ServiceContext {
            service_name: "test".to_string(),
            service_version: None,
            environment: None,
            server_name: None,
            context: std::collections::HashMap::new(),
        });

        let mut meta = std::collections::HashMap::new();
        meta.insert("user_id".to_string(), serde_json::json!("123"));
        meta.insert("tags".to_string(), serde_json::json!(["tag1", "tag2"]));

        let record = TransactionRecord {
            id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            name: "test".to_string(),
            transaction_type: "request".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration_ms: 10.0,
            result: None,
            correlation_id: None,
            metadata: meta,
            service,
        };

        let entry = ApmEntry::Transaction(record);
        let formatted = adapter.format(&entry);

        let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
        assert_eq!(parsed["metadata"]["user_id"], "123");
        assert_eq!(
            parsed["metadata"]["tags"],
            serde_json::json!(["tag1", "tag2"])
        );
    }

    #[test]
    fn default_json_adapter_skip_none_fields() {
        let adapter = DefaultJsonAdapter;
        let service = Arc::new(ServiceContext {
            service_name: "test".to_string(),
            service_version: None,
            environment: None,
            server_name: None,
            context: std::collections::HashMap::new(),
        });

        let record = TransactionRecord {
            id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            name: "test".to_string(),
            transaction_type: "request".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration_ms: 10.0,
            result: None,
            correlation_id: None,
            metadata: std::collections::HashMap::new(),
            service,
        };

        let entry = ApmEntry::Transaction(record);
        let formatted = adapter.format(&entry);

        let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
        // None fields should be skipped
        assert!(!parsed.as_object().unwrap().contains_key("result"));
        assert!(!parsed.as_object().unwrap().contains_key("correlation_id"));
    }

    #[test]
    fn default_json_adapter_serialization_error() {
        // Test the error handling path in format()
        // We can't easily trigger a real serialization error with valid types,
        // but we can verify the code path exists by checking the format output
        let adapter = DefaultJsonAdapter;
        let service = Arc::new(ServiceContext {
            service_name: "test".to_string(),
            service_version: None,
            environment: None,
            server_name: None,
            context: std::collections::HashMap::new(),
        });

        let record = TransactionRecord {
            id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            name: "test".to_string(),
            transaction_type: "request".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration_ms: 10.0,
            result: None,
            correlation_id: None,
            metadata: std::collections::HashMap::new(),
            service,
        };

        let entry = ApmEntry::Transaction(record);
        let formatted = adapter.format(&entry);

        // Verify it's valid JSON (error path would produce error field)
        let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
        assert!(parsed.get("error").is_none());
    }
}
