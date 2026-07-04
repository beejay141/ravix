use std::sync::Arc;

use ravix::prelude::*;
use uuid::Uuid;

use crate::middleware::{active_user_guard, auth_guard};
use crate::models::CreateUserDto;
use crate::services::UserService;

#[injectable]
pub struct UserController {
    #[inject]
    pub svc: Arc<UserService>,
}

#[controller("/users")]
impl UserController {
    /// GET /users — list all users (public)
    #[get("/")]
    pub async fn list_users(&self) -> Response {
        Http::json(self.svc.find_all().await)
    }

    /// GET /users/:id — get a single user (auth-protected, active-users only)
    #[get("/:id")]
    #[middleware(auth_guard)]
    #[middleware(active_user_guard)]
    pub async fn get_user(&self, Path(id): Path<Uuid>) -> Response {
        match self.svc.find_by_id(id).await {
            Some(user) => Http::json(user),
            None => Http::not_found("User not found"),
        }
    }

    /// POST /users — create a new user (public)
    #[post("/")]
    pub async fn create_user(&self, Json(body): Json<CreateUserDto>) -> Response {
        Http::created(self.svc.create(body).await)
    }
}
