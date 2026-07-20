use axum::{
    http::StatusCode,
    response::{IntoResponse, Json as AxumJson},
};
use serde::Serialize;
use serde_json::json;

use crate::ErrorResponse;

/// Wrapper for error objects that should be passed through as-is.
/// Use this to wrap serializable structs when you want them returned directly
/// instead of being wrapped in `{ "error": ... }`.
///
/// # Example
/// ```
/// use rusta::{Http, ErrorResponse, ErrorObject};
///
/// // Simple string error
/// let response = Http::error(400, ErrorResponse::message("Bad request"));
///
/// // Structured error object - accepts any serializable type
/// #[derive(serde::Serialize)]
/// struct ValidationError {
///     code: &'static str,
///     fields: Vec<&'static str>,
/// }
/// let response = Http::error(400, ErrorObject(ValidationError {
///     code: "VALIDATION_ERROR",
///     fields: vec!["email", "password"],
/// }));
/// # Ok::<(), ()>(())
/// ```
pub struct ErrorObject<T>(pub T);

impl<T: Serialize> From<ErrorObject<T>> for ErrorResponse {
    fn from(obj: ErrorObject<T>) -> Self {
        Self::Object(
            serde_json::to_value(obj.0)
                .unwrap_or_else(|_| json!({ "error": "serialization failed" })),
        )
    }
}

/// Convenience helpers for building JSON HTTP responses.
///
/// Every method returns `axum::response::Response` (re-exported as
/// [`Response`]) so they compose naturally with axum handlers.
///
/// # Available Methods
///
/// | Method | Status | Description |
/// |--------|--------|-------------|
/// | `json(data)` | 200 | JSON response with body |
/// | `created(data)` | 201 | Created with JSON body |
/// | `ok()` | 200 | Empty OK response |
/// | `no_content()` | 204 | No Content response |
/// | `status(code)` | custom | Empty response with status code |
/// | `with_status(code, body)` | custom | JSON response with custom status |
/// | `not_found(msg)` | 404 | Not Found error |
/// | `not_found_with(obj)` | 404 | Not Found with structured error |
/// | `unauthorized(msg)` | 401 | Unauthorized error |
/// | `unauthorized_with(obj)` | 401 | Unauthorized with structured error |
/// | `bad_request(msg)` | 400 | Bad Request error |
/// | `bad_request_with(obj)` | 400 | Bad Request with structured error |
/// | `forbidden(msg)` | 403 | Forbidden error |
/// | `forbidden_with(obj)` | 403 | Forbidden with structured error |
/// | `internal_error(msg)` | 500 | Internal Server Error |
/// | `internal_error_with(obj)` | 500 | Internal Error with structured error |
/// | `error(status, error)` | custom | Flexible error response |
pub struct Http;

impl Http {
    /// 200 OK with a JSON-serialised body.
    ///
    /// # Example
    /// ```ignore
    /// use rusta::Http;
    /// let response = Http::json(vec!["item1", "item2"]);
    /// ```
    pub fn json<T: Serialize>(data: T) -> crate::Response {
        AxumJson(data).into_response()
    }

    /// 200 OK with an empty body.
    ///
    /// # Example
    /// ```ignore
    /// use rusta::Http;
    /// let response = Http::ok();
    /// ```
    pub fn ok() -> crate::Response {
        StatusCode::OK.into_response()
    }

    /// 201 Created with a JSON-serialised body.
    ///
    /// # Example
    /// ```ignore
    /// use rusta::Http;
    /// let response = Http::created(my_new_resource);
    /// ```
    pub fn created<T: Serialize>(data: T) -> crate::Response {
        (StatusCode::CREATED, AxumJson(data)).into_response()
    }

    /// 204 No Content with an empty body.
    ///
    /// # Example
    /// ```ignore
    /// use rusta::Http;
    /// let response = Http::no_content();
    /// ```
    pub fn no_content() -> crate::Response {
        StatusCode::NO_CONTENT.into_response()
    }

    /// Arbitrary status code with an empty body.
    ///
    /// # Example
    /// ```ignore
    /// use rusta::Http;
    /// let response = Http::status(202); // Accepted
    /// ```
    pub fn status(code: u16) -> crate::Response {
        StatusCode::from_u16(code)
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            .into_response()
    }

