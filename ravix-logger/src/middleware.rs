use std::sync::Arc;

use axum::extract::Request;
use axum::http::HeaderName;
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

use crate::context::{LogContext, CURRENT_LOG_CONTEXT};
use crate::Logger;

/// Axum middleware that injects and propagates a correlation ID across all
/// log entries within the request handler.
///
/// 1. Reads the configured correlation ID header from the incoming request.
/// 2. Generates a new UUID if the header is absent or empty.
/// 3. Runs the rest of the middleware stack inside a task-local scope so that
///    all [`Logger`] calls within the handler automatically carry the ID.
/// 4. Injects the correlation ID into the response headers.
pub async fn logger_middleware(request: Request, next: Next) -> Response {
    let header_name = Logger::correlation_id_header();

    let correlation_id: Arc<str> = request
        .headers()
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .map(|v| Arc::from(v))
        .unwrap_or_else(|| Arc::from(Uuid::new_v4().to_string()));

    let mut response = CURRENT_LOG_CONTEXT
        .scope(
            LogContext {
                correlation_id: Arc::clone(&correlation_id),
            },
            next.run(request),
        )
        .await;

    // Inject the correlation ID into the response.
    if let Ok(name) = header_name.parse::<HeaderName>() {
        if let Ok(value) = correlation_id.as_ref().parse() {
            response.headers_mut().insert(name, value);
        }
    }

    response
}