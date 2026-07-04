use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use http::HeaderValue;
use tower_http::cors::{Any, CorsLayer};

/// Configuration for CORS middleware.
///
/// This is a Ravix-friendly wrapper around `tower_http::cors::CorsLayer`
/// that allows users to configure CORS without reaching into tower-http directly.
///
/// # Example
/// ```no_run
/// use ravix::middleware::CorsConfig;
///
/// let cors = CorsConfig::builder()
///     .allow_origins(vec!["http://localhost:3000".to_string()])
///     .allow_methods(vec!["GET".to_string(), "POST".to_string()])
///     .allow_headers(vec!["Content-Type".to_string(), "Authorization".to_string()])
///     .max_age(3600)
///     .build();
/// ```
#[derive(Clone)]
pub struct CorsConfig {
    allowed_origins: Vec<String>,
    allowed_methods: Vec<String>,
    allowed_headers: Vec<String>,
    exposed_headers: Vec<String>,
    max_age: Option<u64>,
    allow_credentials: bool,
}

impl CorsConfig {
    /// Create a new CORS configuration builder.
    pub fn builder() -> CorsConfigBuilder {
        CorsConfigBuilder::default()
    }

    /// Convert this configuration into a `CorsLayer` for use with axum.
    pub fn into_layer(self) -> CorsLayer {
        let mut layer = CorsLayer::new();

        if !self.allowed_origins.is_empty() {
            let origins: Vec<HeaderValue> = self
                .allowed_origins
                .into_iter()
                .filter_map(|o| o.parse().ok())
                .collect();
            layer = layer.allow_origin(origins);
        } else {
            layer = layer.allow_origin(Any);
        }

        if !self.allowed_methods.is_empty() {
            let methods: Vec<axum::http::Method> = self
                .allowed_methods
                .into_iter()
                .filter_map(|m| m.parse().ok())
                .collect();
            layer = layer.allow_methods(methods);
        } else if self.allow_credentials {
            // Wildcard methods are forbidden when credentials are enabled.
            // Default to a safe, standard set.
            use axum::http::Method;
            layer = layer.allow_methods(vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
                Method::HEAD,
                Method::OPTIONS,
            ]);
        } else {
            layer = layer.allow_methods(Any);
        }

        if !self.allowed_headers.is_empty() {
            let headers: Vec<http::HeaderName> = self
                .allowed_headers
                .into_iter()
                .filter_map(|h| h.parse().ok())
                .collect();
            layer = layer.allow_headers(headers);
        } else if self.allow_credentials {
            // Wildcard headers are forbidden when credentials are enabled.
            // Default to the headers most APIs need.
            layer = layer.allow_headers(vec![
                http::header::CONTENT_TYPE,
                http::header::AUTHORIZATION,
                http::header::ACCEPT,
            ]);
        } else {
            layer = layer.allow_headers(Any);
        }

        if !self.exposed_headers.is_empty() {
            let headers: Vec<http::HeaderName> = self
                .exposed_headers
                .into_iter()
                .filter_map(|h| h.parse().ok())
                .collect();
            layer = layer.expose_headers(headers);
        }

        if let Some(age) = self.max_age {
            layer = layer.max_age(Duration::from_secs(age));
        }

        if self.allow_credentials {
            layer = layer.allow_credentials(true);
        }

        layer
    }
}

/// Builder for `CorsConfig`.
#[derive(Default)]
pub struct CorsConfigBuilder {
    allowed_origins: Vec<String>,
    allowed_methods: Vec<String>,
    allowed_headers: Vec<String>,
    exposed_headers: Vec<String>,
    max_age: Option<u64>,
    allow_credentials: bool,
}

impl CorsConfigBuilder {
    /// Set allowed origins.
    pub fn allow_origins(mut self, origins: Vec<String>) -> Self {
        self.allowed_origins = origins;
        self
    }

    /// Set allowed HTTP methods.
    pub fn allow_methods(mut self, methods: Vec<String>) -> Self {
        self.allowed_methods = methods;
        self
    }

    /// Set allowed request headers.
    pub fn allow_headers(mut self, headers: Vec<String>) -> Self {
        self.allowed_headers = headers;
        self
    }

    /// Set exposed response headers.
    pub fn expose_headers(mut self, headers: Vec<String>) -> Self {
        self.exposed_headers = headers;
        self
    }

