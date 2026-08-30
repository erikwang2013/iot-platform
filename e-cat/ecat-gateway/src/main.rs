use axum::{Router, response::IntoResponse, routing::{get, post, put}};
use ecat_auth::JwtAuthCompat;
use ecat_health::HealthRegistry;
use ecat_gateway::{
    api_version::ApiVersionLayer,
    proxy::{ProxyState, access_proxy, access_proxy_open, cdn_proxy, data_proxy, rule_proxy},
};
use ecat_middleware::{MemoryStore, RateLimitLayer, RateLimitStore, RedisRateLimitStore};
use ecat_security::SecurityBodyCompatLayer;
use std::sync::Arc;
use std::time::Duration;

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
        .nest("/api", cdn_admin)
        .nest("/api", admin_api)
        .nest("/admin", client_api)
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
