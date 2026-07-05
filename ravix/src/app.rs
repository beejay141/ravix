use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;

use crate::{
    container::ContainerRef, middleware::CorsConfig, middleware::MiddlewareChain,
    router::RouterBuilder,
};

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
    middleware: Option<MiddlewareChain>,
    base_path: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            container: None,
            cors: None,
            middleware: None,
            base_path: None,
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

    /// Attach a global middleware pipeline.
    ///
    /// Layers are applied in registration order: the first added layer runs
    /// closest to the handler (innermost), the last added layer wraps
    /// everything (outermost).
    ///
    /// # Example
    /// ```no_run
    /// # use ravix::{App, Container, CorsConfig, MiddlewareChain};
    /// # use ravix::{Request, Next, Response};
    /// # #[tokio::main]
    /// # async fn main() {
    /// async fn my_mw(
    ///     request: Request,
    ///     next: Next,
    /// ) -> Response {
    ///     next.run(request).await
    /// }
    ///
    /// let chain = MiddlewareChain::new()
    ///     .chain(my_mw);
    /// let mut container = Container::new();
    /// App::new()
    ///     .container(container)
    ///     .middleware(chain)
    ///     .run("0.0.0.0:3000")
    ///     .await;
    /// # }
    /// ```
    pub fn middleware(mut self, chain: MiddlewareChain) -> Self {
        self.middleware = Some(chain);
        self
    }

    /// Set a global prefix for all routes (e.g. `"/api/v1"`).
    ///
    /// The prefix is prepended before each controller's base path.
    ///
    /// # Example
    /// ```no_run
    /// # use ravix::{App, Container};
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut container = Container::new();
    /// App::new()
    ///     .container(container)
    ///     .base_path("/my_service/v1")
    ///     .run("0.0.0.0:3000")
    ///     .await;
    /// # }
    /// ```
    pub fn base_path(mut self, path: impl Into<String>) -> Self {
        self.base_path = Some(path.into());
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
            panic!("[ravix] Missing DI bindings:\n{}", errors.join("\n"));
        }
        let router = RouterBuilder::build_with_cors(container, self.cors, self.base_path);
        match self.middleware {
            Some(chain) => chain.apply(router),
            None => router,
        }
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
