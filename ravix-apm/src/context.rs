use std::cell::Cell;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use chrono::{DateTime, Utc};
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