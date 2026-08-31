use axum::{Router, middleware, routing::get};
use ecat_data_sqlx::SqlxClient;
use ecat_mq_kafka::KafkaMq;
use ecat_rule::{
    api::{self, ApiState},
    push::{AlertBridge, PushHub},
    store::RuleStore,
    ws,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 安全相关变量缺失直接启动失败，禁止静默降级（与 iot-access 一致）
    std::env::var("IOT_GATEWAY_SECRET")
        .map_err(|_| "IOT_GATEWAY_SECRET not set".to_string())?;
    std::env::var("JWT_SECRET").map_err(|_| "JWT_SECRET not set".to_string())?;

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://iot:iot@localhost:3306/iot".into());
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());

    let db = SqlxClient::connect(&db_url).await?;
    ecat_rule::store::migrate(&db).await?;
    let db = Arc::new(db);
    let store = Arc::new(RuleStore::new(db.clone()));

    let kafka = Arc::new(KafkaMq::from_config(ecat_rule::engine::kafka_config(&kafka_brokers)).await?);
    let hub = PushHub::new();
    let http = reqwest::Client::new();

    // 告警跨实例广播桥：本地直达 + Redis pub/sub 扇出（多副本时 WS 推送全量可达）
    let bridge = AlertBridge::connect(hub.clone(), &redis_url).await;
    tokio::spawn(AlertBridge::spawn_subscriber(hub.clone(), redis_url.clone()));

    // 后台任务：消费 iot.events → 匹配 → 推送/落库/webhook
    let (run_kafka, run_store, run_bridge, run_http) = (kafka.clone(), store.clone(), bridge.clone(), http.clone());
    tokio::spawn(async move {
        ecat_rule::runner::run(run_kafka, run_store, run_bridge, run_http).await;
    });

    // 定时任务：每日汇总报表（C-线）——每小时兜底生成前一日报表
    // （幂等：已生成跳过；重启后自动补生成）。启动即先生成一次，
    // 不等首个 tick（scheduler 首跑在第一个 interval 之后）。
    ecat_rule::report::run_once(db.clone()).await;
    let mut scheduler = ecat_scheduler::Scheduler::new();
    let report_interval = std::env::var("REPORT_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3600u64);
    ecat_rule::report::register(
        &mut scheduler,
        db.clone(),
        std::time::Duration::from_secs(report_interval),
    );
    tokio::spawn(async move { scheduler.run().await });

    let api_state = ApiState { store: store.clone() };

    // 受保护路由：需网关 secret + x-tenant-id（网关反代注入）
    let protected = Router::new()
        .merge(api::router(api_state))
        .layer(middleware::from_fn(ecat_middleware::tenant_from_header));

    // WS 直连端点：JWT query token 校验（P5 前端直连 8084，不走网关）
    let ws_route = Router::new()
        .route("/ws", get(ws::ws_handler))
        .with_state(hub);

    let health_router = ecat_health::HealthRegistry::new()
        .with_check(ecat_health::db_check(db.clone()))
        .into_router();

    // C-3 Prometheus：/metrics 公开（scrape 端点），MetricsLayer 记请求数/时延/状态码
    let router = Router::new()
        .merge(health_router)
        .merge(ws_route)
        .nest("/api/rule", protected)
        .merge(ecat_metrics::metrics_router())
        .layer(ecat_metrics::MetricsLayer::new());

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
