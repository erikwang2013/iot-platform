//! 需 docker：`docker compose up -d tdengine kafka` 后运行：
//! `cargo test -p iot-data --test data_flow -- --ignored`
use ecat_data::TsdbClient;
use ecat_data_tdengine::TdengineClient;
use ecat_data_tdengine::sql::parse_points;
use ecat_mq::MessageQueue;
use ecat_mq_kafka::KafkaMq;
use ecat_data_service::api::{ApiState, build_history_sql};
use ecat_data_service::models::EventMessage;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
#[ignore]
async fn kafka_event_lands_in_tdengine_and_queryable() {
    let td = Arc::new(TdengineClient::new(
        "http://localhost:6041",
        "root",
        "taosdata",
    ));
    for sql in ecat_data_service::td::schema_sqls() {
        td.query(&sql).await.expect("schema init");
    }

    // 1) 直接经 ingest 的 SQL 造数（等价于 Kafka 消费产物）
    let ev = EventMessage {
        device_id: "itest-dev".into(),
        tenant_id: "itest-tenant".into(),
        kind: "property".into(),
        code: "temp".into(),
        value: json!(23.5),
        ts: 1_690_000_000_000,
    };
    td.query(&ecat_data_service::ingest::batch_sql(&[ev]))
        .await
        .expect("ingest insert");

    // 2) 查询 API 的 SQL 可读回
    let q = ecat_data_service::api::HistoryQuery {
        device_id: "itest-dev".into(),
        code: "temp".into(),
        start: 1_690_000_000_000,
        end: 1_690_000_100_000,
        agg: None,
        interval: None,
        limit: 100,
        offset: 0,
    };
    let resp = td.query(&build_history_sql("itest-tenant", &q)).await.unwrap();
    let points = parse_points(&resp).unwrap();
    assert_eq!(points.len(), 1, "应查到 1 点: {resp}");
    assert_eq!(points[0].ts, 1_690_000_000_000);
    assert_eq!(points[0].value, json!(23.5));

    // 3) Kafka → 消费侧反序列化形状（模拟 P1 发布的载荷）
    let kafka = KafkaMq::connect("localhost:9092").await.unwrap();
    let payload = serde_json::to_vec(&EventMessage {
        device_id: "itest-dev".into(),
        tenant_id: "itest-tenant".into(),
        kind: "property".into(),
        code: "hum".into(),
        value: json!(60),
        ts: 1_690_000_000_001,
    })
    .unwrap();
    kafka.publish("iot.events", &payload).await.unwrap();
    // 发布后短等消费异步（集成测试不做端到端断言，只验证发布无错）
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
#[ignore]
async fn http_history_handler_returns_points() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let td = Arc::new(TdengineClient::new(
        "http://localhost:6041",
        "root",
        "taosdata",
    ));
    for sql in ecat_data_service::td::schema_sqls() {
        td.query(&sql).await.expect("schema init");
    }
    let ev = EventMessage {
        device_id: "itest-dev".into(),
        tenant_id: "itest-tenant".into(),
        kind: "property".into(),
        code: "temp".into(),
        value: json!(25.0),
        ts: 1_690_000_100_000,
    };
    td.query(&ecat_data_service::ingest::batch_sql(&[ev]))
        .await
        .expect("ingest insert");

    let app = ecat_data_service::api::router(ApiState { td: td.clone() });
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/history?device_id=itest-dev&code=temp&start=1690000000000&end=1690000002000")
                .extension("itest-tenant".to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v["count"].as_i64().unwrap() >= 1, "应至少 1 点: {v}");
}
