use axum::{Router, routing::get};

async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let router = Router::new().route("/health", get(health));
    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8081").router(router);
    let mut app = ecat::App::builder().name("iot-device").version("0.1.0").server(srv).build()?;
    app.run().await?;
    Ok(())
}
