use std::collections::HashMap;
use std::path::PathBuf;

use crate::adapter::{DefaultJsonAdapter, LogAdapter};
use crate::types::{LogLevel, ServiceContext};

/// Classification-to-file mapping.
pub struct LogClassificationConfig {
    /// Classification label (e.g. `"PUBLIC"`, `"CONFIDENTIAL"`).
    pub name: String,
    /// Dedicated file for this classification.
    pub log_path: PathBuf,
}

/// Top-level logger configuration.
pub struct LoggerConfig {
    pub service: ServiceContext,
    pub min_level: LogLevel,
    pub classifications: Vec<LogClassificationConfig>,
    pub default_classification: String,
    pub correlation_id_header: String,
    pub channel_capacity: Option<usize>,
    pub adapter: Box<dyn LogAdapter + Send + Sync>,
}

/// Fluent builder for [`LoggerConfig`].
pub struct LoggerConfigBuilder {
    service: ServiceContext,
    min_level: LogLevel,
    classifications: Vec<LogClassificationConfig>,
    default_classification: Option<String>,
    correlation_id_header: Option<String>,
    channel_capacity: Option<usize>,
    adapter: Option<Box<dyn LogAdapter + Send + Sync>>,
}

impl LoggerConfigBuilder {
    fn new() -> Self {
        Self {
            service: ServiceContext {
                service_name: String::new(),
                service_version: None,
                environment: None,
                server_name: None,
                context: HashMap::new(),
            },
            min_level: LogLevel::Info,
            classifications: Vec::new(),
            default_classification: None,
            correlation_id_header: None,
            channel_capacity: None,
            adapter: None,
        }
    }

    pub fn service_name(mut self, name: impl Into<String>) -> Self {
        self.service.service_name = name.into();
        self
    }

    pub fn service_version(mut self, version: impl Into<String>) -> Self {
        self.service.service_version = Some(version.into());
        self
    }

    pub fn environment(mut self, env: impl Into<String>) -> Self {
        self.service.environment = Some(env.into());
        self
    }

    pub fn server_name(mut self, name: impl Into<String>) -> Self {
        self.service.server_name = Some(name.into());
        self
    }

    /// Add custom key-value context to the service context.
    /// This context will be included in all log entries.
    pub fn context(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.service.context.insert(key.into(), value.into());
        self
    }

    pub fn min_level(mut self, level: LogLevel) -> Self {
        self.min_level = level;
        self
    }

    pub fn add_classification(mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        self.classifications.push(LogClassificationConfig {
            name: name.into(),
            log_path: path.into(),
        });
        self
    }

    pub fn default_classification(mut self, name: impl Into<String>) -> Self {
        self.default_classification = Some(name.into());
        self
    }

    pub fn correlation_id_header(mut self, header: impl Into<String>) -> Self {
        self.correlation_id_header = Some(header.into());
        self
    }

    /// Optional: set the per-writer channel capacity. When not set the
    /// default capacity is used (8192).
    pub fn channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = Some(capacity);
        self
    }

    pub fn adapter(mut self, adapter: Box<dyn LogAdapter + Send + Sync>) -> Self {
        self.adapter = Some(adapter);
        self
    }

    pub fn build(self) -> LoggerConfig {
        let default_classification = self
            .default_classification
            .unwrap_or_else(|| "PUBLIC".to_string());

        assert!(
            !self.service.service_name.is_empty(),
            "rusta-logger: service_name must be set"
        );
        assert!(
            !self.classifications.is_empty(),
            "rusta-logger: at least one classification must be added"
        );

        let valid = self
            .classifications
            .iter()
            .any(|c| c.name == default_classification);
        assert!(
            valid,
            "rusta-logger: default_classification '{}' does not match any configured classification",
            default_classification
        );

        LoggerConfig {
            service: self.service,
            min_level: self.min_level,
            classifications: self.classifications,
            default_classification,
            correlation_id_header: self
                .correlation_id_header
                .unwrap_or_else(|| "X-Correlation-ID".to_string()),
            channel_capacity: self.channel_capacity,
            adapter: self.adapter.unwrap_or_else(|| Box::new(DefaultJsonAdapter)),
        }
    }
}

