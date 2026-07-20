use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;

use crate::{middleware::CorsConfig, middleware::MiddlewareChain, router::RouterBuilder};
use rusta_di::ContainerRef;

/// Top-level application builder.
///
/// # Example
/// ```no_run
/// # use rusta::{App, Container};
/// #[tokio::main]
/// async fn main() {
///     let mut container = Container::new();
///     // ... register services ...
///     App::new()
///         .container(container)
///         .run("0.0.0.0:3000", |res| match res {
///             Ok(addr) => println!("Listening on {}", addr),
///             Err(msg) => eprintln!("Startup error: {}", msg),
///         })
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
    pub fn container(mut self, container: rusta_di::Container) -> Self {
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
    /// # use rusta::{App, Container, CorsConfig};
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
    ///     .run("0.0.0.0:3000", |res| match res {
    ///         Ok(addr) => println!("Listening on {}", addr),
    ///         Err(msg) => eprintln!("Startup error: {}", msg),
    ///     })
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
    /// # use rusta::{App, Container, CorsConfig, MiddlewareChain};
    /// # use rusta::{Request, Next, Response};
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
    ///     .run("0.0.0.0:3000", |res| match res {
    ///         Ok(addr) => println!("Listening on {}", addr),
    ///         Err(msg) => eprintln!("Startup error: {}", msg),
    ///     })
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
    /// # use rusta::{App, Container};
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut container = Container::new();
    /// App::new()
    ///     .container(container)
    ///     .base_path("/my_service/v1")
    ///     .run("0.0.0.0:3000", |res| match res {
    ///         Ok(addr) => println!("Listening on {}", addr),
    ///         Err(msg) => eprintln!("Startup error: {}", msg),
    ///     })
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
            .expect("[rusta] No container set. Call App::new().container(c) before build().");
        let errors = container.verify();
        if !errors.is_empty() {
            panic!("[rusta] Missing DI bindings:\n{}", errors.join("\n"));
        }
        let router = RouterBuilder::build_with_cors(container, self.cors, self.base_path);
        match self.middleware {
            Some(chain) => chain.apply(router),
            None => router,
        }
    }

    /// Start the HTTP server on `addr` (e.g. `"0.0.0.0:3000"`).
    ///
    /// The `result_cb` closure will be invoked with `Ok(addr)` once the
    /// server has successfully bound and begun listening, or with `Err(msg)`
    /// when parsing/binding/serving fails. The closure is called in all
    /// failure cases and on successful start.
    pub async fn run<F>(self, addr: &str, result_cb: F)
    where
        F: Fn(Result<SocketAddr, String>) + Send + Sync + 'static,
    {
        // wrap user-provided closure in Arc so we can call it multiple times
        let result_cb = Arc::new(result_cb);

        let router = self.build();

        let addr: SocketAddr = match addr.parse() {
            Ok(a) => a,
            Err(e) => {
                let msg = format!("Invalid socket address: {}", e);
                (result_cb)(Err(msg.clone()));
                panic!("[rusta] Invalid socket address: {}", addr);
            }
        };

        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                let msg = format!("Cannot bind to {}: {}", addr, e);
                (result_cb)(Err(msg.clone()));
                panic!("[rusta] Cannot bind to {}: {}", addr, e);
            }
        };

        println!("[rusta] Listening on http://{}", addr);
        (result_cb)(Ok(addr));

        if let Err(e) = axum::serve(listener, router).await {
            let msg = format!("Server error: {}", e);
            (result_cb)(Err(msg.clone()));
            panic!("[rusta] Server error: {}", e);
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusta_di::Container;
    use std::sync::Arc;

    fn make_container() -> Container {
        let mut c = Container::new();
        c.register(Arc::new(42_i32));
        c
    }

    #[test]
    fn app_new_has_no_container() {
        let app = App::new();
        assert!(app.container.is_none());
    }

    #[test]
    fn app_container_sets_container() {
        let container = make_container();
        let app = App::new().container(container);
        assert!(app.container.is_some());
    }

    #[test]
    fn app_container_ref_sets_container() {
        let container = make_container();
        let app = App::new().container_ref(Arc::new(container));
        assert!(app.container.is_some());
    }

    #[test]
    fn app_cors_sets_cors() {
        let cors = crate::middleware::CorsConfig::builder()
            .allow_origins(vec!["http://localhost".to_string()])
            .build();
        let app = App::new().cors(cors);
        assert!(app.cors.is_some());
    }

    #[test]
    fn app_middleware_sets_middleware() {
        use crate::middleware::MiddlewareChain;
        async fn dummy(_req: crate::Request, _next: crate::Next) -> crate::Response {
            crate::Response::default()
        }
        let chain = MiddlewareChain::new().chain(dummy);
        let app = App::new().middleware(chain);
        assert!(app.middleware.is_some());
    }

    #[test]
    fn app_base_path_sets_path() {
        let app = App::new().base_path("/api/v1");
        assert_eq!(app.base_path, Some("/api/v1".to_string()));
    }

    #[test]
    fn app_build_panics_without_container() {
        let result = std::panic::catch_unwind(|| App::new().build());
        assert!(result.is_err());
    }

    #[test]
    fn app_build_succeeds_with_container() {
        let container = make_container();
        let _router = App::new().container(container).build();
    }
}
