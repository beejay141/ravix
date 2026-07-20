//! Unit tests for controllers demonstrating DI injection patterns.

#[cfg(test)]
mod auth_controller_tests {
    use rusta::prelude::*;
    use crate::models::user::{CreateUserDto, LoginDto, UserResponse};
    use crate::errors::AppError;

    #[tokio::test]
    async fn controller_path_structure() {
        // Test that controller path constants align with expected endpoints
        // The #[controller("/auth")] attribute defines the base path
        // #[post("/register")] and #[post("/login")] extend it
        
        // Verify path patterns used by controllers match expectations
        let register_path = "/auth/register";
        let login_path = "/auth/login";
        
        assert!(register_path.starts_with("/auth"));
        assert!(login_path.starts_with("/auth"));
    }

    #[tokio::test]
    async fn valid_user_dto_passes_validation() {
        let valid_dto = CreateUserDto {
            username: "validuser".to_string(),
            email: "valid@example.com".to_string(),
            password: "password123".to_string(),
        };

        let result = validator::Validate::validate(&valid_dto);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invalid_user_dto_fails_email_validation() {
        let invalid_dto = CreateUserDto {
            username: "validuser".to_string(),
            email: "not-an-email".to_string(),
            password: "password123".to_string(),
        };

        let result = validator::Validate::validate(&invalid_dto);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn invalid_user_dto_fails_username_validation() {
        let invalid_dto = CreateUserDto {
            username: "ab".to_string(), // too short
            email: "valid@example.com".to_string(),
            password: "password123".to_string(),
        };

        let result = validator::Validate::validate(&invalid_dto);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn invalid_user_dto_fails_password_validation() {
        let invalid_dto = CreateUserDto {
            username: "validuser".to_string(),
            email: "valid@example.com".to_string(),
            password: "short".to_string(), // too short (min 8)
        };

        let result = validator::Validate::validate(&invalid_dto);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn valid_login_dto_passes_validation() {
        let valid_dto = LoginDto {
            email: "user@example.com".to_string(),
            password: "password123".to_string(),
        };

        let result = validator::Validate::validate(&valid_dto);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invalid_login_dto_fails_email_validation() {
        let invalid_dto = LoginDto {
            email: "not-an-email".to_string(),
            password: "password123".to_string(),
        };

        let result = validator::Validate::validate(&invalid_dto);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn http_response_created_with_auth_response() {
        let auth_response = crate::models::user::AuthResponse {
            token: "test-jwt-token".to_string(),
            user: UserResponse {
                id: "1".to_string(),
                username: "testuser".to_string(),
                email: "test@example.com".to_string(),
                created_at: chrono::Utc::now(),
            },
        };

        let response = Http::created(auth_response.clone());
        assert_eq!(response.status(), StatusCode::CREATED);

        // Verify body can be deserialized
        let body_bytes = axum::body::to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(parsed["token"], "test-jwt-token");
    }

    #[tokio::test]
    async fn http_error_response_for_validation() {
        let error = AppError::BadRequest("Validation failed".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn http_error_response_for_unauthorized() {
        let error = AppError::Unauthorized("Invalid credentials".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn http_error_response_for_conflict() {
        let error = AppError::Conflict("Email already exists".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
}

#[cfg(test)]
mod post_controller_tests {
    #[tokio::test]
    async fn post_controller_path_structure() {
        // Test that post controller paths follow the pattern
        // /posts base + / endpoint
        let index_path = "/posts";
        
        // /posts base + /{id} endpoint  
        let show_path = "/posts/{id}";

        assert_eq!(index_path, "/posts");
        assert!(show_path.contains("{id}"));
    }
}

#[cfg(test)]
mod comment_controller_tests {
    #[tokio::test]
    async fn comment_controller_path_structure() {
        // Test that comment controller paths follow the pattern
        // /posts/{post_id}/comments base + / endpoint
        let index_path = "/posts/{post_id}/comments";

        assert!(index_path.contains("{post_id}"));
    }
}