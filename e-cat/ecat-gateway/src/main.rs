use axum::{Router, response::IntoResponse, routing::{delete, get, post, put}};
use ecat_auth::JwtAuthCompat;
use ecat_health::HealthRegistry;
use ecat_gateway::{
    api_version::ApiVersionLayer,
    audit::MysqlAuditSink,
    proxy::{
        ProxyState, access_proxy, access_proxy_open, auth_proxy, cdn_proxy, console_proxy,
        data_proxy, device_proxy, rule_proxy,
    },
};
use ecat_middleware::{
    AuditState, MemoryStore, RateLimitLayer, RateLimitStore, RedisRateLimitStore,
    audit_middleware,
};
use ecat_security::SecurityBodyCompatLayer;
use std::sync::Arc;
use std::time::Duration;

async fn submit() -> &'static str {
    "ok"
}

/// GET /api/open/openapi.json：返回只读端点的 OpenAPI 3.0 文档。
async fn openapi_doc() -> axum::Json<serde_json::Value> {
    let spec = ecat_gateway::openapi::read_only_spec();
    axum::Json(serde_json::to_value(&spec).unwrap_or_else(|_| serde_json::json!({})))
}

/// 值级 RBAC 角色表（与登录签发的 role claim 对齐）：
/// 读操作（GET/HEAD/OPTIONS）三角色均可；写操作 admin/operator；
/// 租户/用户/固件管理仅 admin。
const ROLES_ALL: &[&str] = &["admin", "operator", "read-only"];
const ROLES_WRITE: &[&str] = &["admin", "operator"];
const ROLES_ADMIN: &[&str] = &["admin"];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-key-0123456789abcdefghijklmn".into());
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".into());

    let proxy_state = ProxyState {
        client: reqwest::Client::new(),
        resolver: ecat_gateway::service::ServiceResolver::new(),
    };

    // 全局限流与登录限流共享同一存储（Redis 优先，多实例计数一致；
    // 不可用时降级内存 fail-open，可用性优先，降级有日志告警）
    let rate_store = rate_limit_store(&redis_url).await;

    // 审计 sink：管理面写操作（POST/PUT/PATCH/DELETE）落库 audit_log。
    // MySQL 不可用时降级空实现（日志告警）——审计属可观测性增强，不阻塞启动。
    let audit_dsn = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://iot:iot@localhost:3306/iot".into());
    let audit_state = AuditState(Arc::new(MysqlAuditSink::connect(&audit_dsn).await));

    // /api/access/* 公开路径：OAuth 回调、涂鸦 Webhook、开放 API 换 token
    // （无 JWT，浏览器/厂商服务器/开放客户端直连）
    let access_public = Router::new()
        .route("/oauth/callback", get(access_proxy_open))
        .route("/webhook/tuya", post(access_proxy_open))
        .route("/open/token", post(access_proxy_open))
        .with_state(proxy_state.clone());
    // /api/access/* 受保护路径：JWT 校验后透传租户（AuthClaims.sub → x-tenant-id）
    let access_admin = Router::new()
        .route("/oauth/authorize-url", post(access_proxy))
        .route("/vendors/{vendor}/import", post(access_proxy))
        .route("/devices/{device_id}/command", post(access_proxy))
        // 全写操作：设备命令/厂商导入属于"设备操作"，operator 可做
        // 审计层在 JWT 内层：JWT 先注入 AuthClaims，写操作据此记租户/角色；
        // 401/403 短路（不产生业务事件），与"仅审计成功会话"语义一致。
        .layer(axum::middleware::from_fn_with_state(
            audit_state.clone(),
            audit_middleware,
        ))
        .layer(
            JwtAuthCompat::new(&secret, &["sub", "role"])?
                .role_policy(ROLES_ALL, ROLES_WRITE),
        )
        .with_state(proxy_state.clone());
    // /api/data/* 受保护路径：历史曲线 / 导出（GET，query 透传）
    let data_admin = Router::new()
        .route("/data/history", get(data_proxy))
        .route("/data/export", get(data_proxy))
        // 审计层在 JWT 内层：JWT 先注入 AuthClaims，写操作据此记租户/角色；
        // 401/403 短路（不产生业务事件），与"仅审计成功会话"语义一致。
        .layer(axum::middleware::from_fn_with_state(
            audit_state.clone(),
            audit_middleware,
        ))
        .layer(
            JwtAuthCompat::new(&secret, &["sub", "role"])?
                .role_policy(ROLES_ALL, &[]),
        )
        .with_state(proxy_state.clone());
    // /api/rule/* 受保护路径：规则 CRUD / 告警记录（REST，WS 直连 8084 不走网关）
    let rule_admin = Router::new()
        .route("/rule/rules", get(rule_proxy).post(rule_proxy))
        .route("/rule/rules/{id}", put(rule_proxy).delete(rule_proxy))
        .route("/rule/alerts", get(rule_proxy))
        .route("/rule/alerts/{id}/ack", post(rule_proxy))
        .route("/rule/stats", get(rule_proxy))
        .route("/rule/channels", get(rule_proxy))
        .route("/rule/channels/{channel}", put(rule_proxy).delete(rule_proxy))
        // 审计层在 JWT 内层：JWT 先注入 AuthClaims，写操作据此记租户/角色；
        // 401/403 短路（不产生业务事件），与"仅审计成功会话"语义一致。
        .layer(axum::middleware::from_fn_with_state(
            audit_state.clone(),
            audit_middleware,
        ))
        .layer(
            JwtAuthCompat::new(&secret, &["sub", "role"])?
                .role_policy(ROLES_ALL, ROLES_WRITE),
        )
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
        // 审计层在 JWT 内层：JWT 先注入 AuthClaims，写操作据此记租户/角色；
        // 401/403 短路（不产生业务事件），与"仅审计成功会话"语义一致。
        .layer(axum::middleware::from_fn_with_state(
            audit_state.clone(),
            audit_middleware,
        ))
        .layer(
            JwtAuthCompat::new(&secret, &["sub", "role"])?
                .role_policy(ROLES_ALL, ROLES_WRITE),
        )
        .with_state(proxy_state.clone());

    // 管理面（/api/*）：租户/用户/物模型 → iot-access；设备/OTA → iot-device。
    // 双面都要求 sub+role：客户端登录 token 也带 role（P5 401 根因），
    // 客户端设备列表走 /api/devices 同样受此校验。
    // 租户/用户管理仅 admin（读三角色均可）；物模型属设备能力定义，operator 可写
    let console_admin = Router::new()
        .route("/tenants", get(console_proxy).post(console_proxy))
        .route("/tenants/{id}", delete(console_proxy))
        .route("/users", get(console_proxy).post(console_proxy))
        .route("/users/{id}", delete(console_proxy))
        .route("/audit", get(console_proxy))
        .route("/api-keys", get(console_proxy).post(console_proxy))
        .route("/api-keys/{id}", delete(console_proxy))
        // 审计层在 JWT 内层：JWT 先注入 AuthClaims，写操作据此记租户/角色；
        // 401/403 短路（不产生业务事件），与"仅审计成功会话"语义一致。
        .layer(axum::middleware::from_fn_with_state(
            audit_state.clone(),
            audit_middleware,
        ))
        .layer(
            JwtAuthCompat::new(&secret, &["sub", "role"])?
                .role_policy(ROLES_ALL, ROLES_ADMIN),
        )
        .with_state(proxy_state.clone());
    let models_admin = Router::new()
        .route("/models/things", get(console_proxy).post(console_proxy))
        .route("/models/things/{id}", get(console_proxy).delete(console_proxy))
        // 审计层在 JWT 内层：JWT 先注入 AuthClaims，写操作据此记租户/角色；
        // 401/403 短路（不产生业务事件），与"仅审计成功会话"语义一致。
        .layer(axum::middleware::from_fn_with_state(
            audit_state.clone(),
            audit_middleware,
        ))
        .layer(
            JwtAuthCompat::new(&secret, &["sub", "role"])?
                .role_policy(ROLES_ALL, ROLES_WRITE),
        )
        .with_state(proxy_state.clone());
    // 设备生命周期 operator 可写；OTA 固件管理仅 admin
    let device_admin = Router::new()
        .route("/devices", get(device_proxy))
        .route("/devices/stats", get(device_proxy))
        .route("/devices/groups", get(device_proxy).post(device_proxy))
        .route("/devices/groups/{id}", delete(device_proxy))
        .route(
            "/devices/groups/{id}/members",
            post(device_proxy).delete(device_proxy),
        )
        .route("/devices/batch", post(device_proxy))
        .route("/devices/{id}", put(device_proxy).delete(device_proxy))
        .route("/devices/{id}/unbind", post(device_proxy))
        .route("/devices/{id}/tags", get(device_proxy))
        // 审计层在 JWT 内层：JWT 先注入 AuthClaims，写操作据此记租户/角色；
        // 401/403 短路（不产生业务事件），与"仅审计成功会话"语义一致。
        .layer(axum::middleware::from_fn_with_state(
            audit_state.clone(),
            audit_middleware,
        ))
        .layer(
            JwtAuthCompat::new(&secret, &["sub", "role"])?
                .role_policy(ROLES_ALL, ROLES_WRITE),
        )
        .with_state(proxy_state.clone());
    let ota_admin = Router::new()
        .route("/ota/firmwares", get(device_proxy).post(device_proxy))
        .route("/ota/firmwares/{id}", delete(device_proxy))
        .route("/ota/tasks", get(device_proxy).post(device_proxy))
        .route("/ota/tasks/{id}/report", post(device_proxy))
        // 审计层在 JWT 内层：JWT 先注入 AuthClaims，写操作据此记租户/角色；
        // 401/403 短路（不产生业务事件），与"仅审计成功会话"语义一致。
        .layer(axum::middleware::from_fn_with_state(
            audit_state.clone(),
            audit_middleware,
        ))
        .layer(
            JwtAuthCompat::new(&secret, &["sub", "role"])?
                .role_policy(ROLES_ALL, ROLES_ADMIN),
        )
        .with_state(proxy_state.clone());
    // 登录端点：管理端 /api/auth/login 与客户端 /admin/auth/login 同一 handler，
    // 独立更严的按 IP 限流（10 次/分钟）防爆破；全局 100 次/分钟限流仍叠加生效。
    // 与全局限流共享 Redis 存储：多实例下登录失败计数一致（防爆破不因扩容失效）
    let login_router = Router::new()
        .route("/auth/login", post(auth_proxy))
        .layer(
            tower::ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(rate_limit_error))
                .layer(login_rate_limit(rate_store.clone())),
        )
        .with_state(proxy_state);

    let router = Router::new()
        .merge(HealthRegistry::new().into_router())
        .route("/api/ping", get(|| async { "pong" }))
        .route("/api/submit", post(submit))
        // OpenAPI 3.0 文档（A-4）：只读端点描述，公开只读，无鉴权（纯文档）
        .route("/api/open/openapi.json", get(openapi_doc))
        .nest("/api/access", access_public)
        .nest("/api/access", access_admin)
        .nest("/api", data_admin)
        .nest("/api", rule_admin)
        .nest("/api", cdn_admin)
        .nest("/api", console_admin)
        .nest("/api", models_admin)
        .nest("/api", device_admin)
        .nest("/api", ota_admin)
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
                .layer(rate_limit(rate_store))
                .into_inner(),
        );

    let srv = {
        let mut srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
        // B-5 TLS：配置 TLS_CERT + TLS_KEY（PEM）时启用 HTTPS（ingress 前的本地终结也可用）
        if let (Ok(cert), Ok(key)) = (std::env::var("TLS_CERT"), std::env::var("TLS_KEY")) {
            let tls = ecat_transport::TlsConfig::new(cert, key);
            srv = srv.tls(tls);
            tracing::info!("gateway TLS enabled");
        }
        srv
    };
    let mut app = ecat::App::builder()
        .name("iot-gateway")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}

