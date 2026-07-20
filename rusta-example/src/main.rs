use std::sync::Arc;

use rusta::{App, Container, CorsConfig, MiddlewareChain};
use rusta_apm::{apm_middleware, config as apm_config, Apm};
use rusta_logger::{config as log_config, logger_middleware, Logger};

mod config;
mod controllers;
mod db;
mod errors;
mod middleware;
mod models;
mod repositories;
mod services;

use config::AppConfig;
use controllers::register_controllers;
use repositories::register_repositories;
use services::register_services;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let app_config = AppConfig::from_env();

    // ── APM ─────────────────────────────────────────────────────────────────
    let apm = Apm::configure(
        apm_config()
            .service_name("blog-api")
            .service_version("0.1.0")
            .environment("development")
            .log_path("apm.ndjson")
            .correlation_id_header("X-Correlation-ID")
            .build(),
    )
    .await;

    // ── Logger ───────────────────────────────────────────────────────────────
    let logger = Logger::configure(
        log_config()
            .service_name("blog-api")
            .service_version("0.1.0")
            .environment("development")
            .add_classification("PUBLIC", "public.ndjson")
            .add_classification("CONFIDENTIAL", "confidential.ndjson")
            .build(),
    )
    .await;

    let mut container = Container::new();
    container.register(apm.clone());
    container.register(logger);

    let db = db::init_mongo(&app_config.mongo_uri, &app_config.mongo_db).await;
    container.register(db);

    container.register(Arc::new(app_config.clone()));

    // ── Repositories ──────────────────────────────────────────────────────
    register_repositories(&mut container);

    // ── Services ──────────────────────────────────────────────────────────
    register_services(&mut container);

    // ── Controllers ───────────────────────────────────────────────────────
    register_controllers(&mut container);

    // ── CORS ─────────────────────────────────────────────────────────────
    let cors = CorsConfig::builder()
        .allow_origins(vec![
            String::from("http://localhost:3000"),
            String::from("http://localhost:5173"),
        ])
        .allow_methods(vec![
            String::from("GET"),
            String::from("POST"),
            String::from("PUT"),
            String::from("DELETE"),
            String::from("PATCH"),
        ])
        .allow_headers(vec![
            String::from("content-type"),
            String::from("authorization"),
        ])
        .max_age(3600)
        .build();

    // ── Middleware ────────────────────────────────────────────────────────
    let middleware = MiddlewareChain::new()
        .chain(logger_middleware)
        .chain(move |req, next| apm_middleware(apm.clone(), req, next));

    // ── Boot ──────────────────────────────────────────────────────────────
    App::new()
        .container(container)
        .cors(cors)
        .middleware(middleware)
        .run(&app_config.server_port, |res| match res {
            Ok(addr) => println!("Listening on {}", addr),
            Err(msg) => eprintln!("Startup error: {}", msg),
        })
        .await;

    Apm::shutdown().await;
    Logger::shutdown().await;
}
