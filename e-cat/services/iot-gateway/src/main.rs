use axum::{Router, routing::{get, post}};
use ecat_health::HealthRegistry;
use iot_gateway::{
    api_version::ApiVersionLayer,
    auth_compat::JwtAuthCompat,
    proxy::{ProxyState, access_proxy, access_proxy_open},
    scan::ScanLayer,
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
        .nest("/api", admin_api)
        .nest("/admin", client_api)
        .layer(ApiVersionLayer)
        .layer(ScanLayer::new());

    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
    let mut app = ecat::App::builder()
        .name("iot-gateway")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
