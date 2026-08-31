use axum::{Router, middleware};
use ecat_mq_kafka::KafkaMq;
use ecat_data_service::api::{self, ApiState};
use std::sync::Arc;

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

    // 查询侧：TSDB_KIND 选后端（B-4）tdengine（默认）| clickhouse，幂等建库建表按方言
    let td = ecat_data_service::td::connect_tsdb().await;
    for sql in ecat_data_service::td::schema_sqls() {
        td.query(&sql).await?;
    }

    // 消费组固定：多副本时分区在实例间拆分，避免重复写入（TDengine 同 ts 覆盖幂等兜底）
    let kafka = Arc::new(
        KafkaMq::from_config(ecat_mq_kafka::KafkaConfig {
            brokers: kafka_brokers.clone(),
            group_id: Some("iot-data-ingest".into()),
            auto_commit: false,
            security_protocol: None,
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
        })
        .await?,
    );

    // 异常检测独立消费组（与 ingest 组互不抢分区）：消费 iot.events →
    // 统计基线检测 → 异常事件回写同 topic 供 rule 引擎入告警流
    let anomaly_kafka = Arc::new(
        KafkaMq::from_config(ecat_mq_kafka::KafkaConfig {
            brokers: kafka_brokers.clone(),
            group_id: Some("iot-anomaly".into()),
            auto_commit: false,
            security_protocol: None,
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
        })
        .await?,
    );

    // 后台任务：消费 iot.events → TDengine（写入侧直连；ClickHouse 写入路径为验证型后续，
    // 查询侧 history/export 已按 TSDB_KIND 双方言支持）
    let (ingest_td, ingest_kafka) = (
        Arc::new(ecat_data_service::td::connect(&td_url, &td_user, &td_pass)),
        kafka.clone(),
    );
    tokio::spawn(async move {
        ecat_data_service::ingest::run(ingest_td, ingest_kafka).await;
    });

    // 后台任务：统计异常检测（Welford 在线基线，z-score 判异）
    tokio::spawn(async move {
        ecat_data_service::anomaly::run(anomaly_kafka).await;
    });

    // 定时任务：数据生命周期清理（C-6）——按 DATA_RETENTION_DAYS 周期
    // 删除超期时序数据。每 6h 巡检一次；删除幂等、失败仅记日志。
    let mut scheduler = ecat_scheduler::Scheduler::new();
    let retention_interval = std::env::var("DATA_RETENTION_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6 * 3600u64);
    ecat_data_service::retention::register(
        &mut scheduler,
        td.clone(),
        std::time::Duration::from_secs(retention_interval),
    );
    tokio::spawn(async move { scheduler.run().await });

    let api_state = ApiState {
        td: td.clone(),
        dialect: ecat_data_service::td::dialect(),
    };

    // 受保护路由：需网关 secret + x-tenant-id
    let protected = Router::new()
        .merge(api::router(api_state))
        .layer(middleware::from_fn(ecat_middleware::tenant_from_header));

    // 健康检查 SQL 随后端方言：TDengine 用 server_status()，ClickHouse 无此函数用 SELECT 1
    let health_sql = match ecat_data_service::td::dialect() {
        ecat_data_service::td::Dialect::Clickhouse => "SELECT 1".to_string(),
        _ => "SELECT server_status()".to_string(),
    };
    let health_router = ecat_health::HealthRegistry::new()
        .with_check(ecat_health::FnCheck::new("td", {
            let td = td.clone();
            move || {
                let td = td.clone();
                let health_sql = health_sql.clone();
                async move {
                    td.query(&health_sql)
                        .await
                        .map(|_| ())
                        .map_err(|e| {
                            // 细节只进日志，不回给客户端（/ready 无鉴权直接可达）
                            tracing::warn!(error = %e, "health check tsdb failed");
                            "tsdb check failed".to_string()
                        })
                }
            }
        }))
        .into_router();

    // C-3 Prometheus：/metrics 公开（scrape 端点），MetricsLayer 记请求数/时延/状态码
    let router = Router::new()
        .merge(health_router)
        .nest("/api/data", protected)
        .merge(ecat_metrics::metrics_router())
        .layer(ecat_metrics::MetricsLayer::new());

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
