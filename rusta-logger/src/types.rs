use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum LogLevel {
    Trace = 0,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

// ── Serialize helpers for Arc-wrapped types ────────────────────────────────

fn serialize_arc_str<S: Serializer>(x: &Arc<str>, s: S) -> Result<S::Ok, S::Error> {
    x.as_ref().serialize(s)
}

fn serialize_arc_service_context<S: Serializer>(
    x: &Arc<ServiceContext>,
    s: S,
) -> Result<S::Ok, S::Error> {
    x.as_ref().serialize(s)
}

fn serialize_opt_arc_str<S: Serializer>(x: &Option<Arc<str>>, s: S) -> Result<S::Ok, S::Error> {
    match x {
        Some(v) => serialize_arc_str(v, s),
        None => s.serialize_none(),
    }
}

fn is_arc_str_none(x: &Option<Arc<str>>) -> bool {
    x.is_none()
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    /// Classification label, shared with zero-copy clone.
    #[serde(serialize_with = "serialize_arc_str")]
    pub classification: Arc<str>,
    /// Correlation ID, shared with zero-copy clone.
    #[serde(
        skip_serializing_if = "is_arc_str_none",
        serialize_with = "serialize_opt_arc_str"
    )]
    pub correlation_id: Option<Arc<str>>,
    /// Service context, shared with zero-copy clone.
    #[serde(serialize_with = "serialize_arc_service_context")]
    pub service: Arc<ServiceContext>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: Metadata,
}

/// Caller-supplied per-message options.
#[derive(Debug, Clone, Default)]
pub struct LogOptions {
    /// Overrides the default classification for this message.
    pub classification: Option<String>,
    /// Extra structured fields merged into `LogEntry.context`.
    pub context: Option<Metadata>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "TRACE");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Warn.to_string(), "WARN");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
    }

    #[test]
    fn log_level_ord_and_partial_ord() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn service_context_serialize_minimal() {
        let ctx = ServiceContext {
            service_name: "test-service".to_string(),
            service_version: None,
            environment: None,
            server_name: None,
            context: HashMap::new(),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("test-service"));
        assert!(!json.contains("service_version"));
        assert!(!json.contains("environment"));
    }

    #[test]
    fn service_context_serialize_with_optional_fields() {
        let mut ctx = ServiceContext {
            service_name: "api".to_string(),
            service_version: Some("2.0.0".to_string()),
            environment: Some("production".to_string()),
            server_name: Some("server-1".to_string()),
            context: HashMap::new(),
        };
        ctx.context.insert("region".to_string(), serde_json::json!("us-east-1"));
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("api"));
        assert!(json.contains("2.0.0"));
        assert!(json.contains("production"));
        assert!(json.contains("server-1"));
        assert!(json.contains("us-east-1"));
    }

    #[test]
    fn log_entry_serialize_with_correlation_id() {
        let entry = LogEntry {
            timestamp: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            level: LogLevel::Info,
            message: "test message".to_string(),
            classification: Arc::from("PUBLIC"),
            correlation_id: Some(Arc::from("corr-123")),
            service: Arc::new(ServiceContext {
                service_name: "svc".to_string(),
                ..Default::default()
            }),
            context: HashMap::new(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("test message"));
        assert!(json.contains("PUBLIC"));
        assert!(json.contains("corr-123"));
    }

    #[test]
    fn log_entry_serialize_without_correlation_id() {
        let entry = LogEntry {
            timestamp: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            level: LogLevel::Error,
            message: "error occurred".to_string(),
            classification: Arc::from("PRIVATE"),
            correlation_id: None,
            service: Arc::new(ServiceContext {
                service_name: "svc".to_string(),
                ..Default::default()
            }),
            context: HashMap::new(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("error occurred"));
        assert!(json.contains("PRIVATE"));
    }

    #[test]
    fn log_entry_serialize_with_context() {
        let mut ctx = HashMap::new();
        ctx.insert("user_id".to_string(), serde_json::json!("user-42"));
        ctx.insert("duration_ms".to_string(), serde_json::json!(150));

        let entry = LogEntry {
            timestamp: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            level: LogLevel::Debug,
            message: "debug info".to_string(),
            classification: Arc::from("PUBLIC"),
            correlation_id: None,
            service: Arc::new(ServiceContext {
                service_name: "svc".to_string(),
                ..Default::default()
            }),
            context: ctx,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("user-42"));
        assert!(json.contains("150"));
    }
}