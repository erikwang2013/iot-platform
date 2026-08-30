use axum::{
    Router,
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use ecat_data::TsdbClient;
use ecat_mq_kafka::KafkaMq;
use iot_data::api::{self, ApiState};
use std::sync::Arc;

async fn health() -> &'static str {
    "OK"
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 安全变量缺失直接启动失败，禁止静默降级（与 iot-access 一致）
    std::env::var("IOT_GATEWAY_SECRET")
        .map_err(|_| "IOT_GATEWAY_SECRET not set".to_string())?;

    let td_url = std::env::var("TDENGINE_URL").unwrap_or_else(|_| "http://localhost:6041".into());
    let td_user = std::env::var("TDENGINE_USER").unwrap_or_else(|_| "root".into());
    let td_pass = std::env::var("TDENGINE_PASS").unwrap_or_else(|_| "taosdata".into());
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());

    let td = Arc::new(iot_data::td::connect(&td_url, &td_user, &td_pass));
    // 幂等建库建表
    for sql in iot_data::td::schema_sqls() {
        td.query(&sql).await?;
    }

    let kafka = Arc::new(KafkaMq::connect(&kafka_brokers).await?);

    // 后台任务：消费 iot.events → TDengine
    let (ingest_td, ingest_kafka) = (td.clone(), kafka.clone());
    tokio::spawn(async move {
        iot_data::ingest::run(ingest_td, ingest_kafka).await;
    });

    let api_state = ApiState { td: td.clone() };

    // 受保护路由：需网关 secret + x-tenant-id
    let protected = Router::new()
        .merge(api::router(api_state))
        .layer(middleware::from_fn(ecat_middleware::tenant_from_header));

    let router = Router::new()
        .route("/health", get(health))
        .nest("/api/data", protected);

    let bind = std::env::var("HTTP_BIND").unwrap_or_else(|_| "0.0.0.0:8083".into());
    let srv = ecat_transport_http::HttpServer::new(bind).router(router);
    let mut app = ecat::App::builder()
        .name("iot-data")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
