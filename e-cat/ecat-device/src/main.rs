use axum::{Router, middleware, routing::{delete, get, post, put}};
use ecat_data_sqlx::SqlxClient;
use ecat_device::{
    Db, create_firmware, create_ota_task, delete_device, delete_firmware, device_stats,
    list_devices, list_firmwares, list_ota_tasks, migrate, report_ota_progress, unbind_device,
    update_device,
};
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

    // 受保护路由：需网关 secret + x-tenant-id（与 ecat-access/rule/data 一致）
    let devices = Router::new()
        .route("/", get(list_devices))
        .route("/stats", get(device_stats))
        .route("/{id}", put(update_device).delete(delete_device))
        .route("/{id}/unbind", post(unbind_device))
        .with_state(Db(db.clone()));
    let ota = Router::new()
        .route("/firmwares", get(list_firmwares).post(create_firmware))
        .route("/firmwares/{id}", delete(delete_firmware))
        .route("/tasks", get(list_ota_tasks).post(create_ota_task))
        .route("/tasks/{id}/report", post(report_ota_progress))
        .with_state(Db(db));
    let protected = Router::new()
        .nest("/api/devices", devices)
        .nest("/api/ota", ota)
        .layer(middleware::from_fn(ecat_middleware::tenant_from_header));

    let router = Router::new().merge(health_router).merge(protected);

    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8081").router(router);
    let mut app = ecat::App::builder()
        .name("iot-device")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