/// Create a new [`LoggerConfigBuilder`].
pub fn config() -> LoggerConfigBuilder {
    LoggerConfigBuilder::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builder_default_min_level() {
        let cfg = config()
            .service_name("svc")
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .default_classification("PUBLIC")
            .build();

        assert_eq!(cfg.min_level, LogLevel::Info);
    }

    #[test]
    fn config_builder_custom_min_level() {
        let cfg = config()
            .service_name("svc")
            .min_level(LogLevel::Debug)
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .default_classification("PUBLIC")
            .build();

        assert_eq!(cfg.min_level, LogLevel::Debug);
    }

    #[test]
    fn config_builder_multiple_classifications() {
        let cfg = config()
            .service_name("svc")
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .add_classification("PRIVATE", "/tmp/private.ndjson")
            .add_classification("CONFIDENTIAL", "/tmp/confidential.ndjson")
            .default_classification("PUBLIC")
            .build();

        assert_eq!(cfg.classifications.len(), 3);
        assert_eq!(cfg.classifications[0].name, "PUBLIC");
        assert_eq!(cfg.classifications[1].name, "PRIVATE");
        assert_eq!(cfg.classifications[2].name, "CONFIDENTIAL");
    }

    #[test]
    fn config_builder_default_correlation_header() {
        let cfg = config()
            .service_name("svc")
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .default_classification("PUBLIC")
            .build();

        assert_eq!(cfg.correlation_id_header, "X-Correlation-ID");
    }

    #[test]
    fn config_builder_custom_correlation_header() {
        let cfg = config()
            .service_name("svc")
            .correlation_id_header("X-Request-ID")
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .default_classification("PUBLIC")
            .build();

        assert_eq!(cfg.correlation_id_header, "X-Request-ID");
    }

    #[test]
    fn config_builder_default_classification_when_not_set() {
        let cfg = config()
            .service_name("svc")
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .build();

        assert_eq!(cfg.default_classification, "PUBLIC");
    }

    #[test]
    fn config_builder_service_context_accumulation() {
        let cfg = config()
            .service_name("my-service")
            .service_version("1.0.0")
            .environment("production")
            .server_name("server-1")
            .context("region", "us-east-1")
            .context("instance_id", 42)
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .default_classification("PUBLIC")
            .build();

        assert_eq!(cfg.service.service_name, "my-service");
        assert_eq!(cfg.service.service_version, Some("1.0.0".to_string()));
        assert_eq!(cfg.service.environment, Some("production".to_string()));
        assert_eq!(cfg.service.server_name, Some("server-1".to_string()));
        assert_eq!(cfg.service.context.get("region").unwrap(), "us-east-1");
        assert_eq!(cfg.service.context.get("instance_id").unwrap(), 42);
    }

    #[test]
    fn config_builder_custom_channel_capacity() {
        let cfg = config()
            .service_name("svc")
            .channel_capacity(100)
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .default_classification("PUBLIC")
            .build();

        assert_eq!(cfg.channel_capacity, Some(100));
    }

    #[test]
    fn config_builder_custom_adapter() {
        struct TestAdapter;
        impl LogAdapter for TestAdapter {
            fn format(&self, _entry: &crate::types::LogEntry) -> String {
                "test".to_string()
            }
        }

        let cfg = config()
            .service_name("svc")
            .adapter(Box::new(TestAdapter))
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .default_classification("PUBLIC")
            .build();

        // We just verify the adapter was set - we can't easily inspect it
        // because it's a trait object
        let _ = cfg;
    }

    #[test]
    #[should_panic(expected = "service_name must be set")]
    fn config_builder_panics_without_service_name() {
        let _ = config()
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .default_classification("PUBLIC")
            .build();
    }

    #[test]
    #[should_panic(expected = "at least one classification must be added")]
    fn config_builder_panics_without_classification() {
        let _ = config()
            .service_name("svc")
            .default_classification("PUBLIC")
            .build();
    }

    #[test]
    #[should_panic(expected = "default_classification")]
    fn config_builder_panics_with_invalid_default_classification() {
        let _ = config()
            .service_name("svc")
            .add_classification("PUBLIC", "/tmp/public.ndjson")
            .default_classification("NONEXISTENT")
            .build();
    }
}