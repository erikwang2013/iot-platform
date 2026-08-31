use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use crate::service::ServiceResolver;
use ecat_auth::AuthClaims;

/// iot-access 内部地址（生产走服务发现，P1 直连；ACCESS_BASE 环境变量可覆盖，
/// 供端口冲突的本地环境用）。
const ACCESS_BASE: &str = "http://localhost:8082";
/// iot-data 内部地址（DATA_BASE 环境变量可覆盖）。
const DATA_BASE: &str = "http://localhost:8083";
/// iot-rule 内部地址（RULE_BASE 环境变量可覆盖）。
const RULE_BASE: &str = "http://localhost:8084";
/// iot-cdn 内部地址（CDN_BASE 环境变量可覆盖）。
const CDN_BASE: &str = "http://localhost:8085";
/// iot-device 内部地址（DEVICE_BASE 环境变量可覆盖）。
const DEVICE_BASE: &str = "http://localhost:8081";

#[derive(Clone)]
pub struct ProxyState {
    pub client: reqwest::Client,
    /// 服务发现（B-3）：按服务名解析上游 base URL，env/默认兜底。
    pub resolver: ServiceResolver,
}

fn claims_of(req: &Request) -> Option<AuthClaims> {
    req.extensions().get::<AuthClaims>().cloned()
}

/// 受保护转发：JwtAuthCompat 已把 AuthClaims 放入 extensions（ecat-auth jwt.rs），
/// 取其 sub 作为租户注入 x-tenant-id、role 注入 x-tenant-role。
/// claims 缺失时租户为空，上游会以 401 拒绝（fail-closed）。
pub async fn access_proxy(State(ps): State<ProxyState>, req: Request) -> Response {
    let claims = claims_of(&req);
    forward(&ps, req, claims.as_ref(), "/api/access", "ACCESS_BASE", "iot-access", ACCESS_BASE).await
}

/// 公开转发（OAuth 回调 / 涂鸦 webhook / 登录）：不注入租户。
pub async fn access_proxy_open(State(ps): State<ProxyState>, req: Request) -> Response {
    forward(&ps, req, None, "/api/access", "ACCESS_BASE", "iot-access", ACCESS_BASE).await
}

/// 登录公开代理：管理端 /api/auth/login 与客户端 /admin/auth/login 均转到
/// iot-access 的 /api/auth/login 或 /admin/auth/login（prefix 由挂载点决定）。
pub async fn auth_proxy(State(ps): State<ProxyState>, req: Request) -> Response {
    // handler 看到的剩余路径是 /auth/login；挂载前缀决定补回 /api 还是 /admin
    let prefix = req
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .map(|u| if u.path().starts_with("/admin") { "/admin" } else { "/api" })
        .unwrap_or("/api");
    forward(&ps, req, None, prefix, "ACCESS_BASE", "iot-access", ACCESS_BASE).await
}

/// /api/data/* 受保护转发（GET）：JWT sub → x-tenant-id + x-gateway-secret，
/// query 原样透传（history/export 由 iot-data 从 Query 读取）。
pub async fn data_proxy(State(ps): State<ProxyState>, req: Request) -> Response {
    let claims = claims_of(&req);
    forward(&ps, req, claims.as_ref(), "/api", "DATA_BASE", "iot-data", DATA_BASE).await
}

/// /api/rule/* 受保护转发：JWT sub → x-tenant-id + x-gateway-secret，
/// 路径/query/body 原样透传（rules/alerts CRUD 由 iot-rule 处理）。
pub async fn rule_proxy(State(ps): State<ProxyState>, req: Request) -> Response {
    // 与 data_proxy 同构：nest 在 /api 下、路由为 /rule/...，handler 看到的
    // 路径是 /rule/rules 等剩余段，prefix "/api" 补回后即 /api/rule/rules
    let claims = claims_of(&req);
    forward(&ps, req, claims.as_ref(), "/api", "RULE_BASE", "iot-rule", RULE_BASE).await
}

