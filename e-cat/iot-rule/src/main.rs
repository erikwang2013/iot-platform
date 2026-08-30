use axum::{
    Router,
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use ecat_data_sqlx::SqlxClient;
use ecat_mq_kafka::KafkaMq;
use iot_rule::{api::{self, ApiState}, push::PushHub, store::RuleStore, ws};
use std::sync::Arc;

async fn health() -> &'static str {
    "OK"
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 安全相关变量缺失直接启动失败，禁止静默降级（与 iot-access 一致）
    std::env::var("IOT_GATEWAY_SECRET")
        .map_err(|_| "IOT_GATEWAY_SECRET not set".to_string())?;
    std::env::var("JWT_SECRET").map_err(|_| "JWT_SECRET not set".to_string())?;

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://iot:iot@localhost:3306/iot".into());
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());

    let db = SqlxClient::connect(&db_url).await?;
    iot_rule::store::migrate(&db).await?;
    let store = Arc::new(RuleStore::new(Arc::new(db)));

    let kafka = Arc::new(KafkaMq::from_config(iot_rule::engine::kafka_config(&kafka_brokers)).await?);
    let hub = PushHub::new();
    let http = reqwest::Client::new();

    // 后台任务：消费 iot.events → 匹配 → 推送/落库/webhook
    let (run_kafka, run_store, run_hub, run_http) = (kafka.clone(), store.clone(), hub.clone(), http.clone());
    tokio::spawn(async move {
        iot_rule::runner::run(run_kafka, run_store, run_hub, run_http).await;
    });

    let api_state = ApiState { store: store.clone() };

    // 受保护路由：需网关 secret + x-tenant-id（网关反代注入）
    let protected = Router::new()
        .merge(api::router(api_state))
        .layer(middleware::from_fn(ecat_middleware::tenant_from_header));

    // WS 直连端点：JWT query token 校验（P5 前端直连 8084，不走网关）
    let ws_route = Router::new()
        .route("/ws", get(ws::ws_handler))
        .with_state(hub);

    let router = Router::new()
        .route("/health", get(health))
        .merge(ws_route)
        .nest("/api/rule", protected);

    let bind = std::env::var("HTTP_BIND").unwrap_or_else(|_| "0.0.0.0:8084".into());
    let srv = ecat_transport_http::HttpServer::new(bind).router(router);
    let mut app = ecat::App::builder()
        .name("iot-rule")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
