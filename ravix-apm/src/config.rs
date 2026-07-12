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
}

/// Fluent builder for [`ApmConfig`].
pub struct ApmConfigBuilder {
    service: ServiceContext,
    log_path: Option<PathBuf>,
    adapter: Option<Box<dyn LogAdapter + Send + Sync>>,
    correlation_id_header: Option<String>,
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

    pub fn build(self) -> ApmConfig {
        ApmConfig {
            service: Arc::new(self.service),
            log_path: self
                .log_path
                .unwrap_or_else(|| PathBuf::from("apm.ndjson")),
            adapter: self.adapter.unwrap_or_else(|| Box::new(DefaultJsonAdapter)),
            correlation_id_header: self.correlation_id_header,
        }
    }
}

/// Create a new [`ApmConfigBuilder`].
pub fn config() -> ApmConfigBuilder {
    ApmConfigBuilder::new()
}