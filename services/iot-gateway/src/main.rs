use axum::{Router, routing::get};
use iot_gateway::api_version::ApiVersionLayer;

async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let router = Router::new()
        .route("/health", get(health))
        .route("/api/ping", get(|| async { "pong" }))
        .layer(ApiVersionLayer);
    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
    let mut app = ecat::App::builder().name("iot-gateway").version("0.1.0").server(srv).build()?;
    app.run().await?;
    Ok(())
}
