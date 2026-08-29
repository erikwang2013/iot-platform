use axum::{Router, routing::{get, post}};
use iot_gateway::{api_version::ApiVersionLayer, auth_compat::JwtAuthCompat, scan::ScanLayer};

async fn health() -> &'static str {
    "OK"
}

async fn submit() -> &'static str {
    "ok"
}

async fn devices() -> &'static str {
    "admin-devices"
}

async fn me() -> &'static str {
    "client-me"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-key-0123456789abcdefghijklmn".into());

    let admin_api = Router::new()
        .route("/devices", get(devices))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?);
    let client_api = Router::new()
        .route("/me", get(me))
        .layer(JwtAuthCompat::new(&secret, &["sub"])?);

    let router = Router::new()
        .route("/health", get(health))
        .route("/api/ping", get(|| async { "pong" }))
        .route("/api/submit", post(submit))
        .nest("/api", admin_api)
        .nest("/admin", client_api)
        .layer(ApiVersionLayer)
        .layer(ScanLayer::new());

    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
    let mut app = ecat::App::builder()
        .name("iot-gateway")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
