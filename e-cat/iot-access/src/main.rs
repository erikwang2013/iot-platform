use axum::{Router, middleware};
use ecat_data_redis::RedisCache;
use ecat_data_sqlx::SqlxClient;
use ecat_mq_kafka::KafkaMq;
use ecat_mq_mqtt::MqttMq;
use ecat_access::{
    api::{self, ApiState},
    oauth::{self, OauthState},
    store::Store,
    webhook::{self, WebhookState},
};
use std::sync::Arc;

async fn migrate(db: &SqlxClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for file in ["migrations/0001_init.sql", "migrations/0002_vendor_auth.sql"] {
        db.execute_script(&std::fs::read_to_string(file)?).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 安全相关变量缺失直接启动失败，禁止静默降级（oauth state 签名密钥 / 租户门）
    let enc_key = std::env::var("IOT_CRED_ENCRYPT_KEY")
        .map_err(|_| "IOT_CRED_ENCRYPT_KEY not set".to_string())?;
    std::env::var("IOT_GATEWAY_SECRET")
        .map_err(|_| "IOT_GATEWAY_SECRET not set".to_string())?;

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://iot:iot@localhost:3306/iot".into());
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());
    let mqtt_url = std::env::var("MQTT_URL").unwrap_or_else(|_| "tcp://localhost:1883".into());
    let tuya_client_id =
        std::env::var("TUYA_CLIENT_ID").unwrap_or_else(|_| "dev-tuya-client-id".into());
    // 测试/演示指向 mock：export TUYA_OPENAPI_BASE=http://127.0.0.1:18084
    // TUYA_CLIENT_SECRET 由 oauth::exchange_authorization_code 读取

    let db = SqlxClient::connect(&db_url).await?;
    migrate(&db).await?;
    let db = Arc::new(db);
    let redis = Arc::new(RedisCache::connect(&redis_url).await?);
    let kafka = Arc::new(KafkaMq::connect(&kafka_brokers).await?);
    let mqtt = Arc::new(MqttMq::connect(&mqtt_url).await?);
    let store = Arc::new(Store::new(db.clone(), &enc_key));

    let callback_base = std::env::var("ACCESS_CALLBACK_BASE")
        .unwrap_or_else(|_| "http://localhost:8080/api/access/oauth/callback".into());

    let oauth_state = OauthState {
        store: store.clone(),
        tuya_client_id,
        callback_base,
    };
    let api_state = ApiState {
        store: store.clone(),
        kafka: kafka.clone(),
        redis: redis.clone(),
        mqtt: mqtt.clone(),
    };
    let webhook_state = WebhookState {
        store: store.clone(),
        kafka: kafka.clone(),
        redis: redis.clone(),
    };

    // 后台任务：直连 MQTT 订阅
    let (mqtt_run_mqtt, mqtt_run_store, mqtt_run_redis, mqtt_run_kafka) =
        (mqtt.clone(), store.clone(), redis.clone(), kafka.clone());
    tokio::spawn(async move {
        ecat_access::mqtt::run(mqtt_run_mqtt, mqtt_run_store, mqtt_run_redis, mqtt_run_kafka)
            .await;
    });

    // 公开路由：涂鸦 webhook、OAuth 回调（浏览器跳回，无 JWT/租户）
    let public = Router::new()
        .merge(webhook::router(webhook_state))
        .merge(oauth::router_public(oauth_state.clone()));
    // 受保护路由：需网关 secret + x-tenant-id
    let protected = Router::new()
        .merge(oauth::router(oauth_state))
        .merge(api::router(api_state))
        .layer(middleware::from_fn(ecat_middleware::tenant_from_header));

    let health_router = ecat_health::HealthRegistry::new()
        .with_check(ecat_health::db_check(db.clone()))
        .into_router();

    let router = Router::new()
        .merge(health_router)
        .nest("/api/access", public)
        .nest("/api/access", protected);

    let bind = std::env::var("HTTP_BIND").unwrap_or_else(|_| "0.0.0.0:8082".into());
    let srv = ecat_transport_http::HttpServer::new(bind).router(router);
    let mut app = ecat::App::builder()
        .name("iot-access")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
