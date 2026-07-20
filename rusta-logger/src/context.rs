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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_context_holds_correlation_id() {
        let ctx = LogContext {
            correlation_id: Arc::from("test-corr-id"),
        };
        assert_eq!(&*ctx.correlation_id, "test-corr-id");
    }

    #[test]
    fn log_context_arc_clone_cheap() {
        let ctx = LogContext {
            correlation_id: Arc::from("shared-id"),
        };
        let ctx2 = LogContext {
            correlation_id: Arc::clone(&ctx.correlation_id),
        };
        
        // Both share the same underlying data
        assert!(Arc::ptr_eq(&ctx.correlation_id, &ctx2.correlation_id));
        assert_eq!(Arc::strong_count(&ctx.correlation_id), 2);
    }

    #[tokio::test]
    async fn current_correlation_id_outside_scope_returns_none() {
        // Without being in a task_local scope, this should return None
        assert!(current_correlation_id().is_none());
    }

    #[tokio::test]
    async fn current_correlation_id_inside_scope_returns_value() {
        let id = Arc::from("request-123");
        CURRENT_LOG_CONTEXT.scope(
            LogContext {
                correlation_id: Arc::clone(&id),
            },
            async {
                let result = current_correlation_id();
                assert!(result.is_some());
                assert_eq!(&*result.unwrap(), "request-123");
            },
        ).await;
    }

    #[tokio::test]
    async fn current_correlation_id_nested_scopes() {
        let outer_id = Arc::from("outer-id");
        let inner_id = Arc::from("inner-id");

        CURRENT_LOG_CONTEXT.scope(
            LogContext {
                correlation_id: Arc::clone(&outer_id),
            },
            async {
                assert_eq!(&*current_correlation_id().unwrap(), "outer-id");

                CURRENT_LOG_CONTEXT.scope(
                    LogContext {
                        correlation_id: Arc::clone(&inner_id),
                    },
                    async {
                        // Inner scope should override
                        assert_eq!(&*current_correlation_id().unwrap(), "inner-id");
                    },
                ).await;

                // Back to outer
                assert_eq!(&*current_correlation_id().unwrap(), "outer-id");
            },
        ).await;
    }
}