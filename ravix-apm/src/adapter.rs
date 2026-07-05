use crate::types::ApmEntry;

/// Formats an [`ApmEntry`] into an NDJSON line string (no trailing newline).
pub trait LogAdapter: Send + Sync {
    fn format(&self, entry: &ApmEntry) -> String;
}

/// Default adapter: serializes the entry as a compact JSON line.
pub struct DefaultJsonAdapter;

impl LogAdapter for DefaultJsonAdapter {
    fn format(&self, entry: &ApmEntry) -> String {
        serde_json::to_string(entry).unwrap_or_else(|e| {
            format!(r#"{{"error":"failed to serialize APM entry: {}"}}"#, e)
        })
    }
}