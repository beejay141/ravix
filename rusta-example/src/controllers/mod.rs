mod auth_controller;
mod comment_controller;
mod post_controller;
#[cfg(test)]
mod tests;

use rusta::{Container, Injectable};

use auth_controller::AuthController;
use comment_controller::CommentController;
use post_controller::PostController;

pub fn register_controllers(container: &mut Container) {
    container.register(AuthController::construct(container));
    container.register(PostController::construct(container));
    container.register(CommentController::construct(container));
}