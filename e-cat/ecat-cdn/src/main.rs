use axum::{Router, middleware};
use ecat_data_sqlx::SqlxClient;
use ecat_cdn::{api::{self, ApiState}, store::CdnStore};
use std::sync::Arc;

async fn migrate(db: &SqlxClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    db.execute_script(include_str!("../migrations/0001_cdn.sql")).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 凭据加密密钥与网关租户门：缺失直接启动失败，禁止静默降级
    let enc_key = std::env::var("IOT_CRED_ENCRYPT_KEY")
        .map_err(|_| "IOT_CRED_ENCRYPT_KEY not set".to_string())?;
    std::env::var("IOT_GATEWAY_SECRET")
        .map_err(|_| "IOT_GATEWAY_SECRET not set".to_string())?;

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://iot:iot@localhost:3306/iot".into());
    let db = SqlxClient::connect(&db_url).await?;
    migrate(&db).await?;
    let db = Arc::new(db);
    let store = Arc::new(CdnStore::new(db.clone(), &enc_key));

    // 受保护路由：需网关 secret + x-tenant-id
    let protected = Router::new()
        .nest("/api/cdn", api::router(ApiState { store }))
        .layer(middleware::from_fn(ecat_middleware::tenant_from_header));

    let health_router = ecat_health::HealthRegistry::new()
        .with_check(ecat_health::db_check(db))
        .into_router();

    // C-3 Prometheus：/metrics 公开（scrape 端点），MetricsLayer 记请求数/时延/状态码
    let router = Router::new()
        .merge(health_router)
        .merge(protected)
        .merge(ecat_metrics::metrics_router())
        .layer(ecat_metrics::MetricsLayer::new());

    let bind = std::env::var("HTTP_BIND").unwrap_or_else(|_| "0.0.0.0:8085".into());
    let srv = ecat_transport_http::HttpServer::new(bind).router(router);
    let mut app = ecat::App::builder()
        .name("iot-cdn")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
