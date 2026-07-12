# Ravix

A modern Rust API framework built on [axum](https://github.com/tokio-rs/axum) with clean-architecture patterns, proc-macro route registration, and dependency injection.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-blue.svg)](https://www.rust-lang.org)

## Features

- **Clean Architecture**: Controller → Service → Repository layers with clear separation of concerns
- **Proc-macro Routing**: Declarative route registration with `#[controller]` and HTTP method attributes
- **Dependency Injection**: DI container with `#[injectable]` and `#[inject]` attributes
- **Per-handler Middleware**: Attach middleware to individual handlers with `#[middleware]` attribute
- **Optional Global Middleware**: Register CORS and other security middleware at the `App` level — each feature is opt-in
- **Axum Abstraction**: Clean type aliases for common axum types (no direct `axum::` imports needed)
- **Async-First**: Built for async/await with Tokio runtime

## Quick Start

```toml
# Cargo.toml
[dependencies]
ravix = { path = "ravix" }
```

```rust
use ravix::prelude::*;
use std::sync::Arc;

// Define your controller
#[controller("/users")]
impl UserController {
    #[get("/")]
    pub async fn list_users(&self) -> Response {
        Http::json(self.svc.find_all().await)
    }

    #[get("/:id")]
    pub async fn get_user(&self, Path(id): Path<Uuid>) -> Response {
        match self.svc.find_by_id(id).await {
            Some(user) => Http::json(user),
            None => Http::not_found("User not found"),
        }
    }

    #[post("/")]
    pub async fn create_user(&self, Json(body): Json<CreateUserDto>) -> Response {
        Http::created(self.svc.create(body).await)
    }
}

// Bootstrap
#[tokio::main]
async fn main() {
    let mut container = Container::new();
    // Register dependencies...
    App::new().container(container).run("0.0.0.0:3000").await;
}
```

## Architecture

```
ravix/
├── ravix/           # Framework core (re-exports from ravix-di)
├── ravix-di/        # Dependency injection library
│   ├── container.rs # DI container, Injectable trait, Inject extractor
│   ├── handler.rs   # Route descriptor
│   └── error.rs     # DiError type
├── ravix-di-macros/ # Proc-macros for DI
│   ├── injectable.rs # #[injectable] attribute
│   └── controller.rs # #[controller], #[get], etc.
├── ravix-apm/       # APM (Application Performance Monitoring)
├── ravix-logger/    # Structured logging
└── ravix-example/   # Reference application
```

## API Reference

### Route Macros

| Macro                    | Description                                        |
| ------------------------ | -------------------------------------------------- |
| `#[controller("/base")]` | Marks an impl block as a controller with base path |
| `#[get("/path")]`        | HTTP GET route                                     |
| `#[post("/path")]`       | HTTP POST route                                    |
| `#[put("/path")]`        | HTTP PUT route                                     |
| `#[delete("/path")]`     | HTTP DELETE route                                  |
| `#[patch("/path")]`      | HTTP PATCH route                                   |
| `#[middleware(Guard)]`   | Attach middleware to a handler                     |

### Response Helpers

```rust
Http::json(data)                     // 200 OK with JSON body
Http::created(data)                  // 201 Created with JSON body
Http::ok()                           // 200 OK with empty body
Http::no_content()                   // 204 No Content with empty body
Http::status(code)                   // Arbitrary status code with empty body
Http::with_status(code, body)         // Arbitrary status code with JSON body
Http::error(status, error)           // Flexible error with any status code
Http::not_found("msg")               // 404 Not Found with error JSON
Http::not_found_with(obj)            // 404 Not Found with structured error
Http::unauthorized("msg")              // 401 Unauthorized with error JSON
Http::unauthorized_with(obj)         // 401 Unauthorized with structured error
Http::bad_request("msg")               // 400 Bad Request with error JSON
Http::bad_request_with(obj)          // 400 Bad Request with structured error
Http::forbidden("msg")                // 403 Forbidden with error JSON
Http::forbidden_with(obj)            // 403 Forbidden with structured error
Http::internal_error("msg")           // 500 Internal Server Error with error JSON
Http::internal_error_with(obj)        // 500 Internal Error with structured error
```

### Error Handling

For structured error responses, use `ErrorObject<T>` or `ErrorResponse::object()`:

```rust
use ravix::{Http, ErrorObject, ErrorResponse};

// Structured error with custom fields
#[derive(serde::Serialize)]
struct ValidationError {
    code: &'static str,
    fields: Vec<&'static str>,
}

// In handler:
if let Err(e) = validator::Validate::validate(&body) {
    return Http::bad_request(ErrorObject(ValidationError {
        code: "VALIDATION_ERROR",
        fields: e.field_errors().into_iter().map(|f| f.0).collect(),
    }));
}
```

### Global Middleware Pipeline

Use `MiddlewareChain` to register middleware that applies to all routes:

```rust
use ravix::{App, Container, MiddlewareChain, Request, Next, Response};

// Custom middleware function
async fn request_id_middleware(request: Request, next: Next) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    request.extensions_mut().insert(request_id.clone());
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "X-Request-ID",
        request_id.parse().unwrap()
    );
    response
}

// Tower layer (e.g., compression)
use tower_http::compression::CompressionLayer;

let middleware = MiddlewareChain::new()
    .chain(request_id_middleware)
    .add_layer(CompressionLayer::new());

App::new()
    .container(container)
    .middleware(middleware)
    .run("0.0.0.0:3000")
    .await;
```

**Middleware ordering**: Layers are applied in registration order. The first added layer
runs closest to the handler (innermost), the last added layer wraps everything (outermost).

### Advanced Dependency Injection

```rust
use ravix::prelude::*;
use std::sync::Arc;

// Optional injection - field becomes None if not registered
#[injectable]
pub struct EmailService {
    #[inject(optional)]
    pub smtp_client: Option<Arc<dyn SmtpClient>>,
}

// Named bindings - multiple implementations of the same trait
#[injectable]
pub struct CacheService {
    #[inject(name = "redis")]
    pub primary: Arc<dyn Cache>,
    #[inject(name = "memory")]
    pub fallback: Arc<dyn Cache>,
}

// Registration
let mut container = Container::new();
container.register_named(Arc::new(RedisCache::new()), "redis");
container.register_named(Arc::new(MemoryCache::new()), "memory");

// Verify all required bindings at startup
let errors = container.verify();
if !errors.is_empty() {
    panic!("Missing bindings: {:?}", errors);
}
```

### Dependency Injection

```rust
// Mark a struct as injectable
#[injectable]
pub struct UserService {
    #[inject]
    pub repo: Arc<dyn UserRepository>,
}

// Register in container
let mut container = Container::new();
container.register(Arc::new(InMemoryUserRepository::default()));
container.register(UserService::construct(&container));

// Resolve in handlers
pub async fn get_user(Inject(svc): Inject<Arc<UserService>>) -> Response {
    // ...
}
```

### Prelude Exports

The `ravix::prelude::*` glob import provides:

```rust
// HTTP method macros
controller, get, post, put, delete, patch, middleware, injectable

// DI types
Container, ContainerRef, Inject, Injectable

// Framework types
Http, Json, Path, Query, State, Body, Request, Response, IntoResponse, StatusCode
```

### CORS

CORS is opt-in and registered globally on the `App` builder. It is applied to every route, including `OPTIONS` preflight requests, before any per-handler middleware runs.

```rust
use ravix::{App, Container, CorsConfig};

let cors = CorsConfig::builder()
    .allow_origins(vec!["https://app.example.com".to_string()])
    .allow_methods(vec!["GET".to_string(), "POST".to_string(), "PUT".to_string(), "DELETE".to_string()])
    .allow_headers(vec!["content-type".to_string(), "authorization".to_string()])
    .max_age(3600)   // seconds to cache preflight response
    .build();

App::new()
    .container(container)
    .cors(cors)      // omit to disable CORS entirely
    .run("0.0.0.0:3000")
    .await;
```

**`CorsConfig` builder options:**

| Method                       | Description                                               |
| ---------------------------- | --------------------------------------------------------- |
| `.allow_origins(vec![...])`  | Explicit allowed origins. Omit to allow any origin (`*`). |
| `.allow_methods(vec![...])`  | Allowed HTTP methods. Omit to allow any.                  |
| `.allow_headers(vec![...])`  | Allowed request headers. Omit to allow any.               |
| `.expose_headers(vec![...])` | Response headers exposed to the browser.                  |
| `.max_age(seconds)`          | How long browsers may cache the preflight response.       |
| `.allow_credentials()`       | Enable `Access-Control-Allow-Credentials: true`.          |

> **Security note**: When `allow_credentials()` is enabled you **must** specify explicit `allow_origins()` and `allow_headers()`. Combining credentials with a wildcard (`*`) is invalid per the CORS specification and will panic at startup. If you use cookie-based auth alongside credentials, also add CSRF protections.

## APM (Application Performance Monitoring)

Ravix includes an APM module for distributed tracing and performance monitoring.

```toml
# Cargo.toml
[dependencies]
ravix-apm = { path = "ravix-apm" }
```

### Configuration

```rust
use ravix_apm::{Apm, config};

let apm = Apm::configure(
    config()
        .service_name("my-service")
        .service_version("1.0.0")
        .environment("production")
        .log_path("apm.ndjson")
        .correlation_id_header("X-Correlation-ID")
        .build(),
).await;
```

### Usage

```rust
use ravix_apm::Apm;

// Automatic transaction wrapping (via middleware)
// Each request creates a transaction automatically

// Manual span creation
let span = Apm::start_span("database.query", "db", Some(metadata));
// ... do work ...
span.end(None, None);

// Wrap futures with spans
let result = Apm::wrap_span_future(
    "external.api_call",
    "http",
    Some(metadata),
    async_operation()
).await;
```

### Transaction and Span Records

- **Transactions**: Represent entire requests (e.g., `GET /users`)
- **Spans**: Represent operations within a transaction (e.g., database queries, HTTP calls)
- Both record timing, metadata, and correlation IDs

## Logger

Structured logging with classification-based routing.

```toml
# Cargo.toml
[dependencies]
ravix-logger = { path = "ravix-logger" }
```

### Configuration

```rust
use ravix_logger::{Logger, config};

let logger = Logger::configure(
    config()
        .service_name("my-service")
        .service_version("1.0.0")
        .environment("production")
        .min_level(LogLevel::Info)
        .add_classification("PUBLIC", "public.ndjson")
        .add_classification("CONFIDENTIAL", "confidential.ndjson")
        .correlation_id_header("X-Correlation-ID")
        .build(),
).await;
```

### Usage

```rust
use ravix_logger::{Logger, LogLevel, LogOptions};

// Simple logging
Logger::info("User logged in", LogOptions::default());

// With context
Logger::info(
    "Order created",
    LogOptions {
        context: Some([("order_id", json!(order.id))].into()),
        ..Default::default()
    }
);

// Classification-based logging
Logger::log(
    LogLevel::Info,
    "Sensitive operation",
    LogOptions {
        classification: Some("CONFIDENTIAL".to_string()),
        ..Default::default()
    }
);
```

## Development

```bash
# Check all crates
cargo check --workspace

# Run tests
cargo test --workspace

# Run the example
cargo run -p ravix-example
```

## License

MIT
