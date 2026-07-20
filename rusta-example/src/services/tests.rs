//! Unit tests for services demonstrating dependency mocking.

#[cfg(test)]
mod auth_service_tests {
    use crate::models::user::{CreateUserDto, LoginDto, User, UserResponse, Claims};
    use crate::errors::AppError;
    use async_trait::async_trait;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Manual mock for UserRepository that can be controlled in tests
    #[async_trait]
    pub trait MockUserRepository: Send + Sync {
        async fn find_by_id(&self, id: &str) -> Result<Option<User>, AppError>;
        async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError>;
        async fn find_by_username(&self, username: &str) -> Result<Option<User>, AppError>;
        async fn save(&self, dto: CreateUserDto, password_hash: String) -> Result<User, AppError>;
    }

    /// Simple mock implementation using Mutex for state control
    #[derive(Debug, Default)]
    pub struct MockUserRepo {
        /// Users that will be returned by find_by_id
        pub find_by_id_users: Mutex<HashMap<String, User>>,
        /// Users that will be returned by find_by_email
        pub find_by_email_users: Mutex<HashMap<String, User>>,
        /// Users that will be returned by find_by_username
        pub find_by_username_users: Mutex<HashMap<String, User>>,
        /// Track call counts
        pub find_by_id_calls: AtomicUsize,
        pub find_by_email_calls: AtomicUsize,
        pub find_by_username_calls: AtomicUsize,
        pub save_calls: AtomicUsize,
    }

    impl MockUserRepo {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn user_for_email(&self, user: User) {
            self.find_by_email_users.lock().unwrap().insert(user.email.clone(), user);
        }
    }

    #[async_trait]
    impl MockUserRepository for MockUserRepo {
        async fn find_by_id(&self, id: &str) -> Result<Option<User>, AppError> {
            self.find_by_id_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.find_by_id_users.lock().unwrap().get(id).cloned())
        }

        async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
            self.find_by_email_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.find_by_email_users.lock().unwrap().get(email).cloned())
        }

        async fn find_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
            self.find_by_username_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.find_by_username_users.lock().unwrap().get(username).cloned())
        }

        async fn save(&self, dto: CreateUserDto, _password_hash: String) -> Result<User, AppError> {
            self.save_calls.fetch_add(1, Ordering::SeqCst);
            Ok(User {
                id: format!("mock-{}", self.save_calls.load(Ordering::SeqCst)),
                username: dto.username,
                email: dto.email,
                password_hash: "[mocked]".to_string(),
                created_at: Utc::now(),
            })
        }
    }

    /// Helper to create a test user
    fn create_test_user(id: &str, username: &str, email: &str) -> User {
        User {
            id: id.to_string(),
            username: username.to_string(),
            email: email.to_string(),
            password_hash: "hashed_password".to_string(),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn mock_user_repository_find_by_email_returns_user() {
        let mock = MockUserRepo::new();
        let test_user = create_test_user("1", "testuser", "test@example.com");
        mock.user_for_email(test_user.clone());

        let result = mock.find_by_email("test@example.com").await.unwrap();
        assert!(result.is_some());
        let found = result.unwrap();
        assert_eq!(found.email, "test@example.com");
        assert_eq!(mock.find_by_email_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn mock_user_repository_find_by_email_returns_none() {
        let mock = MockUserRepo::new();

        let result = mock.find_by_email("nonexistent@example.com").await.unwrap();
        assert!(result.is_none());
        assert_eq!(mock.find_by_email_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn mock_user_repository_save_creates_user() {
        let mock = MockUserRepo::new();
        let dto = CreateUserDto {
            username: "newuser".to_string(),
            email: "new@example.com".to_string(),
            password: "password123".to_string(),
        };

        let result = mock.save(dto.clone(), "hash".to_string()).await.unwrap();
        assert_eq!(result.username, "newuser");
        assert_eq!(mock.save_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn login_endpoint_validation_error() {
        // Test validation error handling in controller
        let invalid_dto = LoginDto {
            email: "not-an-email".to_string(),
            password: "password123".to_string(),
        };

        // Validation should fail for invalid email
        let validation_result = validator::Validate::validate(&invalid_dto);
        assert!(validation_result.is_err());
    }

    #[tokio::test]
    async fn register_endpoint_validation_error() {
        let invalid_dto = CreateUserDto {
            username: "ab".to_string(), // too short (min 3)
            email: "not-an-email".to_string(),
            password: "short".to_string(), // too short (min 8)
        };

        let validation_result = validator::Validate::validate(&invalid_dto);
        assert!(validation_result.is_err());
    }

    #[test]
    fn user_response_from_user() {
        let user = create_test_user("1", "testuser", "test@example.com");
        let response: UserResponse = UserResponse::from(user);

        assert_eq!(response.id, "1");
        assert_eq!(response.username, "testuser");
        assert_eq!(response.email, "test@example.com");
    }

    #[tokio::test]
    async fn auth_service_verify_token_valid() {
        // Create a valid JWT token with a test secret
        let jwt_secret = "test-secret-key";

        let claims = Claims {
            sub: "user-123".to_string(),
            username: "testuser".to_string(),
            exp: (Utc::now().timestamp() as usize) + 3600,
        };
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(jwt_secret.as_bytes()),
        )
        .unwrap();

        // Verify the token structure
        let token_data = jsonwebtoken::decode::<Claims>(
            &token,
            &jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_bytes()),
            &jsonwebtoken::Validation::default(),
        )
        .unwrap();

        assert_eq!(token_data.claims.sub, "user-123");
        assert_eq!(token_data.claims.username, "testuser");
    }

    #[tokio::test]
    async fn auth_service_verify_token_invalid() {
        let jwt_secret = "test-secret-key";

        let invalid_token = "invalid.token.here";
        let result = jsonwebtoken::decode::<Claims>(
            invalid_token,
            &jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_bytes()),
            &jsonwebtoken::Validation::default(),
        );

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn mock_user_repository_find_by_id_returns_none() {
        let mock = MockUserRepo::new();

        let result = mock.find_by_id("nonexistent-id").await.unwrap();
        assert!(result.is_none());
        assert_eq!(mock.find_by_id_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn mock_user_repository_find_by_username_returns_none() {
        let mock = MockUserRepo::new();

        let result = mock.find_by_username("nonexistent").await.unwrap();
        assert!(result.is_none());
        assert_eq!(mock.find_by_username_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn mock_user_repository_multiple_calls() {
        let mock = MockUserRepo::new();

        // Multiple calls increment the counter
        let _ = mock.find_by_email("a@example.com").await;
        let _ = mock.find_by_email("b@example.com").await;
        let _ = mock.find_by_email("c@example.com").await;

        assert_eq!(mock.find_by_email_calls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn config_from_env_defaults() {
        // Test that config loads defaults when env vars are missing
        let config = crate::config::AppConfig::from_env();
        assert_eq!(config.mongo_uri, "mongodb://localhost:27017");
        assert_eq!(config.mongo_db, "blog");
        assert_eq!(config.jwt_secret, "change_me_in_production");
        assert_eq!(config.jwt_expiry_seconds, 3600);
        assert_eq!(config.server_port, "0.0.0.0:3001");
    }
}