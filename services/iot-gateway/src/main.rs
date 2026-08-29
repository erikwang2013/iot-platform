use axum::{Router, routing::{get, post}};
use iot_gateway::{api_version::ApiVersionLayer, scan::ScanLayer};

async fn health() -> &'static str {
    "OK"
}

async fn submit() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let router = Router::new()
        .route("/health", get(health))
        .route("/api/ping", get(|| async { "pong" }))
        .route("/api/submit", post(submit))
        .layer(ApiVersionLayer)
        .layer(ScanLayer::new());
    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
    let mut app = ecat::App::builder().name("iot-gateway").version("0.1.0").server(srv).build()?;
    app.run().await?;
    Ok(())
}
