use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response as AxumResponse},
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiError {
    #[error("Dependency injection error: {0}")]
    InjectionError(String),
}

impl IntoResponse for DiError {
    fn into_response(self) -> AxumResponse {
        let (status, message) = match &self {
            Self::InjectionError(m) => (StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn di_error_display() {
        let err = DiError::InjectionError("test error".into());
        assert!(err.to_string().contains("test error"));
    }

    #[test]
    fn di_error_message_contents() {
        let err = DiError::InjectionError("missing dependency".into());
        assert_eq!(err.to_string(), "Dependency injection error: missing dependency");
    }

    #[test]
    fn di_error_into_response_status() {
        let err = DiError::InjectionError("test".into());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn di_error_into_response_body() {
        let err = DiError::InjectionError("test error".into());
        let response = err.into_response();
        let _body = response.into_body();
    }

    #[test]
    fn di_error_is_error_trait() {
        let err = DiError::InjectionError("err".into());
        let _: &dyn std::error::Error = &err;
    }
}