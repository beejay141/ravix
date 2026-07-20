mod adapter;
mod config;
mod context;
mod manager;
#[cfg(feature = "axum-middleware")]
mod middleware;
mod span;
mod transaction;
mod types;
mod writer;

pub use adapter::{DefaultJsonAdapter, LogAdapter};
pub use config::{config, ApmConfig, ApmConfigBuilder};
pub use context::ActiveTransaction;
pub use manager::Apm;
#[cfg(feature = "axum-middleware")]
pub use middleware::apm_middleware;
pub use span::SpanHandle;
pub use transaction::TransactionHandle;
pub use types::{ApmEntry, Metadata, ServiceContext, SpanRecord, TransactionRecord};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn apm_config_builder_builds_defaults() {
        let cfg = config().service_name("svc").build();
        assert_eq!(cfg.service.service_name, "svc");
        // default log path should be apm.ndjson
        assert!(cfg.log_path.ends_with("apm.ndjson"));
    }

    #[test]
    fn apm_config_builder_with_all_options() {
        let cfg = config()
            .service_name("my-service")
            .service_version("1.0.0")
            .environment("production")
            .log_path("/tmp/apm.log")
            .correlation_id_header("X-Request-ID")
            .build();
        assert_eq!(cfg.service.service_name, "my-service");
        assert_eq!(cfg.service.service_version, Some("1.0.0".to_string()));
        assert_eq!(cfg.service.environment, Some("production".to_string()));
        assert!(cfg.log_path.ends_with("/tmp/apm.log"));
        assert_eq!(cfg.correlation_id_header, Some("X-Request-ID".to_string()));
    }

    #[test]
    fn apm_new_returns_arc() {
        let apm = Apm::new();
        assert!(Arc::strong_count(&apm) >= 1);
    }

    #[test]
    fn span_handle_noop_is_noop() {
        let handle = SpanHandle::noop();
        // noop handle should not panic on end
        handle.end(None);
    }

    #[test]
    fn transaction_handle_new_and_end() {
        let txn = Arc::new(ActiveTransaction::new("test".to_string()));
        let handle = TransactionHandle::new(txn, "test-type".to_string());
        handle.end(None, None);
    }

    #[test]
    fn types_service_context_default() {
        let ctx = ServiceContext::default();
        assert!(ctx.service_name.is_empty());
        assert!(ctx.service_version.is_none());
        assert!(ctx.environment.is_none());
        assert!(ctx.server_name.is_none());
        assert!(ctx.context.is_empty());
    }

    #[test]
    fn types_metadata_new() {
        let meta: Metadata = Metadata::new();
        assert!(meta.is_empty());
    }

    #[test]
    fn types_metadata_insert_and_get() {
        let mut meta = Metadata::new();
        meta.insert("key".to_string(), serde_json::json!("value"));
        assert_eq!(meta.get("key"), Some(&serde_json::json!("value")));
    }
}
