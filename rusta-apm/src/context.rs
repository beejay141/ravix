use chrono::{DateTime, Utc};
use std::cell::Cell;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

use crate::types::Metadata;

tokio::task_local! {
    pub(crate) static CURRENT_TXN: Arc<ActiveTransaction>;
    /// The ID of the currently active span, used to set `parent_id` on
    /// child spans. This enables nested span trees.
    pub(crate) static CURRENT_SPAN_ID: Cell<Uuid>;
}

/// In-flight transaction state carried on the task-local.
pub struct ActiveTransaction {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub name: String,
    /// Monotonic start instant for calculating wall-clock duration.
    pub start: Instant,
    /// Wall-clock timestamp for the record.
    pub wall_start: DateTime<Utc>,
    /// Mutable metadata that can be enriched during the transaction lifetime.
    pub metadata: Mutex<Metadata>,
    /// Cross-service correlation ID extracted from (or generated for) the
    /// incoming request. Set by the middleware or manually after
    /// `start_transaction`.
    pub correlation_id: Mutex<Option<String>>,
}

impl ActiveTransaction {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            trace_id: Uuid::new_v4(),
            name,
            start: Instant::now(),
            wall_start: Utc::now(),
            metadata: Mutex::new(Metadata::new()),
            correlation_id: Mutex::new(None),
        }
    }

    /// Attach a correlation ID to this transaction.
    pub fn set_correlation_id(&self, id: String) {
        if let Ok(mut cid) = self.correlation_id.lock() {
            *cid = Some(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_transaction_new_creates_unique_ids() {
        let txn1 = ActiveTransaction::new("txn-1".to_string());
        let txn2 = ActiveTransaction::new("txn-2".to_string());

        assert_ne!(txn1.id, txn2.id);
        assert_ne!(txn1.trace_id, txn2.trace_id);
        assert_eq!(txn1.name, "txn-1");
        assert_eq!(txn2.name, "txn-2");
    }

    #[test]
    fn active_transaction_metadata_mutex() {
        let txn = ActiveTransaction::new("test".to_string());

        // Add metadata
        {
            let mut meta = txn.metadata.lock().unwrap();
            meta.insert("key1".to_string(), serde_json::json!("value1"));
        }

        // Read metadata
        {
            let meta = txn.metadata.lock().unwrap();
            assert_eq!(meta.get("key1"), Some(&serde_json::json!("value1")));
        }
    }

    #[test]
    fn active_transaction_correlation_id() {
        let txn = ActiveTransaction::new("test".to_string());

        // Initially None
        {
            let cid = txn.correlation_id.lock().unwrap();
            assert!(cid.is_none());
        }

        // Set correlation ID
        txn.set_correlation_id("corr-123".to_string());

        // Verify it's set
        {
            let cid = txn.correlation_id.lock().unwrap();
            assert_eq!(*cid, Some("corr-123".to_string()));
        }
    }

    #[test]
    fn active_transaction_metadata_extend() {
        let txn = ActiveTransaction::new("test".to_string());

        // Add initial metadata
        {
            let mut meta = txn.metadata.lock().unwrap();
            meta.insert("key1".to_string(), serde_json::json!("value1"));
        }

        // Extend with more
        {
            let mut meta = txn.metadata.lock().unwrap();
            meta.extend(vec![("key2".to_string(), serde_json::json!("value2"))].into_iter());
        }

        // Verify both
        {
            let meta = txn.metadata.lock().unwrap();
            assert_eq!(meta.len(), 2);
            assert_eq!(meta.get("key1"), Some(&serde_json::json!("value1")));
            assert_eq!(meta.get("key2"), Some(&serde_json::json!("value2")));
        }
    }
}
