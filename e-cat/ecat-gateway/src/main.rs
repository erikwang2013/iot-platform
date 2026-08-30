use axum::{Router, response::IntoResponse, routing::{delete, get, post, put}};
use ecat_auth::JwtAuthCompat;
use ecat_health::HealthRegistry;
use ecat_gateway::{
    api_version::ApiVersionLayer,
    proxy::{
        ProxyState, access_proxy, access_proxy_open, auth_proxy, cdn_proxy, console_proxy,
        data_proxy, device_proxy, rule_proxy,
    },
};
use ecat_middleware::{MemoryStore, RateLimitLayer, RateLimitStore, RedisRateLimitStore};
use ecat_security::SecurityBodyCompatLayer;
use std::sync::Arc;
use std::time::Duration;

async fn submit() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-key-0123456789abcdefghijklmn".into());
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".into());

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
        .route("/rule/stats", get(rule_proxy))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?)
        .with_state(proxy_state.clone());
    // /api/cdn/* 受保护路径：供应商 CRUD / 启停 / 刷新预热 / 签名 URL
    let cdn_admin = Router::new()
        .route("/cdn/providers", get(cdn_proxy).post(cdn_proxy))
        .route(
            "/cdn/providers/{id}",
            get(cdn_proxy).put(cdn_proxy).delete(cdn_proxy),
        )
        .route("/cdn/providers/{id}/enable", post(cdn_proxy))
        .route("/cdn/providers/{id}/disable", post(cdn_proxy))
        .route("/cdn/providers/{id}/test", post(cdn_proxy))
        .route("/cdn/providers/{id}/signed-url", post(cdn_proxy))
        .route("/cdn/providers/{id}/purge", post(cdn_proxy))
        .route("/cdn/providers/{id}/prefetch", post(cdn_proxy))
        .route("/cdn/tasks", get(cdn_proxy))
        .route("/cdn/stats", get(cdn_proxy))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?)
        .with_state(proxy_state.clone());

    // 管理面（/api/*）：租户/用户/物模型 → iot-access；设备/OTA → iot-device。
    // 双面都要求 sub+role：客户端登录 token 也带 role（P5 401 根因），
    // 客户端设备列表走 /api/devices 同样受此校验。
    let console_admin = Router::new()
        .route("/tenants", get(console_proxy).post(console_proxy))
        .route("/tenants/{id}", delete(console_proxy))
        .route("/users", get(console_proxy).post(console_proxy))
        .route("/users/{id}", delete(console_proxy))
        .route("/models/things", get(console_proxy).post(console_proxy))
        .route("/models/things/{id}", get(console_proxy).delete(console_proxy))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?)
        .with_state(proxy_state.clone());
    let device_admin = Router::new()
        .route("/devices", get(device_proxy))
        .route("/devices/stats", get(device_proxy))
        .route("/devices/{id}", put(device_proxy).delete(device_proxy))
        .route("/devices/{id}/unbind", post(device_proxy))
        .route("/ota/firmwares", get(device_proxy).post(device_proxy))
        .route("/ota/firmwares/{id}", delete(device_proxy))
        .route("/ota/tasks", get(device_proxy).post(device_proxy))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?)
        .with_state(proxy_state.clone());
    // 登录端点：管理端 /api/auth/login 与客户端 /admin/auth/login 同一 handler，
    // 独立更严的按 IP 限流（10 次/分钟）防爆破；全局 100 次/分钟限流仍叠加生效
    let login_router = Router::new()
        .route("/auth/login", post(auth_proxy))
        .layer(
            tower::ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(rate_limit_error))
                .layer(login_rate_limit()),
        )
        .with_state(proxy_state);

    let router = Router::new()
        .merge(HealthRegistry::new().into_router())
        .route("/api/ping", get(|| async { "pong" }))
        .route("/api/submit", post(submit))
        .nest("/api/access", access_public)
        .nest("/api/access", access_admin)
        .nest("/api", data_admin)
        .nest("/api", rule_admin)
        .nest("/api", cdn_admin)
        .nest("/api", console_admin)
        .nest("/api", device_admin)
        .nest("/api", login_router.clone())
        .nest("/admin", login_router)
        .layer(ApiVersionLayer)
        // 严格模式 + 1MB 体上限：与原 scan.rs 语义一致（任意 severity 即 403）
        .layer(
            SecurityBodyCompatLayer::new()
                .strict()
                .body_limit(1024 * 1024),
        )
        // 限流：默认 100 次/分钟（RATE_LIMIT_MAX / RATE_LIMIT_WINDOW_SECS 可配），
        // Redis 不可用降级内存存储（fail-open，与 store 语义一致）；
        // HandleErrorLayer 包裹限流层，把超限错误映射为 429
        .layer(
            // ServiceBuilder：先加的层在外层，故 HandleError 包住限流层
            tower::ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(rate_limit_error))
                .layer(rate_limit(&redis_url).await)
                .into_inner(),
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

/// Redis 限流存储，不可用时降级内存（fail-open，与 store 语义一致）。
async fn rate_limit(redis_url: &str) -> RateLimitLayer<axum::body::Body> {
    let max = env_u32("RATE_LIMIT_MAX", 100);
    let window_secs = env_u32("RATE_LIMIT_WINDOW_SECS", 60);
    let store: Arc<dyn RateLimitStore> = match RedisRateLimitStore::connect(redis_url).await {
        Ok(s) => {
            tracing::info!("rate limit store: redis");
            Arc::new(s)
        }
        Err(e) => {
            tracing::warn!(error = %e, "redis rate-limit store unavailable; fallback to in-memory");
            Arc::new(MemoryStore::new())
        }
    };
    RateLimitLayer::new(max, Duration::from_secs(window_secs as u64))
        .with_store(store)
        // 优先按租户（x-tenant-id），否则按来源 IP（ConnectInfo 由 HttpServer 填充）
        .with_key_fn(|req: &axum::http::Request<axum::body::Body>| {
            if let Some(t) = req
                .headers()
                .get("x-tenant-id")
                .and_then(|v| v.to_str().ok())
            {
                return format!("tenant:{t}");
            }
            match req
                .extensions()
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            {
                Some(axum::extract::ConnectInfo(addr)) => format!("ip:{}", addr.ip()),
                None => "global".into(),
            }
        })
}

/// 登录防爆破：按来源 IP 10 次/分钟（LOGIN_RATE_LIMIT_MAX 可配），
/// 独立内存存储，Redis 故障不影响登录可用性。
fn login_rate_limit() -> RateLimitLayer<axum::body::Body> {
    let max = env_u32("LOGIN_RATE_LIMIT_MAX", 10);
    let window_secs = env_u32("LOGIN_RATE_LIMIT_WINDOW_SECS", 60);
    RateLimitLayer::new(max, Duration::from_secs(window_secs as u64))
        .with_store(Arc::new(MemoryStore::new()))
        .with_key_fn(|req: &axum::http::Request<axum::body::Body>| {
            match req
                .extensions()
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            {
                Some(axum::extract::ConnectInfo(addr)) => format!("login:{}", addr.ip()),
                None => "login:global".into(),
            }
        })
}

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// 限流超限 → 429；其余服务错误 → 500（详情只进日志）。
async fn rate_limit_error(
    e: Box<dyn std::error::Error + Send + Sync>,
) -> axum::response::Response {
    if e.is::<ecat_middleware::RateLimitError>() {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "rate limit exceeded",
        )
            .into_response();
    }
    tracing::warn!(error = %e, "rate limit service error");
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "internal error",
    )
        .into_response()
}
