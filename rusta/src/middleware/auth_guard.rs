use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

/// Reference `AuthGuard` middleware function for use with
/// `#[middleware(auth_guard)]` or `axum::middleware::from_fn`.
///
/// Checks for an `Authorization: Bearer <token>` header and returns
/// `401 Unauthorized` when it is absent or malformed.
///
/// In production, replace the presence-only check with real JWT validation.
pub async fn auth_guard(request: Request<Body>, next: Next) -> Response {
    let has_bearer = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("Bearer "))
        .unwrap_or(false);

    if has_bearer {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Unauthorized: missing or invalid Bearer token" })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use tower::ServiceExt;

    #[tokio::test]
    async fn auth_guard_allows_bearer_token() {
        let router = axum::Router::new()
            .route("/", axum::routing::get(|| async {}))
            .layer(axum::middleware::from_fn(auth_guard));
        let req = Request::builder()
            .header("Authorization", "Bearer abc123")
            .uri("/")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_guard_rejects_missing_header() {
        let router = axum::Router::new()
            .route("/", axum::routing::get(|| async {}))
            .layer(axum::middleware::from_fn(auth_guard));
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_guard_rejects_non_bearer() {
        let router = axum::Router::new()
            .route("/", axum::routing::get(|| async {}))
            .layer(axum::middleware::from_fn(auth_guard));
        let req = Request::builder()
            .header("Authorization", "Basic abc123")
            .uri("/")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_guard_rejects_malformed_bearer() {
        let router = axum::Router::new()
            .route("/", axum::routing::get(|| async {}))
            .layer(axum::middleware::from_fn(auth_guard));
        let req = Request::builder()
            .header("Authorization", "Bearer")
            .uri("/")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
