use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;

use crate::{container::ContainerRef, middleware::CorsConfig, router::RouterBuilder};

/// Top-level application builder.
///
/// # Example
/// ```no_run
/// # use ravix::{App, Container};
/// #[tokio::main]
/// async fn main() {
///     let mut container = Container::new();
///     // ... register services ...
///     App::new()
///         .container(container)
///         .run("0.0.0.0:3000")
///         .await;
/// }
/// ```
pub struct App {
    container: Option<ContainerRef>,
    cors: Option<Arc<CorsConfig>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            container: None,
            cors: None,
        }
    }

    /// Wrap a [`Container`] in an `Arc` and attach it to the application.
    pub fn container(mut self, container: crate::container::Container) -> Self {
        self.container = Some(std::sync::Arc::new(container));
        self
    }

    /// Attach an already-wrapped [`ContainerRef`] to the application.
    pub fn container_ref(mut self, container: ContainerRef) -> Self {
        self.container = Some(container);
        self
    }

    /// Configure CORS middleware for the application.
    ///
    /// This is optional. If not called, no CORS middleware will be applied.
    ///
    /// # Example
    /// ```no_run
    /// # use ravix::{App, Container, CorsConfig};
    /// # #[tokio::main]
    /// # async fn main() {
    /// let cors = CorsConfig::builder()
    ///     .allow_origins(vec!["http://localhost:3000".to_string()])
    ///     .allow_methods(vec!["GET".to_string(), "POST".to_string()])
    ///     .build();
    /// let mut container = Container::new();
    /// App::new()
    ///     .container(container)
    ///     .cors(cors)
    ///     .run("0.0.0.0:3000")
    ///     .await;
    /// # }
    /// ```
    pub fn cors(mut self, cors: CorsConfig) -> Self {
        self.cors = Some(Arc::new(cors));
        self
    }

    /// Build the `axum::Router` without starting a server.
    ///
    /// Useful for integration testing with `tower::ServiceExt::oneshot`.
    ///
    /// # Panics
    /// Panics if any required DI bindings are missing.  See
    /// [`Container::verify`] for details.
    pub fn build(self) -> axum::Router {
        let container = self
            .container
            .expect("[ravix] No container set. Call App::new().container(c) before build().");
        let errors = container.verify();
        if !errors.is_empty() {
            panic!(
                "[ravix] Missing DI bindings:\n{}",
                errors.join("\n")
            );
        }
        RouterBuilder::build_with_cors(container, self.cors)
    }

    /// Start the HTTP server on `addr` (e.g. `"0.0.0.0:3000"`).
    pub async fn run(self, addr: &str) {
        let router = self.build();
        let addr: SocketAddr = addr
            .parse()
            .unwrap_or_else(|_| panic!("[ravix] Invalid socket address: {}", addr));
        let listener = TcpListener::bind(addr)
            .await
            .unwrap_or_else(|e| panic!("[ravix] Cannot bind to {}: {}", addr, e));
        println!("[ravix] Listening on http://{}", addr);
        axum::serve(listener, router)
            .await
            .unwrap_or_else(|e| panic!("[ravix] Server error: {}", e));
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
