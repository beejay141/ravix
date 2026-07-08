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