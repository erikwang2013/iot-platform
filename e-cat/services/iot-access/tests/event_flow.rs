//! 集成测试：需要 `docker compose up -d mysql redis emqx kafka`。
//! 运行：cargo test -p iot-access --test event_flow -- --ignored --nocapture
use axum::body::Body;
use ecat_data::{Cache, RdbmsClient};
use ecat_data_redis::RedisCache;
use ecat_data_sqlx::SqlxClient;
use ecat_mq::MessageQueue;
use ecat_mq_kafka::KafkaMq;
use futures_util::StreamExt;
use iot_access::crypto::derive_key;
use iot_access::events::shadow_key;
use iot_access::store::Store;
use iot_access::webhook::{WebhookState, router};
use std::sync::Arc;
use tower::ServiceExt;

async fn setup() -> (Store, KafkaMq, RedisCache) {
    // 端口对应本地容器映射（compose 默认：mysql 3306、redis 6379（MYSQL_PORT/REDIS_PORT 可覆盖）、kafka 9092），可用环境变量覆盖
    let db = Arc::new(
        SqlxClient::connect(
            &std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "mysql://iot:iot@localhost:3306/iot".into()),
        )
        .await
        .unwrap(),
    );
    // 幂等建表（与 iot-access 启动迁移一致）
    for file in ["migrations/0001_init.sql", "migrations/0002_vendor_auth.sql"] {
        let sql = std::fs::read_to_string(file).unwrap();
        for stmt in sql.split(';').filter(|s| !s.trim().is_empty()) {
            db.execute(stmt).await.unwrap();
        }
    }
    let store = Store::new(db, "test-encrypt-key-0123456789");
    // 种子：租户 + 涂鸦设备 + 凭据（REPLACE 保证覆盖历史残留行，密文始终为本测试密钥所写）
    let db = store.db.clone();
    let _ = db
        .execute_with(
            "REPLACE INTO tenants (id, name) VALUES ('t1', 'mock-tenant')",
            &[],
        )
        .await;
    let _ = db
        .execute_with(
            "REPLACE INTO devices (id, tenant_id, name, vendor, status) \
             VALUES ('p1', 't1', 'mock-tuya-1', 'tuya', 'online')",
            &[],
        )
        .await;
    let _ = db
        .execute_with(
            "REPLACE INTO device_links (device_id, tenant_id, vendor, vendor_id, vendor_name, category) \
             VALUES ('p1', 't1', 'tuya', 'tuya-dev-1', 'mock-tuya-1', 'temp_sensor')",
            &[],
        )
        .await;
    let enc = iot_access::crypto::encrypt(
        &derive_key("test-encrypt-key-0123456789"),
        b"{\"client_secret\":\"mock-client-secret\"}",
    )
    .unwrap();
    let _ = db
        .execute_with(
            "REPLACE INTO vendor_credentials (id, tenant_id, vendor, config_encrypted, status) \
             VALUES ('c1', 't1', 'tuya', ?, 'active')",
            &[serde_json::json!(enc)],
        )
        .await;
    let kafka = KafkaMq::connect(&std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into()))
        .await
        .unwrap();
    let redis = RedisCache::connect(&std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into()))
        .await
        .unwrap();
    (store, kafka, redis)
}

#[tokio::test]
#[ignore = "requires docker compose up -d mysql redis kafka"]
async fn webhook_event_reaches_kafka_and_shadow() {
    let (store, kafka, redis) = setup().await;
    // 先订阅 Kafka（group 从 latest 开始，先于发布才能收到事件），等 rebalance
    let mut stream = kafka.subscribe("iot.events").await.unwrap();
    let mut stream = futures_util::stream::poll_fn(move |cx| stream.poll_recv(cx)).boxed();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let redis = Arc::new(redis);
    let ws = WebhookState {
        store: Arc::new(store),
        kafka: Arc::new(kafka),
        redis: redis.clone(),
    };
    let app = router(ws);
    let resp = app.clone().oneshot(webhook_request()).await.unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    // Kafka 断言：group 首次 join 要等 broker rebalance（默认延迟 3s），
    // 分配前发布的消息会被跳过，故超时则重发（webhook 幂等，重发只补事件）。
    let mut tries = 0;
    let raw = loop {
        if let Ok(Some(Ok(raw))) =
            tokio::time::timeout(std::time::Duration::from_secs(10), stream.next()).await
        {
            break raw;
        }
        tries += 1;
        assert!(tries < 5, "kafka event timeout after {tries} re-posts");
        let resp = app.clone().oneshot(webhook_request()).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    };
    let ev: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(ev["device_id"], "p1");
    assert_eq!(ev["kind"], "property");
    assert_eq!(ev["value"], 23.5);

    // 影子断言：shadow:p1 含属性 temp=23.5 且 online=true
    let shadow: serde_json::Value = serde_json::from_slice(
        &redis.get(&shadow_key("p1")).await.unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(shadow["online"], true);
    assert_eq!(shadow["properties"]["temp"], 23.5);
}

fn webhook_request() -> axum::http::Request<Body> {
    let raw =
        br#"{"type":"deviceData","bizCode":"report","data":{"deviceId":"tuya-dev-1","code":"temp","value":23.5,"ts":1690000000000}}"#;
    axum::http::Request::builder()
        .method("POST")
        .uri("/webhook/tuya")
        .header("content-type", "application/json")
        // 签名：HMAC-SHA256(body, mock-client-secret)
        .header("x-tuya-signature", tuya_sign(raw))
        .body(Body::from(raw.to_vec()))
        .unwrap()
}

fn tuya_sign(raw: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(b"mock-client-secret").unwrap();
    mac.update(raw);
    hex::encode(mac.finalize().into_bytes())
}
