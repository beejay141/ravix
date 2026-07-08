use std::sync::Arc;

use ravix::{App, Container, CorsConfig, Injectable};
use ravix_apm::{apm_middleware, config as apm_config, Apm};
use ravix::MiddlewareChain;
use ravix_logger::{config as log_config, logger_middleware, Logger};

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

    // ── Logger ───────────────────────────────────────────────────────────────
    Logger::configure(
        log_config()
            .service_name("ravix-example")
            .service_version("0.1.0")
            .environment("development")
            .add_classification("PUBLIC", "public.ndjson")
            .add_classification("CONFIDENTIAL", "confidential.ndjson")
            .build(),
    )
    .await;

    let mut container = Container::new();

    // ── Infrastructure ─────────────────────────────────────────────────────
    let logger = Logger::new();
    container.register(logger);

    // ── DAL layer ─────────────────────────────────────────────────────────────
    let repo = InMemoryUserRepository::construct(&container) as Arc<dyn UserRepository>;
    container.register(repo);

    // ── Service layer ─────────────────────────────────────────────────────────
    let svc = UserService::construct(&container);
    container.register(svc);

    // ── Controller layer ───────────────────────────────────────────────────────
    let ctrl = UserController::construct(&container);
    container.register(ctrl);

    // ── CORS (optional) ───────────────────────────────────────────────────────
    let cors = CorsConfig::builder()
        .allow_origins(vec![
            "http://localhost:3000".to_string(),
            "http://localhost:5173".to_string(),
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
    let middleware = MiddlewareChain::new()
        .chain(logger_middleware)
        .chain(apm_middleware);

    // ── Boot ──────────────────────────────────────────────────────────────────
    App::new()
        .container(container)
        .cors(cors)
        .middleware(middleware)
        .run("0.0.0.0:3001")
        .await;

    // Gracefully drain all log writers before the runtime drops.
    Logger::shutdown().await;
}