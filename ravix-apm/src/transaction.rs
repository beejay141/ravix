use chrono::Utc;
use std::sync::Arc;

use crate::context::ActiveTransaction;
use crate::manager::send_entry;
use crate::types::{Metadata, TransactionRecord};

/// Handle for an in-flight transaction.
///
/// Created by [`Apm::start_transaction`](crate::manager::Apm::start_transaction).
/// Call [`Self::end`] to finalise the transaction and write the record.
///
/// If the handle is dropped without calling `end`, it auto-finalises with
/// `result: None` and no extra metadata (guarded against double-finalisation).
/// This ensures transactions are recorded even when a handler panics and the
/// handle is dropped during unwinding.
pub struct TransactionHandle {
    inner: Option<TransactionInner>,
}

struct TransactionInner {
    txn: Arc<ActiveTransaction>,
    transaction_type: String,
    ended: bool,
}

impl TransactionHandle {
    pub(crate) fn new(txn: Arc<ActiveTransaction>, transaction_type: String) -> Self {
        Self {
            inner: Some(TransactionInner {
                txn,
                transaction_type,
                ended: false,
            }),
        }
    }

    /// Obtain a clone of the internal [`ActiveTransaction`] for use with
    /// [`CURRENT_TXN.scope`](crate::context::CURRENT_TXN).
    pub fn active_txn(&self) -> Arc<ActiveTransaction> {
        self.inner
            .as_ref()
            .expect("TransactionHandle inner missing")
            .txn
            .clone()
    }

    /// End the transaction, build a [`TransactionRecord`], and send it to the
    /// APM writer.
    ///
    /// Consumes the handle. If not called, the transaction is auto-finalised
    /// on drop with `result: None`.
    pub fn end(mut self, result: Option<&str>, metadata: Option<Metadata>) {
        self.finalise(result, metadata);
    }

    fn finalise(&mut self, result: Option<&str>, metadata: Option<Metadata>) {
        let Some(ref mut inner) = self.inner else {
            return;
        };
        if inner.ended {
            return;
        }
        inner.ended = true;

        let now = Utc::now();
        let duration_ms = inner.txn.start.elapsed().as_secs_f64() * 1000.0;

        let mut meta = inner
            .txn
            .metadata
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(extra) = metadata {
            meta.extend(extra);
        }

        let correlation_id = inner
            .txn
            .correlation_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        let record = TransactionRecord {
            id: inner.txn.id,
            trace_id: inner.txn.trace_id,
            name: inner.txn.name.clone(),
            transaction_type: inner.transaction_type.clone(),
            start_time: inner.txn.wall_start,
            end_time: now,
            duration_ms,
            result: result.map(|s| s.to_string()),
            correlation_id,
            metadata: meta,
            service: Default::default(), // filled by manager
        };

        send_entry(crate::types::ApmEntry::Transaction(record));
    }
}

impl Drop for TransactionHandle {
    fn drop(&mut self) {
        self.finalise(None, None);
    }
}