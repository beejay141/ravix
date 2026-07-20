//! Rusta - A modern Rust API framework built on axum.
//!
//! Rusta provides clean-architecture patterns, proc-macro route registration,
//! and InversifyJS-style dependency injection for building production-ready
//! async web services.
//!
//! # Architecture
//!
//! Rusta follows a layered architecture pattern:
//! - **Controllers**: Handle HTTP requests/responses, extract parameters
//! - **Services**: Business logic, orchestrate operations
//! - **Repositories**: Data access layer, abstract persistence
//!
//! # Quick Example
//!
//! ```rust,ignore
//! use rusta::prelude::*;
//! use std::sync::Arc;
//!
//! #[controller("/users")]
//! impl UserController {
//!     #[get("/")]
//!     pub async fn list(&self) -> Response {
//!         Http::json(self.svc.find_all().await)
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut container = Container::new();
//!     container.register(UserService::construct(&container));
//!     App::new().container(container).run("0.0.0.0:3000", |res| match res {
//!         Ok(addr) => println!("Started on {}", addr),
//!         Err(msg) => eprintln!("Startup error: {}", msg),
//!     }).await;
//! }
//! ```
//!
//! # Features
//!
//! - Clean Architecture with Controller → Service → Repository layers
//! - Proc-macro routing with `#[controller]` and HTTP method attributes
//! - Dependency injection with `#[injectable]` and `#[inject]`
//! - Per-handler middleware with `#[middleware]` attribute
//! - Optional global middleware (CORS, logging, APM)
//! - Axum abstraction with clean type aliases

pub mod app;
pub mod error;
pub mod middleware;
pub mod request;
pub mod response;
pub mod router;

pub use app::App;
pub use error::ErrorResponse;
pub use middleware::MiddlewareChain;
pub use middleware::{CorsConfig, CorsConfigBuilder};
pub use response::{ErrorObject, Http};

// Re-export DI types from rusta-di
pub use rusta_di::{
    BindingCheck, Container, ContainerRef, DiError, Inject, Injectable, RouteDescriptor,
};

// Re-export proc-macros so users import everything from `rusta`.
pub use rusta_di_macros::{controller, delete, get, injectable, middleware, patch, post, put};

// Re-export inventory so the macro-generated `::rusta::inventory::submit!` works.
pub use rusta_di::inventory;

// Re-export frequently used axum types with cleaner names.
pub use axum::{
    body::Body,
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::from_fn as middleware_fn,
    middleware::Next,
    response::IntoResponse,
    response::Response,
};

/// JSON body extractor from axum.
pub use axum::extract::Json;

/// Extension extractor from axum.
pub use axum::extract::Extension;

/// Convenience prelude — glob-import this to use all HTTP method macros,
/// DI attributes, and common types without the `rusta::` prefix.
///
/// ```rust
/// use rusta::prelude::*;
/// ```
pub mod prelude {
    pub use crate::error::ErrorResponse;
    pub use crate::response::ErrorObject;
    pub use crate::response::Http;
    pub use crate::MiddlewareChain;
    pub use crate::{controller, delete, get, injectable, middleware, patch, post, put};
    pub use crate::{
        Body, Extension, IntoResponse, Json, Next, Path, Query, Request, Response, State,
        StatusCode,
    };
    pub use crate::{Container, ContainerRef, CorsConfig, CorsConfigBuilder, Inject, Injectable};
}
