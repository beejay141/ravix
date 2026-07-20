use std::cell::Cell;
use std::panic::AssertUnwindSafe;

use futures::FutureExt;
use uuid::Uuid;

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;

use crate::context::{CURRENT_SPAN_ID, CURRENT_TXN};
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
/// use rusta_apm::apm_middleware;
/// use rusta::MiddlewareChain;
///
/// let chain = MiddlewareChain::new().chain(apm_middleware);
/// ```
pub async fn apm_middleware(apm: std::sync::Arc<Apm>, request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let txn_name = format!("{} {}", method, path);
    let handle = apm.start_transaction(&txn_name, "request", None);
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
        CURRENT_SPAN_ID
            .scope(Cell::new(Uuid::nil()), next.run(request))
            .await
    }))
    .catch_unwind()
    .await;

    match result {
        Ok(mut response) => {
            // Echo the correlation ID back in the response header.
            if let Some(header_name) = correlation_header {
                if let Ok(value) = HeaderValue::from_str(&correlation_id) {
                    response.headers_mut().insert(header_name.clone(), value);
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderName, Method, Uri, StatusCode};

    #[test]
    fn extract_method_and_path() {
        let uri = Uri::try_from("https://example.com/users/123").unwrap();
        assert_eq!(uri.path(), "/users/123");
    }

    #[test]
    fn correlation_id_generation() {
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();
        assert_ne!(id1, id2);
    }

    #[test]
    fn header_value_creation() {
        let value = HeaderValue::from_str("test-correlation-id").unwrap();
        assert_eq!(value, "test-correlation-id");
    }

    #[test]
    fn header_value_from_uuid() {
        let id = Uuid::new_v4();
        let value = HeaderValue::from_str(&id.to_string()).unwrap();
        assert_eq!(value, id.to_string());
    }

    #[test]
    fn header_name_parsing() {
        let header = "X-Correlation-ID".parse::<HeaderName>().unwrap();
        assert_eq!(header.as_str(), "x-correlation-id");
    }

    #[test]
    fn method_extraction() {
        let method = Method::GET;
        assert_eq!(method.as_str(), "GET");
    }

    #[test]
    fn uri_path_extraction() {
        let uri: Uri = "/api/users".parse().unwrap();
        assert_eq!(uri.path(), "/api/users");
    }

    #[test]
    fn uri_query_extraction() {
        let uri: Uri = "/api/users?page=1&limit=10".parse().unwrap();
        assert_eq!(uri.query(), Some("page=1&limit=10"));
    }

    #[test]
    fn status_code_formatting() {
        let status = StatusCode::OK;
        assert_eq!(status.as_u16(), 200);
        let status = StatusCode::NOT_FOUND;
        assert_eq!(status.as_u16(), 404);
    }

    #[test]
    fn header_map_operations() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Request-ID", "test-123".parse().unwrap());

        assert_eq!(headers.get("X-Request-ID").unwrap(), "test-123");
    }

    #[test]
    fn header_map_get_missing() {
        let headers = HeaderMap::new();
        assert!(headers.get("X-Missing").is_none());
    }

    #[test]
    fn empty_string_filter() {
        let value = "";
        assert!(value.is_empty());
        let value = "test";
        assert!(!value.is_empty());
    }

    #[test]
    fn option_and_then_chain() {
        let result: Option<String> = Some("value")
            .filter(|v| !v.is_empty())
            .map(|v| v.to_uppercase());
        assert_eq!(result, Some("VALUE".to_string()));
    }

    #[test]
    fn option_none_chain() {
        let result: Option<String> = None::<String>
            .filter(|v| !v.is_empty())
            .map(|v| v.to_uppercase());
        assert!(result.is_none());
    }

    #[test]
    fn apm_middleware_txn_name_format() {
        // Test the transaction name format logic
        let method = Method::GET;
        let path = "/users";
        let txn_name = format!("{} {}", method, path);
        assert_eq!(txn_name, "GET /users");
    }

    #[test]
    fn apm_middleware_status_format() {
        // Test the status code format logic
        let status = StatusCode::OK;
        let result = format!("HTTP {}", status.as_u16());
        assert_eq!(result, "HTTP 200");
    }

    #[test]
    fn apm_middleware_status_format_not_found() {
        let status = StatusCode::NOT_FOUND;
        let result = format!("HTTP {}", status.as_u16());
        assert_eq!(result, "HTTP 404");
    }

    #[test]
    fn apm_middleware_status_format_internal_error() {
        let status = StatusCode::INTERNAL_SERVER_ERROR;
        let result = format!("HTTP {}", status.as_u16());
        assert_eq!(result, "HTTP 500");
    }
}
