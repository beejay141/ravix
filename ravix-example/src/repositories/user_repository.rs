use std::{collections::HashMap, sync::Mutex};

use async_trait::async_trait;
use ravix::injectable;
use uuid::Uuid;

use crate::models::{CreateUserDto, User};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_all(&self) -> Vec<User>;
    async fn find_by_id(&self, id: Uuid) -> Option<User>;
    async fn save(&self, dto: CreateUserDto) -> User;
}

/// In-memory user repository backed by a `Mutex<HashMap>`.
/// Has no injectable dependencies, so all fields use `Default::default()`.
#[injectable]
pub struct InMemoryUserRepository {
    store: Mutex<HashMap<Uuid, User>>,
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn find_all(&self) -> Vec<User> {
        self.store.lock().unwrap().values().cloned().collect()
    }

    async fn find_by_id(&self, id: Uuid) -> Option<User> {
        self.store.lock().unwrap().get(&id).cloned()
    }

    async fn save(&self, dto: CreateUserDto) -> User {
        let user = User {
            id: Uuid::new_v4(),
            name: dto.name,
            email: dto.email,
            is_active: dto.is_active,
        };
        self.store.lock().unwrap().insert(user.id, user.clone());
        user
    }
}
