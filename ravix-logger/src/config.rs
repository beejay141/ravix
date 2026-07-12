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
    pub adapter: Box<dyn LogAdapter + Send + Sync>,
}

/// Fluent builder for [`LoggerConfig`].
pub struct LoggerConfigBuilder {
    service: ServiceContext,
    min_level: LogLevel,
    classifications: Vec<LogClassificationConfig>,
    default_classification: Option<String>,
    correlation_id_header: Option<String>,
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

    pub fn adapter(mut self, adapter: Box<dyn LogAdapter + Send + Sync>) -> Self {
        self.adapter = Some(adapter);
        self
    }

    pub fn build(self) -> LoggerConfig {
        let default_classification = self.default_classification.unwrap_or_else(|| "PUBLIC".to_string());

        assert!(
            !self.service.service_name.is_empty(),
            "ravix-logger: service_name must be set"
        );
        assert!(
            !self.classifications.is_empty(),
            "ravix-logger: at least one classification must be added"
        );

        let valid = self
            .classifications
            .iter()
            .any(|c| c.name == default_classification);
        assert!(
            valid,
            "ravix-logger: default_classification '{}' does not match any configured classification",
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
            adapter: self.adapter.unwrap_or_else(|| Box::new(DefaultJsonAdapter)),
        }
    }
}

/// Create a new [`LoggerConfigBuilder`].
pub fn config() -> LoggerConfigBuilder {
    LoggerConfigBuilder::new()
}