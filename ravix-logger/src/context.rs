use std::sync::Arc;

/// Lightweight struct for future extensibility.
/// Additional fields can be added without breaking the task-local signature.
pub struct LogContext {
    /// Correlation ID shared via `Arc<str>` so cloning in [`super::Logger::log`]
    /// is a single ref-count bump instead of a heap allocation.
    pub correlation_id: Arc<str>,
}

tokio::task_local! {
    pub(crate) static CURRENT_LOG_CONTEXT: LogContext;
}

/// Read the correlation ID from the task-local context.
///
/// Returns `None` when called outside an async context or before the
/// middleware has run.
pub fn current_correlation_id() -> Option<Arc<str>> {
    CURRENT_LOG_CONTEXT
        .try_with(|ctx| Arc::clone(&ctx.correlation_id))
        .ok()
}