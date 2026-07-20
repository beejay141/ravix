use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};
use uuid::Uuid;

pub type Metadata = HashMap<String, serde_json::Value>;

#[derive(Debug, Clone, Default, Serialize)]
pub struct ServiceContext {
    pub service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: Metadata,
}

// ── Serialize helpers for Arc-wrapped types ────────────────────────────────

fn serialize_arc_service_context<S: Serializer>(
    x: &Arc<ServiceContext>,
    s: S,
) -> Result<S::Ok, S::Error> {
    x.as_ref().serialize(s)
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct TransactionRecord {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub name: String,
    pub transaction_type: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: Metadata,
    #[serde(serialize_with = "serialize_arc_service_context")]
    pub service: Arc<ServiceContext>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpanRecord {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub trace_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub span_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_ms: f64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ApmEntry {
    #[serde(rename = "transaction")]
    Transaction(TransactionRecord),
    #[serde(rename = "span")]
    Span(SpanRecord),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_context_default() {
        let ctx = ServiceContext::default();
        assert!(ctx.service_name.is_empty());
        assert!(ctx.service_version.is_none());
        assert!(ctx.environment.is_none());
        assert!(ctx.server_name.is_none());
        assert!(ctx.context.is_empty());
    }

    #[test]
    fn service_context_serialization() {
        let ctx = ServiceContext {
            service_name: "test-service".to_string(),
            service_version: Some("1.0.0".to_string()),
            environment: Some("production".to_string()),
            server_name: None,
            context: Metadata::new(),
        };

        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("test-service"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("production"));
        // server_name should be skipped
        assert!(!json.contains("server_name"));
    }

    #[test]
    fn service_context_with_context() {
        let mut ctx = ServiceContext::default();
        ctx.service_name = "test".to_string();
        ctx.context.insert("region".to_string(), serde_json::json!("us-east-1"));

        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("region"));
        assert!(json.contains("us-east-1"));
    }

    #[test]
    fn transaction_record_serialization() {
        let service = Arc::new(ServiceContext {
            service_name: "svc".to_string(),
            ..Default::default()
        });

        let record = TransactionRecord {
            id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            name: "GET /users".to_string(),
            transaction_type: "request".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration_ms: 10.0,
            result: Some("HTTP 200".to_string()),
            correlation_id: Some("corr-123".to_string()),
            metadata: Metadata::new(),
            service,
        };

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("GET /users"));
        assert!(json.contains("request"));
        assert!(json.contains("HTTP 200"));
        assert!(json.contains("corr-123"));
    }

    #[test]
    fn span_record_serialization() {
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
            duration_ms: 5.0,
            metadata: Metadata::new(),
        };

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("db-query"));
        assert!(json.contains("db"));
        assert!(json.contains("postgresql"));
    }

    #[test]
    fn apm_entry_transaction_variant() {
        let service = Arc::new(ServiceContext {
            service_name: "svc".to_string(),
            ..Default::default()
        });

        let entry = ApmEntry::Transaction(TransactionRecord {
            id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            name: "test".to_string(),
            transaction_type: "request".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration_ms: 1.0,
            result: None,
            correlation_id: None,
            metadata: Metadata::new(),
            service,
        });

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains(r#""type":"transaction""#));
    }

    #[test]
    fn apm_entry_span_variant() {
        let entry = ApmEntry::Span(SpanRecord {
            id: Uuid::new_v4(),
            transaction_id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            parent_id: None,
            name: "test-span".to_string(),
            span_type: "custom".to_string(),
            subtype: None,
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration_ms: 1.0,
            metadata: Metadata::new(),
        });

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains(r#""type":"span""#));
    }
}
