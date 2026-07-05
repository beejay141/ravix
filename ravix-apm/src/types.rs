use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

pub type Metadata = HashMap<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, Default)]
pub struct ServiceContext {
    pub service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
}

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
    pub service: ServiceContext,
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