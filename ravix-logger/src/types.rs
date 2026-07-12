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
