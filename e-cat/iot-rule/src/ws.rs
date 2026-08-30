use crate::push::PushHub;
use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: String,
}

/// 校验 JWT（与网关同一 JWT_SECRET），sub 即租户；失败 401。
/// 浏览器 WS 无法携带自定义 header，token 走 query 参数。
fn verify_token(token: &str) -> Result<String, String> {
    let secret = std::env::var("JWT_SECRET").map_err(|_| "JWT_SECRET not set")?;
    let mut validation = jsonwebtoken::Validation::default();
    // 开发/演示 token 可不带 exp/nbf，只校验签名与 sub
    validation.required_spec_claims = std::collections::HashSet::new();
    let data = jsonwebtoken::decode::<ecat_auth::AuthClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| format!("bad token: {e}"))?;
    Ok(data.claims.sub.clone())
}

/// GET /ws?token=<JWT> —— 直连端点（网关不代理 WebSocket，P5 前端直连 8084）。
pub async fn ws_handler(
    State(hub): State<PushHub>,
    Query(q): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let tenant = match verify_token(&q.token) {
        Ok(t) => t,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": e}))).into_response(),
    };
    let rx = hub.subscribe(&tenant);
    ws.on_upgrade(move |socket| on_socket(socket, rx))
}

async fn on_socket(mut socket: WebSocket, mut rx: tokio::sync::broadcast::Receiver<crate::models::AlertMessage>) {
    // 只推送告警，不回显其他消息；发送失败（客户端断开）即退出
    while let Ok(msg) = rx.recv().await {
        let text = serde_json::to_string(&msg).unwrap_or_default();
        if socket.send(Message::Text(text.into())).await.is_err() {
            break;
        }
    }
}
