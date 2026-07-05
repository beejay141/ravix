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
