use crate::adapter::VendorCreds;
use crate::store::Store;
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// state = base64url(tenant_id:vendor)，回调时还原租户归属。
pub fn encode_state(tenant_id: &str, vendor: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(format!("{tenant_id}:{vendor}"))
}

pub fn decode_state(state: &str) -> Result<(String, String), String> {
    use base64::Engine;
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(state)
        .map_err(|e| format!("bad state: {e}"))?;
    let s = String::from_utf8(raw).map_err(|e| format!("bad state: {e}"))?;
    let (t, v) = s.split_once(':').ok_or_else(|| "bad state".to_string())?;
    Ok((t.to_string(), v.to_string()))
}

#[derive(Clone)]
pub struct OauthState {
    pub store: Arc<Store>,
    /// 涂鸦开放平台 client_id（授权 URL 用）
    pub tuya_client_id: String,
    /// 授权完成后浏览器跳回的地址（含 /api/access/oauth/callback）
    pub callback_base: String,
}

#[derive(Deserialize)]
pub struct AuthorizeReq {
    pub vendor: String,
}

#[derive(Serialize)]
pub struct AuthorizeResp {
    pub url: String,
}

/// POST /api/access/oauth/authorize-url（受保护：需 x-tenant-id）。
/// axum 0.8 无 Extension 提取器，直接读请求头。
pub async fn authorize_url(
    State(oauth): State<OauthState>,
    headers: HeaderMap,
    Json(req): Json<AuthorizeReq>,
) -> Result<Json<AuthorizeResp>, (StatusCode, String)> {
    let tenant_id = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "missing x-tenant-id".to_string()))?
        .to_string();
    if req.vendor != "tuya" {
        return Err((StatusCode::BAD_REQUEST, format!("vendor {} not supported", req.vendor)));
    }
    let state = encode_state(&tenant_id, &req.vendor);
    let url = format!(
        "https://openapi.tuyacn.com/oauth2/auth?client_id={}&response_type=code&redirect_uri={}&state={}",
        oauth.tuya_client_id, oauth.callback_base, state
    );
    Ok(Json(AuthorizeResp { url }))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

/// GET /api/access/oauth/callback（公开：浏览器从涂鸦跳回，无 JWT）
/// 用授权码换 token 并加密落库；返回 HTML 提示可关闭窗口。
/// axum 0.8 移除了 Html 响应类型，用 text/html 头 + 字符串代替。
pub async fn callback(
    State(oauth): State<OauthState>,
    Query(q): Query<CallbackQuery>,
) -> Result<impl axum::response::IntoResponse, (StatusCode, String)> {
    let (tenant_id, vendor) = decode_state(&q.state).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    if vendor != "tuya" {
        return Err((StatusCode::BAD_REQUEST, "unsupported vendor in state".into()));
    }
    let creds = exchange_authorization_code(&q.code, &oauth.tuya_client_id)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    oauth
        .store
        .save_creds(&tenant_id, "tuya", &serde_json::to_value(&creds).unwrap())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok((
        [(axum::http::header::CONTENT_TYPE, "text/html")],
        "<html><body><h2>授权成功，可关闭此窗口</h2></body></html>",
    ))
}

/// 调涂鸦 token 端点换授权码（真实环境走 openapi.tuyacn.com；测试指向 mock）。
pub async fn exchange_authorization_code(
    code: &str,
    client_id: &str,
) -> Result<VendorCreds, String> {
    let base = std::env::var("TUYA_OPENAPI_BASE").unwrap_or_else(|_| "https://openapi.tuyacn.com".into());
    let client_secret = std::env::var("TUYA_CLIENT_SECRET").map_err(|_| "TUYA_CLIENT_SECRET not set".to_string())?;
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string();
    // 取 token 时签名不携带 access_token（约定为空串）
    let sign = crate::adapters::tuya::sign(&client_id, &t, "", &client_secret);
    let url = format!(
        "{base}/v1.0/token?grant_type=authorization_code&code={code}"
    );
    let resp = reqwest::Client::new()
        .get(&url)
        .header("client_id", client_id)
        .header("t", &t)
        .header("sign_method", "HMAC-SHA256")
        .header("sign", sign)
        .send()
        .await
        .map_err(|e| format!("tuya token request: {e}"))?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("tuya token parse: {e}"))?;
    if body["success"] != true {
        return Err(format!("tuya token error: {body}"));
    }
    let r = &body["result"];
    let expires_in = r["expire_time"].as_i64().unwrap_or(2592000);
    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + expires_in;
    Ok(VendorCreds {
        client_id: client_id.to_string(),
        client_secret,
        uid: r["uid"].as_str().unwrap_or("").to_string(),
        access_token: r["access_token"].as_str().unwrap_or("").to_string(),
        refresh_token: r["refresh_token"].as_str().unwrap_or("").to_string(),
        expires_at,
    })
}

/// 受保护路由（authorize-url 需 x-tenant-id）。
pub fn router(oauth: OauthState) -> axum::Router {
    axum::Router::new()
        .route("/oauth/authorize-url", axum::routing::post(authorize_url))
        .with_state(oauth)
}

/// 公开路由（浏览器从涂鸦跳回，无 JWT/tenant）。
pub fn router_public(oauth: OauthState) -> axum::Router {
    axum::Router::new()
        .route("/oauth/callback", axum::routing::get(callback))
        .with_state(oauth)
}
