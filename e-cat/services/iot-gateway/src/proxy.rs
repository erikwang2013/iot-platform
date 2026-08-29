use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use ecat_auth::AuthClaims;

/// iot-access 内部地址（生产走服务发现，P1 直连；ACCESS_BASE 环境变量可覆盖，
/// 供端口冲突的本地环境用）。
const ACCESS_BASE: &str = "http://localhost:8082";

#[derive(Clone)]
pub struct ProxyState {
    pub client: reqwest::Client,
}

/// 受保护转发：JwtAuthCompat 已把 AuthClaims 放入 extensions（ecat-auth jwt.rs），
/// 取其 sub 作为租户注入 x-tenant-id。claims 缺失时租户为空，上游会以 401 拒绝（fail-closed）。
pub async fn access_proxy(State(ps): State<ProxyState>, req: Request) -> Response {
    let tenant = req
        .extensions()
        .get::<AuthClaims>()
        .map(|c| c.sub.clone())
        .unwrap_or_default();
    forward(&ps, req, &tenant).await
}

/// 公开转发（OAuth 回调 / 涂鸦 webhook）：不注入租户。
pub async fn access_proxy_open(State(ps): State<ProxyState>, req: Request) -> Response {
    forward(&ps, req, "").await
}

/// 原样转发到 iot-access：方法/路径/body 透传（iot-access 侧全部按原方法路由），
/// 注入 x-gateway-secret（iot-access 的 tenant_from_header 门禁校验，见其 main.rs）；
/// 受保护路径额外注入 x-tenant-id。上游失败 → 502。
async fn forward(ps: &ProxyState, req: Request, tenant: &str) -> Response {
    // axum nest 会剥掉 /api/access 前缀，handler 里看到的路径以 /oauth/... 开头，转发时补回
    let path = format!("/api/access{}", req.uri().path());
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
    let base = std::env::var("ACCESS_BASE").unwrap_or_else(|_| ACCESS_BASE.into());
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
    if !tenant.is_empty() {
        rb = rb.header("x-tenant-id", tenant);
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
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes.to_vec()))
        .unwrap()
}
