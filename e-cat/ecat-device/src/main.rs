use axum::{Router, routing::get};
use ecat_data_sqlx::SqlxClient;
use ecat_device::{Db, list_devices, migrate};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://iot:iot@localhost:3306/iot".into());
    let db = SqlxClient::connect(&db_url).await?;
    migrate(&db).await?;
    let db = Arc::new(db);

    let health_router = ecat_health::HealthRegistry::new()
        .with_check(ecat_health::db_check(db.clone()))
        .into_router();

    let router = Router::new()
        .merge(health_router)
        .merge(Router::new().route("/api/devices", get(list_devices)).with_state(Db(db)));

    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8081").router(router);
    let mut app = ecat::App::builder()
        .name("iot-device")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
