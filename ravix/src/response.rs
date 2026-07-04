use axum::{
    http::StatusCode,
    response::{IntoResponse, Json as AxumJson},
};
use serde::Serialize;
use serde_json::json;

/// Convenience helpers for building JSON HTTP responses.
///
/// Every method returns `axum::response::Response` (re-exported as
/// [`Response`]) so they compose naturally with axum handlers.
pub struct Http;

impl Http {
    /// 200 OK with a JSON-serialised body.
    pub fn json<T: Serialize>(data: T) -> crate::Response {
        AxumJson(data).into_response()
    }

    /// 200 OK with an empty body.
    pub fn ok() -> crate::Response {
        StatusCode::OK.into_response()
    }

    /// 201 Created with a JSON-serialised body.
    pub fn created<T: Serialize>(data: T) -> crate::Response {
        (StatusCode::CREATED, AxumJson(data)).into_response()
    }

    /// Arbitrary status code with an empty body.
    pub fn status(code: u16) -> crate::Response {
        StatusCode::from_u16(code)
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            .into_response()
    }

    /// Arbitrary status code with a JSON-serialised body.
    pub fn with_status<T: Serialize>(code: u16, body: T) -> crate::Response {
        let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, AxumJson(body)).into_response()
    }

    /// 204 No Content with an empty body.
    pub fn no_content() -> crate::Response {
        StatusCode::NO_CONTENT.into_response()
    }

    /// 403 Forbidden with a JSON error message.
    pub fn forbidden(message: &str) -> crate::Response {
        (
            StatusCode::FORBIDDEN,
            AxumJson(json!({ "error": message })),
        )
            .into_response()
    }

    /// Alias for `unauthorized` to support different naming preferences.
    pub fn unauthorize(message: &str) -> crate::Response {
        Self::unauthorized(message)
    }

    /// 404 Not Found with a JSON error message.
    pub fn not_found(message: &str) -> crate::Response {
        (StatusCode::NOT_FOUND, AxumJson(json!({ "error": message }))).into_response()
    }

    /// 401 Unauthorized with a JSON error message.
    pub fn unauthorized(message: &str) -> crate::Response {
        (
            StatusCode::UNAUTHORIZED,
            AxumJson(json!({ "error": message })),
        )
            .into_response()
    }

    /// 400 Bad Request with a JSON error message.
    pub fn bad_request(message: &str) -> crate::Response {
        (
            StatusCode::BAD_REQUEST,
            AxumJson(json!({ "error": message })),
        )
            .into_response()
    }

    /// 500 Internal Server Error with a JSON error message.
    pub fn internal_error(message: &str) -> crate::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({ "error": message })),
        )
            .into_response()
    }
}
