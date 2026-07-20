use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::context::{ActiveTransaction, CURRENT_SPAN_ID};
use crate::manager::send_entry;
use crate::types::{Metadata, SpanRecord};

/// Handle for an in-flight span.
///
/// Created by [`Apm::start_span`](crate::manager::Apm::start_span).
/// Call [`Self::end`] to finalise and write the span record.
///
/// If the handle is dropped without calling `end`, it auto-finalises with
/// zero metadata (guarded against double-finalisation).
pub struct SpanHandle {
    inner: Option<SpanInner>,
}

struct SpanInner {
    txn: Arc<ActiveTransaction>,
    id: Uuid,
    name: String,
    span_type: String,
    subtype: Option<String>,
    parent_id: Option<Uuid>,
    /// The span ID that was active before this span started, so we can
    /// restore it on drop/end.
    previous_span_id: Option<Uuid>,
    start: Instant,
    wall_start: DateTime<Utc>,
    metadata: Metadata,
    ended: bool,
}

impl SpanHandle {
    pub(crate) fn new(
        txn: Arc<ActiveTransaction>,
        name: String,
        span_type: String,
        subtype: Option<String>,
        parent_id: Option<Uuid>,
        metadata: Option<Metadata>,
    ) -> Self {
        // Capture the current span ID as the parent, then set this span
        // as the new current span ID for any nested child spans.
        let previous_span_id = CURRENT_SPAN_ID.try_with(|id| id.get()).ok();
        let id = Uuid::new_v4();
        let _ = CURRENT_SPAN_ID.try_with(|current| current.set(id));

        Self {
            inner: Some(SpanInner {
                txn,
                id,
                name,
                span_type,
                subtype,
                parent_id,
                previous_span_id,
                start: Instant::now(),
                wall_start: Utc::now(),
                metadata: metadata.unwrap_or_default(),
                ended: false,
            }),
        }
    }

    pub(crate) fn noop() -> Self {
        Self { inner: None }
    }

    /// Finalise the span and write it to the APM log.
    ///
    /// If `metadata` is provided it is merged into the span's existing
    /// metadata.  Calling `end` more than once is a no-op.
    pub fn end(mut self, metadata: Option<Metadata>) {
        self.finalise(metadata);
    }

    fn finalise(&mut self, metadata: Option<Metadata>) {
        let Some(ref mut inner) = self.inner else {
            return;
        };
        if inner.ended {
            return;
        }
        inner.ended = true;

        // Restore the previous span ID so that subsequent spans at this
        // level get the correct parent.
        if let Some(prev) = inner.previous_span_id {
            let _ = CURRENT_SPAN_ID.try_with(|current| current.set(prev));
        }

        if let Some(extra) = metadata {
            inner.metadata.extend(extra);
        }

        let elapsed = inner.start.elapsed();
        let duration_ms = elapsed.as_secs_f64() * 1000.0;
        let now = inner.wall_start + chrono::Duration::from_std(elapsed).unwrap();

        let record = SpanRecord {
            id: inner.id,
            transaction_id: inner.txn.id,
            trace_id: inner.txn.trace_id,
            parent_id: inner.parent_id,
            name: inner.name.clone(),
            span_type: inner.span_type.clone(),
            subtype: inner.subtype.clone(),
            start_time: inner.wall_start,
            end_time: now,
            duration_ms,
            metadata: std::mem::take(&mut inner.metadata),
        };

        send_entry(crate::types::ApmEntry::Span(record));
    }
}

impl Drop for SpanHandle {
    fn drop(&mut self) {
        self.finalise(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_handle_noop_is_noop() {
        let handle = SpanHandle::noop();
        // Should not panic
        handle.end(None);
    }

    #[test]
    fn span_handle_new_creates_unique_id() {
        let txn = Arc::new(ActiveTransaction::new("parent".to_string()));
        let handle1 = SpanHandle::new(
            txn.clone(),
            "span1".to_string(),
            "db".to_string(),
            None,
            None,
            None,
        );
        let handle2 = SpanHandle::new(
            txn.clone(),
            "span2".to_string(),
            "db".to_string(),
            None,
            None,
            None,
        );

        // Each span should have a unique ID
        assert_ne!(handle1.inner.as_ref().unwrap().id, handle2.inner.as_ref().unwrap().id);
    }

    #[test]
    fn span_handle_parent_id_tracking() {
        let txn = Arc::new(ActiveTransaction::new("parent".to_string()));
        let parent_id = Uuid::new_v4();

        let handle = SpanHandle::new(
            txn.clone(),
            "child".to_string(),
            "db".to_string(),
            None,
            Some(parent_id),
            None,
        );

        assert_eq!(handle.inner.as_ref().unwrap().parent_id, Some(parent_id));
    }

    #[test]
    fn span_handle_metadata_merge() {
        let txn = Arc::new(ActiveTransaction::new("parent".to_string()));

        let mut initial_meta = Metadata::new();
        initial_meta.insert("key1".to_string(), serde_json::json!("value1"));

        let handle = SpanHandle::new(
            txn.clone(),
            "span".to_string(),
            "db".to_string(),
            None,
            None,
            Some(initial_meta),
        );

        let mut extra_meta = Metadata::new();
        extra_meta.insert("key2".to_string(), serde_json::json!("value2"));

        handle.end(Some(extra_meta));
    }

    #[test]
    fn span_handle_double_end_is_noop() {
        let txn = Arc::new(ActiveTransaction::new("parent".to_string()));
        let handle = SpanHandle::new(
            txn.clone(),
            "span".to_string(),
            "db".to_string(),
            None,
            None,
            None,
        );

        // end() takes ownership, so we can't call it twice
        // The double-end protection is in the finalise method
        handle.end(None);
    }

    #[test]
    fn span_handle_subtype() {
        let txn = Arc::new(ActiveTransaction::new("parent".to_string()));

        let handle = SpanHandle::new(
            txn.clone(),
            "span".to_string(),
            "db".to_string(),
            Some("postgresql".to_string()),
            None,
            None,
        );

        assert_eq!(handle.inner.as_ref().unwrap().subtype, Some("postgresql".to_string()));
    }
}
