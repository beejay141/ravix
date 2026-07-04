use std::sync::Arc;

use ravix::prelude::*;
use serde_json::json;
use uuid::Uuid;

use crate::repositories::UserRepository;

/// Bearer-token auth guard for use with `#[middleware(auth_guard)]`.
///
/// Returns `401 Unauthorized` when the `Authorization` header is absent or
/// does not start with `"Bearer "`.
///
/// For production use, replace the presence check with real JWT validation.
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

/// Active-user guard for use with `#[middleware(active_user_guard)]`.
///
/// Resolves [`UserRepository`] from the DI container, looks up the user by
/// the `:id` path parameter, and rejects the request if the user account is
/// inactive (`is_active = false`).
///
/// - `403 Forbidden`  — user exists but is inactive
/// - `404 Not Found`  — no user with that id
pub async fn active_user_guard(
    State(container): State<ContainerRef>,
    Path(id): Path<Uuid>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let repo = container.resolve::<Arc<dyn UserRepository>>();

    match repo.find_by_id(id).await {
        Some(user) if user.is_active => next.run(request).await,
        Some(_) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Forbidden: user account is inactive" })),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "User not found" })),
        )
            .into_response(),
    }
}