    /// Set max age for preflight cache in seconds.
    pub fn max_age(mut self, seconds: u64) -> Self {
        self.max_age = Some(seconds);
        self
    }

    /// Allow credentials (cookies, authorization headers, TLS client certificates).
    pub fn allow_credentials(mut self) -> Self {
        self.allow_credentials = true;
        self
    }

    /// Build the final `CorsConfig`.
    pub fn build(self) -> CorsConfig {
        CorsConfig {
            allowed_origins: self.allowed_origins,
            allowed_methods: self.allowed_methods,
            allowed_headers: self.allowed_headers,
            exposed_headers: self.exposed_headers,
            max_age: self.max_age,
            allow_credentials: self.allow_credentials,
        }
    }
}

/// Apply CORS layer to a router if configured.
pub fn apply_cors(router: Router, cors: Option<Arc<CorsConfig>>) -> Router {
    match cors {
        Some(config) => router.layer((*config).clone().into_layer()),
        None => router,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    fn test_router() -> Router {
        Router::new().route("/", get(|| async { "ok" }))
    }

    #[tokio::test]
    async fn cors_preflight_returns_access_control_headers() {
        let cors = CorsConfig::builder()
            .allow_origins(vec!["http://localhost:3000".to_string()])
            .allow_methods(vec!["GET".to_string(), "POST".to_string()])
            .build();

        let router = apply_cors(test_router(), Some(Arc::new(cors)));

        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/")
            .header("Origin", "http://localhost:3000")
            .header("Access-Control-Request-Method", "GET")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        let headers = response.headers();

        assert!(headers.contains_key("access-control-allow-origin"));
        assert!(headers.contains_key("access-control-allow-methods"));
    }

    #[tokio::test]
    async fn cors_matching_origin_returns_headers() {
        let cors = CorsConfig::builder()
            .allow_origins(vec!["http://localhost:3000".to_string()])
            .allow_methods(vec!["GET".to_string(), "POST".to_string()])
            .build();

        let router = apply_cors(test_router(), Some(Arc::new(cors)));

        let request = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header("Origin", "http://localhost:3000")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        let headers = response.headers();

        assert_eq!(
            headers.get("access-control-allow-origin").unwrap(),
            "http://localhost:3000"
        );
    }

    #[tokio::test]
    async fn cors_disallowed_origin_no_headers() {
        let cors = CorsConfig::builder()
            .allow_origins(vec!["http://localhost:3000".to_string()])
            .build();

        let router = apply_cors(test_router(), Some(Arc::new(cors)));

        let request = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header("Origin", "http://evil.com")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        let headers = response.headers();

        assert!(!headers.contains_key("access-control-allow-origin"));
    }

    #[tokio::test]
    async fn cors_disabled_no_headers() {
        let router = apply_cors(test_router(), None);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header("Origin", "http://localhost:3000")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        let headers = response.headers();

        assert!(!headers.contains_key("access-control-allow-origin"));
    }

    #[tokio::test]
    async fn cors_with_credentials() {
        // tower-http forbids wildcard methods or headers when credentials=true;
        // explicit values are required on both.
        let cors = CorsConfig::builder()
            .allow_origins(vec!["http://localhost:3000".to_string()])
            .allow_methods(vec!["GET".to_string(), "POST".to_string()])
            .allow_headers(vec![
                "content-type".to_string(),
                "authorization".to_string(),
            ])
            .allow_credentials()
            .build();

        let router = apply_cors(test_router(), Some(Arc::new(cors)));

        let request = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header("Origin", "http://localhost:3000")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        let headers = response.headers();

        assert_eq!(
            headers.get("access-control-allow-origin").unwrap(),
            "http://localhost:3000"
        );
        assert_eq!(
            headers.get("access-control-allow-credentials").unwrap(),
            "true"
        );
    }

    #[tokio::test]
    async fn cors_max_age_header() {
        let cors = CorsConfig::builder()
            .allow_origins(vec!["http://localhost:3000".to_string()])
            .max_age(3600)
            .build();

        let router = apply_cors(test_router(), Some(Arc::new(cors)));

        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/")
            .header("Origin", "http://localhost:3000")
            .header("Access-Control-Request-Method", "GET")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        let headers = response.headers();

        assert!(headers.contains_key("access-control-max-age"));
    }
}