    /// Arbitrary status code with a JSON-serialised body.
    ///
    /// # Example
    /// ```ignore
    /// use rusta::Http;
    /// let response = Http::with_status(202, serde_json::json!({ "status": "processing" }));
    /// ```
    pub fn with_status<T: Serialize>(code: u16, body: T) -> crate::Response {
        let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, AxumJson(body)).into_response()
    }

    /// 403 Forbidden with a flexible error response.
    ///
    /// # Example
    /// ```
    /// use rusta::{Http, ErrorResponse};
    ///
    /// // Simple string error
    /// let response = Http::forbidden("Access denied");
    ///
    /// // Structured error object
    /// let response = Http::forbidden_with(serde_json::json!({
    ///     "code": "INSUFFICIENT_PERMISSIONS",
    ///     "message": "Access denied",
    ///     "required_role": "admin"
    /// }));    /// # Ok::<(), ()>(())    /// ```
    pub fn forbidden(message: &str) -> crate::Response {
        (StatusCode::FORBIDDEN, AxumJson(json!({ "error": message }))).into_response()
    }

    /// 403 Forbidden with a structured error object.
    pub fn forbidden_with<T: Serialize>(error: T) -> crate::Response {
        (
            StatusCode::FORBIDDEN,
            AxumJson(
                serde_json::to_value(error)
                    .unwrap_or_else(|_| json!({ "error": "serialization failed" })),
            ),
        )
            .into_response()
    }

    /// Alias for `unauthorized` to support different naming preferences.
    pub fn unauthorize(message: &str) -> crate::Response {
        Self::unauthorized(message)
    }

    /// 404 Not Found with a flexible error response.
    pub fn not_found(message: &str) -> crate::Response {
        (StatusCode::NOT_FOUND, AxumJson(json!({ "error": message }))).into_response()
    }

    /// 404 Not Found with a structured error object.
    pub fn not_found_with<T: Serialize>(error: T) -> crate::Response {
        (
            StatusCode::NOT_FOUND,
            AxumJson(
                serde_json::to_value(error)
                    .unwrap_or_else(|_| json!({ "error": "serialization failed" })),
            ),
        )
            .into_response()
    }

    /// 401 Unauthorized with a flexible error response.
    pub fn unauthorized(message: &str) -> crate::Response {
        (
            StatusCode::UNAUTHORIZED,
            AxumJson(json!({ "error": message })),
        )
            .into_response()
    }

    /// 401 Unauthorized with a structured error object.
    pub fn unauthorized_with<T: Serialize>(error: T) -> crate::Response {
        (
            StatusCode::UNAUTHORIZED,
            AxumJson(
                serde_json::to_value(error)
                    .unwrap_or_else(|_| json!({ "error": "serialization failed" })),
            ),
        )
            .into_response()
    }

    /// 400 Bad Request with a flexible error response.
    pub fn bad_request(message: &str) -> crate::Response {
        (
            StatusCode::BAD_REQUEST,
            AxumJson(json!({ "error": message })),
        )
            .into_response()
    }

    /// 400 Bad Request with a structured error object.
    pub fn bad_request_with<T: Serialize>(error: T) -> crate::Response {
        (
            StatusCode::BAD_REQUEST,
            AxumJson(
                serde_json::to_value(error)
                    .unwrap_or_else(|_| json!({ "error": "serialization failed" })),
            ),
        )
            .into_response()
    }

    /// 500 Internal Server Error with a flexible error response.
    pub fn internal_error(message: &str) -> crate::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({ "error": message })),
        )
            .into_response()
    }

    /// 500 Internal Server Error with a structured error object.
    pub fn internal_error_with<T: Serialize>(error: T) -> crate::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(
                serde_json::to_value(error)
                    .unwrap_or_else(|_| json!({ "error": "serialization failed" })),
            ),
        )
            .into_response()
    }

    /// Flexible error response with any status code.
    ///
    /// Accepts `ErrorResponse` or `ErrorObject<T>` which can be created from strings
    /// or any serializable struct.
    ///
    /// # Example
    /// ```
    /// use rusta::{Http, ErrorResponse, ErrorObject};
    ///
    /// // Simple string error
    /// let response = Http::error(400, ErrorResponse::message("Bad request"));
    ///
    /// // Structured error object - accepts any serializable type
    /// #[derive(serde::Serialize)]
    /// struct ValidationError {
    ///     code: &'static str,
    ///     fields: Vec<&'static str>,
    /// }
    /// let response = Http::error(400, ErrorObject(ValidationError {
    ///     code: "VALIDATION_ERROR",
    ///     fields: vec!["email", "password"],
    /// }));
    /// # Ok::<(), ()>(())
    /// ```
    pub fn error(status: u16, error: impl Into<ErrorResponse>) -> crate::Response {
        let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, AxumJson(error.into().into_json())).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use serde::Serialize;

    #[tokio::test]
    async fn json_returns_200_and_body() {
        let resp = Http::json(serde_json::json!({ "a": 1, "b": "x" }));
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], "x");
    }

    #[tokio::test]
    async fn created_returns_201() {
        let resp = Http::created(serde_json::json!({ "id": 5 }));
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["id"], 5);
    }

    #[test]
    fn ok_and_no_content_statuses() {
        assert_eq!(Http::ok().status(), StatusCode::OK);
        assert_eq!(Http::no_content().status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn error_with_message_renders_error_object() {
        let resp = Http::error(400, ErrorResponse::message("Bad request"));
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "Bad request");
    }

    #[derive(Serialize)]
    struct MyError {
        code: &'static str,
        detail: &'static str,
    }

    #[tokio::test]
    async fn error_with_struct_passes_through_object() {
        let obj = MyError {
            code: "E1",
            detail: "failed",
        };
        let resp = Http::error(422, ErrorObject(obj));
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["code"], "E1");
        assert_eq!(v["detail"], "failed");
    }

    #[tokio::test]
    async fn status_valid_code() {
        let resp = Http::status(202);
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn status_invalid_code_returns_unused() {
        // StatusCode::from_u16 returns the code as-is for unknown codes
        let resp = Http::status(999);
        assert_eq!(resp.status(), StatusCode::from_u16(999).unwrap());
    }

    #[tokio::test]
    async fn with_status_valid_code() {
        let resp = Http::with_status(202, serde_json::json!({ "status": "processing" }));
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "processing");
    }

    #[tokio::test]
    async fn with_status_invalid_code_returns_unused() {
        // StatusCode::from_u16 returns the code as-is for unknown codes
        let resp = Http::with_status(999, serde_json::json!({ "x": 1 }));
        assert_eq!(resp.status(), StatusCode::from_u16(999).unwrap());
    }

    #[tokio::test]
    async fn forbidden_renders_error() {
        let resp = Http::forbidden("Access denied");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "Access denied");
    }

    #[tokio::test]
    async fn forbidden_with_renders_object() {
        let resp = Http::forbidden_with(json!({ "code": "FORBIDDEN" }));
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["code"], "FORBIDDEN");
    }

    #[tokio::test]
    async fn unauthorized_renders_error() {
        let resp = Http::unauthorized("Login required");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "Login required");
    }

    #[tokio::test]
    async fn unauthorized_with_renders_object() {
        let resp = Http::unauthorized_with(json!({ "code": "UNAUTH" }));
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["code"], "UNAUTH");
    }

    #[tokio::test]
    async fn unauthorize_is_alias_for_unauthorized() {
        let resp = Http::unauthorize("Login required");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn not_found_renders_error() {
        let resp = Http::not_found("Missing");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "Missing");
    }

    #[tokio::test]
    async fn not_found_with_renders_object() {
        let resp = Http::not_found_with(json!({ "code": "NOT_FOUND" }));
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn bad_request_renders_error() {
        let resp = Http::bad_request("Invalid");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "Invalid");
    }

    #[tokio::test]
    async fn bad_request_with_renders_object() {
        let resp = Http::bad_request_with(json!({ "code": "BAD_REQ" }));
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["code"], "BAD_REQ");
    }

    #[tokio::test]
    async fn internal_error_renders_error() {
        let resp = Http::internal_error("Crash");
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "Crash");
    }

    #[tokio::test]
    async fn internal_error_with_renders_object() {
        let resp = Http::internal_error_with(json!({ "code": "INT_ERR" }));
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["code"], "INT_ERR");
    }
}
