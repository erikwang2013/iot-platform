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

/// GET /ws?token=<JWT> —— 直连端点（网关不代理 WebSocket，P5 前端直连 8084）。
/// 浏览器 WS 无法携带自定义 header，token 走 query 参数。
/// JWT 校验收编至框架 ecat_auth::verify_token（与网关同一 JWT_SECRET），sub 即租户；失败 401。
pub async fn ws_handler(
    State(hub): State<PushHub>,
    Query(q): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let tenant = match std::env::var("JWT_SECRET")
        .map_err(|_| "server error".to_string())
        .and_then(|secret| ecat_auth::verify_token(&q.token, &secret))
    {
        Ok(c) => c.sub.clone(),
        Err(e) => {
            // 细节只进日志，不回给客户端（防内部实现/签名细节泄露）
            tracing::warn!(error = %e, "ws jwt validation failed");
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid token"}))).into_response();
        }
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