/// Redis 限流存储，不可用时降级内存（fail-open，与 store 语义一致）。
/// 全局与登录限流共享同一 Arc<dyn RateLimitStore>，多实例计数一致。
async fn rate_limit_store(redis_url: &str) -> Arc<dyn RateLimitStore> {
    match RedisRateLimitStore::connect(redis_url).await {
        Ok(s) => {
            tracing::info!("rate limit store: redis");
            Arc::new(s)
        }
        Err(e) => {
            tracing::warn!(error = %e, "redis rate-limit store unavailable; fallback to in-memory");
            Arc::new(MemoryStore::new())
        }
    }
}

fn rate_limit(store: Arc<dyn RateLimitStore>) -> RateLimitLayer<axum::body::Body> {
    let max = env_u32("RATE_LIMIT_MAX", 100);
    let window_secs = env_u32("RATE_LIMIT_WINDOW_SECS", 60);
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

/// 登录防爆破：按来源 IP 10 次/分钟（LOGIN_RATE_LIMIT_MAX 可配）。
/// 与全局限流共享 Redis 存储（多实例登录失败计数一致，防爆破不因扩容失效）；
/// Redis 不可用时降级内存 fail-open——登录可用性优先，爆破防护降级由日志告警。
fn login_rate_limit(store: Arc<dyn RateLimitStore>) -> RateLimitLayer<axum::body::Body> {
    let max = env_u32("LOGIN_RATE_LIMIT_MAX", 10);
    let window_secs = env_u32("LOGIN_RATE_LIMIT_WINDOW_SECS", 60);
    RateLimitLayer::new(max, Duration::from_secs(window_secs as u64))
        .with_store(store)
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
