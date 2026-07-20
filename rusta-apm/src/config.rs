use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::adapter::{DefaultJsonAdapter, LogAdapter};
use crate::types::ServiceContext;

/// Configuration for the APM subsystem.
pub struct ApmConfig {
    pub service: Arc<ServiceContext>,
    pub log_path: PathBuf,
    pub adapter: Box<dyn LogAdapter + Send + Sync>,
    /// Optional header name for cross-service correlation IDs.
    /// When set (e.g. `"X-Correlation-ID"`), the middleware reads the value
    /// from incoming requests, generates a new UUID if missing, and echoes
    /// it back in the response header. When `None`, no correlation-id
    /// handling is performed.
    pub correlation_id_header: Option<String>,
    /// Optional channel capacity for the APM writer. When `None`, a sane
    /// default is used (8192).
    pub channel_capacity: Option<usize>,
}

/// Fluent builder for [`ApmConfig`].
pub struct ApmConfigBuilder {
    service: ServiceContext,
    log_path: Option<PathBuf>,
    adapter: Option<Box<dyn LogAdapter + Send + Sync>>,
    correlation_id_header: Option<String>,
    channel_capacity: Option<usize>,
}

impl ApmConfigBuilder {
    fn new() -> Self {
        Self {
            service: ServiceContext {
                service_name: String::new(),
                service_version: None,
                environment: None,
                server_name: None,
                context: HashMap::new(),
            },
            log_path: None,
            adapter: None,
            correlation_id_header: None,
            channel_capacity: None,
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
    /// This context will be included in all APM entries.
    pub fn context(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.service.context.insert(key.into(), value.into());
        self
    }

    pub fn log_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.log_path = Some(path.into());
        self
    }

    pub fn adapter(mut self, adapter: Box<dyn LogAdapter + Send + Sync>) -> Self {
        self.adapter = Some(adapter);
        self
    }

    /// Set the request/response header name used for service correlation
    /// IDs (e.g. `"X-Correlation-ID"`). When set, the APM middleware
    /// extracts the value from the request header, generates one if
    /// missing, and injects it into the response.
    pub fn correlation_id_header(mut self, header: impl Into<String>) -> Self {
        self.correlation_id_header = Some(header.into());
        self
    }

    /// Optional: set the APM writer channel capacity. When not set the
    /// default capacity is used (8192).
    pub fn channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = Some(capacity);
        self
    }

    pub fn build(self) -> ApmConfig {
        ApmConfig {
            service: Arc::new(self.service),
            log_path: self.log_path.unwrap_or_else(|| PathBuf::from("apm.ndjson")),
            adapter: self.adapter.unwrap_or_else(|| Box::new(DefaultJsonAdapter)),
            correlation_id_header: self.correlation_id_header,
            channel_capacity: self.channel_capacity,
        }
    }
}

/// Create a new [`ApmConfigBuilder`].
pub fn config() -> ApmConfigBuilder {
    ApmConfigBuilder::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builder_defaults() {
        let cfg = config().service_name("test-service").build();
        assert_eq!(cfg.service.service_name, "test-service");
        assert!(cfg.log_path.ends_with("apm.ndjson"));
        assert!(cfg.correlation_id_header.is_none());
        assert!(cfg.channel_capacity.is_none());
    }

    #[test]
    fn config_builder_all_options() {
        let cfg = config()
            .service_name("my-service")
            .service_version("1.0.0")
            .environment("production")
            .server_name("server-1")
            .log_path("/var/log/apm.ndjson")
            .correlation_id_header("X-Correlation-ID")
            .channel_capacity(16384)
            .context("region", "us-east-1")
            .context("cluster", "prod-cluster")
            .build();

        assert_eq!(cfg.service.service_name, "my-service");
        assert_eq!(cfg.service.service_version, Some("1.0.0".to_string()));
        assert_eq!(cfg.service.environment, Some("production".to_string()));
        assert_eq!(cfg.service.server_name, Some("server-1".to_string()));
        assert_eq!(cfg.log_path, PathBuf::from("/var/log/apm.ndjson"));
        assert_eq!(
            cfg.correlation_id_header,
            Some("X-Correlation-ID".to_string())
        );
        assert_eq!(cfg.channel_capacity, Some(16384));
        assert_eq!(cfg.service.context.len(), 2);
    }

    #[test]
    fn config_builder_context_overwrites() {
        let cfg = config()
            .service_name("test")
            .context("key", "value1")
            .context("key", "value2")
            .build();

        assert_eq!(
            cfg.service.context.get("key"),
            Some(&serde_json::json!("value2"))
        );
    }

    #[test]
    fn config_builder_multiple_context_entries() {
        let cfg = config()
            .service_name("test")
            .context("key1", "value1")
            .context("key2", 42)
            .context("key3", true)
            .build();

        assert_eq!(cfg.service.context.len(), 3);
        assert_eq!(
            cfg.service.context.get("key1"),
            Some(&serde_json::json!("value1"))
        );
        assert_eq!(
            cfg.service.context.get("key2"),
            Some(&serde_json::json!(42))
        );
        assert_eq!(
            cfg.service.context.get("key3"),
            Some(&serde_json::json!(true))
        );
    }

    #[test]
    fn config_builder_custom_adapter() {
        // Test that custom adapter can be set
        struct CustomAdapter;
        impl LogAdapter for CustomAdapter {
            fn format(&self, _entry: &crate::types::ApmEntry) -> String {
                "custom".to_string()
            }
        }

        let cfg = config()
            .service_name("test")
            .adapter(Box::new(CustomAdapter))
            .build();

        // Verify adapter is used (we can't easily check the type, but we can verify it works)
        let entry = crate::types::ApmEntry::Transaction(crate::types::TransactionRecord {
            id: uuid::Uuid::new_v4(),
            trace_id: uuid::Uuid::new_v4(),
            name: "test".to_string(),
            transaction_type: "request".to_string(),
            start_time: chrono::Utc::now(),
            end_time: chrono::Utc::now(),
            duration_ms: 1.0,
            result: None,
            correlation_id: None,
            metadata: std::collections::HashMap::new(),
            service: std::sync::Arc::new(ServiceContext::default()),
        });
        assert_eq!(cfg.adapter.format(&entry), "custom");
    }
}
