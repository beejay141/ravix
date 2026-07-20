pub mod auth_service;
pub mod comment_service;
pub mod post_service;
#[cfg(test)]
mod tests;

pub use auth_service::AuthService;
pub use comment_service::CommentService;
pub use post_service::PostService;

use rusta::{Container, Injectable};

pub fn register_services(container: &mut Container) {
    container.register(AuthService::construct(container));
    container.register(PostService::construct(container));
    container.register(CommentService::construct(container));
}