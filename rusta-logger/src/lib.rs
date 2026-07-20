mod adapter;
mod config;
mod context;
mod manager;
#[cfg(feature = "axum-middleware")]
mod middleware;
mod types;
mod writer;

pub use adapter::{DefaultJsonAdapter, LogAdapter};
pub use config::{config, LogClassificationConfig, LoggerConfig, LoggerConfigBuilder};
pub use context::current_correlation_id;
pub use manager::Logger;
#[cfg(feature = "axum-middleware")]
pub use middleware::logger_middleware;
pub use types::{LogEntry, LogLevel, LogOptions, Metadata, ServiceContext};
pub use writer::{LogWriter, LogWriterHandle};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_options_default_has_no_classification() {
        let opts = LogOptions::default();
        assert!(opts.classification.is_none());
    }

    #[test]
    fn log_options_default_has_no_context() {
        let opts = LogOptions::default();
        assert!(opts.context.is_none());
    }

    #[test]
    fn log_options_can_override_classification() {
        let mut opts = LogOptions::default();
        opts.classification = Some("PRIVATE".to_string());
        assert_eq!(opts.classification, Some("PRIVATE".to_string()));
    }

    #[test]
    fn log_options_can_set_context() {
        let mut opts = LogOptions::default();
        let mut ctx = Metadata::new();
        ctx.insert("key".to_string(), serde_json::json!("value"));
        opts.context = Some(ctx);
        assert!(opts.context.is_some());
    }
}