/// /api/cdn/* 受保护转发：JWT sub → x-tenant-id + x-gateway-secret，
/// 供应商 CRUD/启停/刷新预热/签名 URL 由 iot-cdn 处理。
pub async fn cdn_proxy(State(ps): State<ProxyState>, req: Request) -> Response {
    let claims = claims_of(&req);
    forward(&ps, req, claims.as_ref(), "/api", "CDN_BASE", "iot-cdn", CDN_BASE).await
}

/// /api/tenants、/api/users、/api/models/* 受保护转发 → iot-access。
pub async fn console_proxy(State(ps): State<ProxyState>, req: Request) -> Response {
    let claims = claims_of(&req);
    forward(&ps, req, claims.as_ref(), "/api", "ACCESS_BASE", "iot-access", ACCESS_BASE).await
}

/// /api/devices/*、/api/ota/* 受保护转发 → iot-device。
pub async fn device_proxy(State(ps): State<ProxyState>, req: Request) -> Response {
    let claims = claims_of(&req);
    forward(&ps, req, claims.as_ref(), "/api", "DEVICE_BASE", "iot-device", DEVICE_BASE).await
}

/// 原样转发到上游服务：方法/路径/query/body 透传，注入 x-gateway-secret
/// （上游 tenant_from_header 门禁校验）+ 受保护路径的 x-tenant-id / x-tenant-role。
/// 上游 base 由服务发现（B-3）解析，env/默认兜底。上游失败 → 502。
async fn forward(
    ps: &ProxyState,
    req: Request,
    claims: Option<&AuthClaims>,
    path_prefix: &str,
    base_env: &str,
    service_name: &str,
    default_base: &str,
) -> Response {
    // axum nest 会剥掉挂载前缀，handler 里看到的路径是剩余部分，转发时补回；
    // query 也要透传（OAuth callback 的 ?code=&state= 由上游从 Query 读取）
    let mut path = format!("{path_prefix}{}", req.uri().path());
    if let Some(q) = req.uri().query() {
        path.push('?');
        path.push_str(q);
    }
    let method = req.method().clone();
    let content_type = req.headers().get(header::CONTENT_TYPE).cloned();
    // 只透传白名单头（x-tuya-signature 供涂鸦 webhook 验签用）；
    // 不复制全部请求头，避免客户端伪造 x-tenant-id / x-gateway-secret。
    let tuya_sig = req.headers().get("x-tuya-signature").cloned();
    let raw = match to_bytes(req.into_body(), 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                axum::Json(serde_json::json!({"error": "body too large"})),
            )
                .into_response()
        }
    };
    // 服务发现解析（env → Consul → 默认），见 ServiceResolver::resolve
    let base = ps
        .resolver
        .resolve(base_env, service_name)
        .await
        .unwrap_or_else(|| default_base.into());
    let mut rb = ps.client.request(method, format!("{base}{path}"));
    if let Some(ct) = content_type {
        rb = rb.header(header::CONTENT_TYPE, ct);
    }
    if let Some(sig) = tuya_sig {
        rb = rb.header("x-tuya-signature", sig);
    }
    if let Ok(secret) = std::env::var("IOT_GATEWAY_SECRET") {
        rb = rb.header("x-gateway-secret", secret);
    }
    if let Some(claims) = claims {
        rb = rb.header("x-tenant-id", &claims.sub);
        if let Some(role) = &claims.role {
            rb = rb.header("x-tenant-role", role);
        }
    }
    let resp = match rb.body(raw).send().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({"error": format!("upstream: {e}")})),
            )
                .into_response()
        }
    };
    let status = resp.status();
    let content_type = resp.headers().get(header::CONTENT_TYPE).cloned();
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({"error": format!("upstream read: {e}")})),
            )
                .into_response()
        }
    };
    let mut rb = Response::builder().status(status);
    if let Some(ct) = content_type {
        rb = rb.header(header::CONTENT_TYPE, ct);
    }
    rb.body(Body::from(bytes.to_vec())).unwrap()
}
