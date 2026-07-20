use std::convert::Infallible;
use std::future::Future;

use axum::middleware::Next;
use axum::response::Response;
use axum::routing::Route;
use axum::{body::Body, extract::Request, response::IntoResponse, Router};
use tower::Layer;
use tower::Service;

/// A chain of middleware applied globally to every request.
///
/// Layers are applied in registration order:
/// the **first** added layer ends up **innermost** (closest to the handler),
/// the **last** added layer ends up **outermost**.
/// This matches tower convention where the last `.layer()` call is outermost.
///
/// # Architecture
///
/// - Uses type-erased layers for flexibility
/// - Supports both function middleware and tower layers
/// - Lock-free after construction
/// - Consumed once during `App::build()`
///
/// # Example
/// ```ignore
/// use rusta::MiddlewareChain;
/// use tower_http::trace::TraceLayer;
///
/// let chain = MiddlewareChain::new()
///     .add_layer(TraceLayer::new_for_http());
/// ```
pub struct MiddlewareChain {
    layers: Vec<Box<dyn Fn(Router) -> Router + Send + Sync>>,
}

impl MiddlewareChain {
    /// Create an empty middleware chain.
    ///
    /// Use [`Self::chain`] to add function middleware or
    /// [`Self::add_layer`] to add tower layers.
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Register a middleware handler function.
    ///
    /// The function must match the signature `async fn(Request, Next) -> Response`.
    /// It receives the incoming request and a `Next` handle to call the next
    /// layer in the stack (or the handler itself if this is the innermost layer).
    ///
    /// Under the hood this wraps the function with axum's `from_fn`, but the user
    /// never needs to import or reference axum directly.
    ///
    /// # Example
    /// ```ignore
    /// use rusta::{MiddlewareChain, Request, Next, Response};
    ///
    /// async fn auth_mw(request: Request, next: Next) -> Response {
    ///     // Perform auth check here…
    ///     next.run(request).await
    /// }
    ///
    /// let chain = MiddlewareChain::new().chain(auth_mw);
    /// ```
    pub fn chain<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(Request, Next) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.layers.push(Box::new(move |router: Router| {
            router.layer(axum::middleware::from_fn(f.clone()))
        }));

        self
    }

    /// Add an external tower `Layer` (e.g. `TraceLayer`, `CompressionLayer`,
    /// `TimeoutLayer` from `tower-http`).
    ///
    /// The trait bounds match axum's [`Router::layer`] requirements: the layer
    /// service's `Response` must implement [`IntoResponse`] and its `Error` must
    /// be convertible to [`Infallible`].
    ///
    /// # Example
    /// ```ignore
    /// use rusta::MiddlewareChain;
    /// use tower_http::compression::CompressionLayer;
    ///
    /// let chain = MiddlewareChain::new()
    ///     .add_layer(CompressionLayer::new());
    /// ```
    pub fn add_layer<L>(mut self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request<Body>> + Clone + Send + 'static,
        <L::Service as Service<Request<Body>>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request<Body>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request<Body>>>::Future: Send + 'static,
    {
        self.layers
            .push(Box::new(move |router: Router| router.layer(layer.clone())));
        self
    }

    /// Consume the chain and apply all stored layers to a router.
    ///
    /// The chain is consumed — call this once per `App::build()`.
    /// Layers are applied in registration order (first = innermost).
    pub fn apply(self, router: Router) -> Router {
        self.layers.into_iter().fold(router, |r, f| f(r))
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    async fn dummy_mw(_req: Request, _next: Next) -> Response {
        axum::http::StatusCode::OK.into_response()
    }

    #[test]
    fn chain_adds_middleware() {
        let chain = MiddlewareChain::new().chain(dummy_mw);
        assert_eq!(chain.layers.len(), 1);
    }

    #[test]
    fn chain_multiple_middleware() {
        let chain = MiddlewareChain::new().chain(dummy_mw).chain(dummy_mw);
        assert_eq!(chain.layers.len(), 2);
    }

    #[test]
    fn default_chain_is_empty() {
        let chain = MiddlewareChain::default();
        assert!(chain.layers.is_empty());
    }

    #[tokio::test]
    async fn apply_with_empty_chain_returns_router() {
        let chain = MiddlewareChain::new();
        let router = chain.apply(Router::new());
        // Just verify it doesn't panic
        let _ = router;
    }
}
