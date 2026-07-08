use std::sync::Arc;

use ravix::prelude::*;
use uuid::Uuid;

use crate::middleware::{active_user_guard, auth_guard};
use crate::models::CreateUserDto;
use crate::services::UserService;
use ravix_logger::{LogOptions, Logger};
use serde_json::json;

#[injectable]
pub struct UserController {
    #[inject]
    pub svc: Arc<UserService>,
    #[inject]
    pub logger: Arc<Logger>,
}

#[controller("/users")]
impl UserController {
    /// GET /users — list all users (public)
    #[get("/")]
    pub async fn list_users(&self) -> Response {
        self.logger.info("listing users", None);
        Http::json(self.svc.find_all().await)
    }

    /// GET /users/:id — get a single user (auth-protected, active-users only)
    #[get("/:id")]
    #[middleware(auth_guard)]
    #[middleware(active_user_guard)]
    pub async fn get_user(&self, Path(id): Path<Uuid>) -> Response {
        self.logger.info(
            "user fetched",
            Some(LogOptions {
                classification: None,
                context: Some(
                    [("user_id".to_string(), json!(id.to_string()))]
                        .into_iter()
                        .collect(),
                ),
            }),
        );
        match self.svc.find_by_id(id).await {
            Some(user) => Http::json(user),
            None => Http::not_found("User not found"),
        }
    }

    /// POST /users — create a new user (public)
    #[post("/")]
    pub async fn create_user(&self, Json(body): Json<CreateUserDto>) -> Response {
        self.logger.info(
            "user created",
            Some(LogOptions {
                classification: None,
                context: Some(
                    [("name".to_string(), json!(body.name))]
                        .into_iter()
                        .collect(),
                ),
            }),
        );
        Http::created(self.svc.create(body).await)
    }
}