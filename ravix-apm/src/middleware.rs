use std::cell::Cell;
use std::panic::AssertUnwindSafe;

use futures::FutureExt;
use uuid::Uuid;

use axum::extract::Request;
use axum::http::HeaderName;
use axum::middleware::Next;
use axum::response::Response;

use crate::context::{CURRENT_TXN, CURRENT_SPAN_ID};
use crate::Apm;

/// Axum middleware that wraps every request inside an APM transaction.
///
/// The transaction name is `{METHOD} {path}` (e.g. `GET /users`) and the
/// type is `"request"`. The response status is attached as the transaction
/// result (`"HTTP 200"`, `"HTTP 404"`, etc.).
///
/// If the handler panics, the transaction is still recorded with result
/// `"HTTP 500"` and the panic is resumed after recording.
///
/// When a correlation-id header is configured via
/// [`ApmConfigBuilder::correlation_id_header`], the middleware:
///
/// 1. Reads the header value from the incoming request.
/// 2. Generates a new UUID if the header is absent or empty.
/// 3. Attaches the correlation ID to the transaction.
/// 4. Echoes it back in the response header.
///
/// # Usage
///
/// ```ignore
/// use ravix_apm::apm_middleware;
/// use ravix::MiddlewareChain;
///
/// let chain = MiddlewareChain::new().chain(apm_middleware);
/// ```
pub async fn apm_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let txn_name = format!("{} {}", method, path);

    let handle = Apm::start_transaction(&txn_name, "request", None);
    let txn = handle.active_txn();

    // ── Correlation-ID handling ──────────────────────────────────────────
    let correlation_header = Apm::correlation_id_header();
    let correlation_id = correlation_header.and_then(|header_name| {
        request
            .headers()
            .get(header_name)
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string())
    });
    let correlation_id = correlation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    // Attach to the transaction so it appears on the record.
    txn.set_correlation_id(correlation_id.clone());

    // ── Execute request ──────────────────────────────────────────────────
    let result = AssertUnwindSafe(CURRENT_TXN.scope(txn, async {
        CURRENT_SPAN_ID.scope(Cell::new(Uuid::nil()), next.run(request)).await
    }))
        .catch_unwind()
        .await;

    match result {
        Ok(mut response) => {
            // Echo the correlation ID back in the response header.
            if let Some(header_name) = correlation_header {
                if let Ok(name) = header_name.parse::<HeaderName>() {
                    if let Ok(value) = correlation_id.parse() {
                        response.headers_mut().insert(name, value);
                    }
                }
            }

            let status = response.status().as_u16();
            handle.end(Some(&format!("HTTP {}", status)), None);
            response
        }
        Err(panic_payload) => {
            handle.end(Some("HTTP 500"), None);
            std::panic::resume_unwind(panic_payload);
        }
    }
}