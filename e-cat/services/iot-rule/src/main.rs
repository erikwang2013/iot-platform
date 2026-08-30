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

/// 恒定时间比较，避免通过响应时序探测 secret（与 iot-access 一致）。
fn secret_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// 受保护路由前置门（与 iot-access/iot-data 一致）：必须携带与 IOT_GATEWAY_SECRET
/// 一致的 x-gateway-secret（只由网关反代持有）+ 合法 x-tenant-id，
/// 防止客户端绕过网关直接自报任意租户。租户写入 extensions 供 handler 用。
async fn tenant_from_header(mut req: Request, next: Next) -> Response {
    let expected = std::env::var("IOT_GATEWAY_SECRET").unwrap_or_default();
    let secret_ok = req
        .headers()
        .get("x-gateway-secret")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| secret_eq(v, expected.as_str()));
    if !secret_ok {
        return (StatusCode::UNAUTHORIZED, "missing or bad x-gateway-secret").into_response();
    }
    let tenant = match req
        .headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
    {
        Some(t)
            if !t.is_empty()
                && t.len() <= 64
                && t.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') =>
        {
            t
        }
        _ => return (StatusCode::UNAUTHORIZED, "missing or invalid x-tenant-id").into_response(),
    };
    req.extensions_mut().insert(tenant);
    next.run(req).await
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
        .layer(middleware::from_fn(tenant_from_header));

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
