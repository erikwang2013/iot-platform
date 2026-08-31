use axum::{Router, middleware};
use ecat_data_redis::RedisCache;
use ecat_data_sqlx::SqlxClient;
use ecat_mq_kafka::KafkaMq;
use ecat_mq_mqtt::MqttMq;
use ecat_access::{
    api::{self, ApiState},
    auth::{self, AuthState},
    console::{self, ConsoleState},
    oauth::{self, OauthState},
    store::Store,
    webhook::{self, WebhookState},
};
use std::sync::Arc;

async fn migrate(db: &SqlxClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 编译期 include_str! 内联，无运行时文件依赖（iot-device 亦经此读取单一副本）
    for sql in [
        include_str!("../migrations/0001_init.sql"),
        include_str!("../migrations/0002_vendor_auth.sql"),
        include_str!("../migrations/0003_platform.sql"),
        include_str!("../migrations/0004_audit.sql"),
        include_str!("../migrations/0005_groups.sql"),
        include_str!("../migrations/0006_api_keys.sql"),
        include_str!("../migrations/0007_command_queue.sql"),
    ] {
        db.execute_script(sql).await?;
    }
    Ok(())
}

/// 首次启动播种约定初始账号：租户 tenant-1 + 管理员 admin/admin123
/// （哈希随运行时 pepper 计算，不硬编码进 SQL）。
async fn seed_admin(store: &Store) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    store.ensure_tenant("tenant-1", "默认租户").await?;
    if store.find_user("admin").await?.is_some() {
        return Ok(());
    }
    let pepper = std::env::var("IOT_PASSWORD_PEPPER")
        .unwrap_or_else(|_| "iot-password-pepper-v1".into());
    let hash = ecat_security::crypto::hmac_sha256_hex(&pepper, b"admin123");
    store.create_user("tenant-1", "admin", &hash, "admin").await?;
    tracing::info!("seeded initial admin account: admin / admin123 (tenant-1)");
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
    let miot_client_id =
        std::env::var("MIOT_CLIENT_ID").unwrap_or_else(|_| "dev-miot-client-id".into());
    // 测试/演示指向 mock：export TUYA_OPENAPI_BASE=http://127.0.0.1:18084
    // TUYA_CLIENT_SECRET / MIOT_CLIENT_SECRET 由各 exchange 函数读取

    let db = SqlxClient::connect(&db_url).await?;
    migrate(&db).await?;
    let db = Arc::new(db);
    let redis = Arc::new(RedisCache::connect(&redis_url).await?);
    let kafka = Arc::new(KafkaMq::connect(&kafka_brokers).await?);
    let mqtt = Arc::new(MqttMq::connect(&mqtt_url).await?);
    let store = Arc::new(Store::new(db.clone(), &enc_key));
    seed_admin(&store).await?;

    let callback_base = std::env::var("ACCESS_CALLBACK_BASE")
        .unwrap_or_else(|_| "http://localhost:8080/api/access/oauth/callback".into());

    let oauth_state = OauthState {
        store: store.clone(),
        tuya_client_id,
        miot_client_id,
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
    let auth_state = AuthState {
        store: store.clone(),
        jwt_secret: std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "dev-secret-key-0123456789abcdefghijklmn".into()),
        token_ttl_secs: std::env::var("JWT_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(24 * 3600),
    };
    let console_state = ConsoleState { store: store.clone() };

    // 后台任务：直连 MQTT 订阅
    let (mqtt_run_mqtt, mqtt_run_store, mqtt_run_redis, mqtt_run_kafka) =
        (mqtt.clone(), store.clone(), redis.clone(), kafka.clone());
    tokio::spawn(async move {
        ecat_access::mqtt::run(mqtt_run_mqtt, mqtt_run_store, mqtt_run_redis, mqtt_run_kafka)
            .await;
    });

    // 定时任务：设备离线巡检（B-1）——心跳超时（默认 5 分钟）未上报则标记
    // offline + 发 offline 事件。每 60s 巡检一次（OFFLINE_PATROL_SECS 可配）。
    let mut scheduler = ecat_scheduler::Scheduler::new();
    let patrol_interval = std::env::var("OFFLINE_PATROL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60u64);
    ecat_access::offline::register(
        &mut scheduler,
        store.clone(),
        redis.clone(),
        kafka.clone(),
        std::time::Duration::from_secs(patrol_interval),
    );
    tokio::spawn(async move { scheduler.run().await });

    // 后台任务：消费指令事件（D-3 联动）→ MQTT 下发目标设备
    let (cmd_kafka, cmd_mqtt, cmd_store) = (kafka.clone(), mqtt.clone(), store.clone());
    tokio::spawn(async move {
        ecat_access::command_consumer::run(cmd_kafka, cmd_mqtt, cmd_store).await;
    });

    // 公开路由：涂鸦 webhook、OAuth 回调、登录、开放 API 换 token
    // （浏览器跳回/登录前置/开放客户端，无 JWT/租户）
    let login = Router::new().route("/login", axum::routing::post(auth::login)).with_state(auth_state.clone());
    let open = Router::new()
        .route("/open/token", axum::routing::post(auth::open_token))
        .with_state(auth_state);
    let public = Router::new()
        .merge(webhook::router(webhook_state))
        .merge(oauth::router_public(oauth_state.clone()))
        .merge(open);
    // 受保护路由：需网关 secret + x-tenant-id
    let protected = Router::new()
        .merge(oauth::router(oauth_state))
        .merge(api::router(api_state))
        .merge(console::router(console_state))
        .layer(middleware::from_fn(ecat_middleware::tenant_from_header));

    let health_router = ecat_health::HealthRegistry::new()
        .with_check(ecat_health::db_check(db.clone()))
        .into_router();

    let router = Router::new()
        .merge(health_router)
        .nest("/api/access", public)
        .nest("/api/access", protected)
        // 登录（管理端 /api/auth/login 与客户端 /admin/auth/login）走网关节流，
        // 顶层挂载避开 /api/access 前缀
        .nest("/api/auth", login.clone())
        .nest("/admin/auth", login);

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
