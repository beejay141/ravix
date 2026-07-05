use std::sync::Arc;

use ravix::{App, Container, CorsConfig, Injectable};
use ravix_apm::{apm_middleware, config as apm_config, Apm};
use ravix::MiddlewareChain;

mod controllers;
mod middleware;
mod models;
mod repositories;
mod services;

use controllers::UserController;
use repositories::{InMemoryUserRepository, UserRepository};
use services::UserService;

#[tokio::main]
async fn main() {
    // ── APM ─────────────────────────────────────────────────────────────────
    Apm::configure(
        apm_config()
            .service_name("ravix-example")
            .service_version("0.1.0")
            .environment("development")
            .log_path("apm.ndjson")
            .correlation_id_header("X-Correlation-ID")
            .build(),
    )
    .await;

    let mut container = Container::new();

    // ── DAL layer ─────────────────────────────────────────────────────────────
    // InMemoryUserRepository has no #[inject] fields; construct() uses Default for all.
    let repo = InMemoryUserRepository::construct(&container) as Arc<dyn UserRepository>;
    container.register(repo);

    // ── Service layer ─────────────────────────────────────────────────────────
    // UserService has #[inject] repo: Arc<dyn UserRepository>.
    // construct() calls container.resolve::<Arc<dyn UserRepository>>() automatically.
    let svc = UserService::construct(&container);
    container.register(svc);

    // ── Controller layer ───────────────────────────────────────────────────────
    // UserController has #[inject] svc: Arc<dyn IUserService>.
    let ctrl = UserController::construct(&container);
    container.register(ctrl);

    // ── CORS (optional) ───────────────────────────────────────────────────────
    // Remove or adjust this for production. Wildcard origins are convenient for
    // local development but should be restricted to specific domains in prod.
    // NOTE: If you enable allow_credentials(), you must also specify explicit
    // allow_origins() and allow_headers() — wildcards + credentials are invalid.
    let cors = CorsConfig::builder()
        .allow_origins(vec![
            "http://localhost:3000".to_string(),
            "http://localhost:5173".to_string(), // Vite default
        ])
        .allow_methods(vec![
            "GET".to_string(),
            "POST".to_string(),
            "PUT".to_string(),
            "DELETE".to_string(),
            "PATCH".to_string(),
        ])
        .allow_headers(vec![
            "content-type".to_string(),
            "authorization".to_string(),
        ])
        .max_age(3600)
        .build();

    // ── Middleware ────────────────────────────────────────────────────────────
    let middleware = MiddlewareChain::new().chain(apm_middleware);

    // ── Boot ──────────────────────────────────────────────────────────────────
    App::new()
        .container(container)
        .cors(cors) // omit this line to disable CORS entirely
        .middleware(middleware)
        .run("0.0.0.0:3001")
        .await;
}