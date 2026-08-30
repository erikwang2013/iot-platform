use axum::{Router, routing::{get, post, put}};
use ecat_auth::JwtAuthCompat;
use ecat_health::HealthRegistry;
use ecat_security::SecurityBodyCompatLayer;
use iot_gateway::{
    api_version::ApiVersionLayer,
    proxy::{ProxyState, access_proxy, access_proxy_open, data_proxy, rule_proxy},
};

async fn submit() -> &'static str {
    "ok"
}

async fn devices() -> &'static str {
    "admin-devices"
}

async fn me() -> &'static str {
    "client-me"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-key-0123456789abcdefghijklmn".into());

    let proxy_state = ProxyState {
        client: reqwest::Client::new(),
    };

    // /api/access/* 公开路径：OAuth 回调、涂鸦 Webhook（无 JWT，浏览器/厂商服务器直连）
    let access_public = Router::new()
        .route("/oauth/callback", get(access_proxy_open))
        .route("/webhook/tuya", post(access_proxy_open))
        .with_state(proxy_state.clone());
    // /api/access/* 受保护路径：JWT 校验后透传租户（AuthClaims.sub → x-tenant-id）
    let access_admin = Router::new()
        .route("/oauth/authorize-url", post(access_proxy))
        .route("/vendors/{vendor}/import", post(access_proxy))
        .route("/devices/{device_id}/command", post(access_proxy))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?)
        .with_state(proxy_state.clone());
    // /api/data/* 受保护路径：历史曲线 / 导出（GET，query 透传）
    let data_admin = Router::new()
        .route("/data/history", get(data_proxy))
        .route("/data/export", get(data_proxy))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?)
        .with_state(proxy_state.clone());
    // /api/rule/* 受保护路径：规则 CRUD / 告警记录（REST，WS 直连 8084 不走网关）
    let rule_admin = Router::new()
        .route("/rule/rules", get(rule_proxy).post(rule_proxy))
        .route("/rule/rules/{id}", put(rule_proxy).delete(rule_proxy))
        .route("/rule/alerts", get(rule_proxy))
        .route("/rule/alerts/{id}/ack", post(rule_proxy))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?)
        .with_state(proxy_state);

    let admin_api = Router::new()
        .route("/devices", get(devices))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?);
    let client_api = Router::new()
        .route("/me", get(me))
        .layer(JwtAuthCompat::new(&secret, &["sub"])?);

    let router = Router::new()
        .merge(HealthRegistry::new().into_router())
        .route("/api/ping", get(|| async { "pong" }))
        .route("/api/submit", post(submit))
        .nest("/api/access", access_public)
        .nest("/api/access", access_admin)
        .nest("/api", data_admin)
        .nest("/api", rule_admin)
        .nest("/api", admin_api)
        .nest("/admin", client_api)
        .layer(ApiVersionLayer)
        // 严格模式 + 1MB 体上限：与原 scan.rs 语义一致（任意 severity 即 403）
        .layer(
            SecurityBodyCompatLayer::new()
                .strict()
                .body_limit(1024 * 1024),
        );

    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
    let mut app = ecat::App::builder()
        .name("iot-gateway")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
