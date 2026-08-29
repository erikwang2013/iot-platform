use crate::events::{publish_event, shadow_apply};
use crate::models::EventMessage;
use crate::store::Store;
use axum::{
    Json,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use ecat_data_redis::RedisCache;
use ecat_mq_kafka::KafkaMq;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

/// 涂鸦 Webhook 原始结构：type/bizCode/data（data 可能为 JSON 字符串或对象）。
#[derive(Deserialize)]
pub struct WebhookPayload {
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(rename = "bizCode")]
    pub biz_code: String,
    pub data: Value,
}

/// 归一化：data 解包（字符串→内层 JSON），bizCode → EventMessage.kind。
/// 返回 Err 表示不支持的 bizCode（如 delete），调用方直接丢弃。
pub fn normalize_event(
    platform_id: &str,
    tenant_id: &str,
    p: &WebhookPayload,
) -> Result<EventMessage, String> {
    let data = match &p.data {
        Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
        other => other.clone(),
    };
    let code = data["code"].as_str().unwrap_or("").to_string();
    let value = data["value"].clone();
    let ts = data["ts"].as_i64().unwrap_or_else(now_ms);
    let (kind, ev_code, ev_value) = match p.biz_code.as_str() {
        "report" => ("property", code, value),
        "online" => ("online", "online".to_string(), json!(true)),
        "offline" => ("offline", "offline".to_string(), json!(false)),
        other => return Err(format!("unsupported bizCode: {other}")),
    };
    Ok(EventMessage {
        device_id: platform_id.to_string(),
        tenant_id: tenant_id.to_string(),
        kind: kind.to_string(),
        code: ev_code,
        value: ev_value,
        ts,
    })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[derive(Clone)]
pub struct WebhookState {
    pub store: Arc<Store>,
    pub kafka: Arc<KafkaMq>,
    pub redis: Arc<RedisCache>,
}

/// POST /api/access/webhook/tuya（公开：涂鸦服务器回调，无 JWT）。
pub async fn receive(
    State(ws): State<WebhookState>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let raw = match to_bytes(body, 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(json!({"error": "body too large"})),
            )
                .into_response()
        }
    };
    let p: WebhookPayload = match serde_json::from_slice(&raw) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("bad payload: {e}")})),
            )
                .into_response()
        }
    };
    let device_id = match extract_device_id(&p) {
        Some(d) => d,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "no deviceId in data"})),
            )
                .into_response()
        }
    };
    // 涂鸦设备 ID → 平台设备 + 租户
    let platform_id = match ws
        .store
        .find_device_by_vendor_id("tuya", &device_id)
        .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::warn!(device_id, "webhook event for unknown device, dropped");
            return (StatusCode::OK, Json(json!({"accepted": false}))).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e})),
            )
                .into_response()
        }
    };
    let tenant_id = match ws.store.tenant_of_device(&platform_id).await {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e})),
            )
                .into_response()
        }
    };
    // 签名校验：header 存在则必须通过（HMAC-SHA256(raw body, client_secret)）
    if let Some(sig) = headers
        .get("x-tuya-signature")
        .and_then(|v| v.to_str().ok())
    {
        let secret = match ws.store.load_creds(&tenant_id, "tuya").await {
            Ok(c) => c["client_secret"].as_str().unwrap_or("").to_string(),
            Err(_) => String::new(),
        };
        if !secret.is_empty() && !verify_signature(&secret, &raw, sig) {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "bad tuya signature"})),
            )
                .into_response();
        }
    }
    let ev = match normalize_event(&platform_id, &tenant_id, &p) {
        Ok(ev) => ev,
        Err(_) => return (StatusCode::OK, Json(json!({"accepted": false}))).into_response(),
    };
    if let Err(e) = publish_event(&ws.kafka, &ev).await {
        tracing::error!(error = %e, "kafka publish failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("kafka: {e}")})),
        )
            .into_response();
    }
    if let Err(e) = shadow_apply(&ws.redis, &ev).await {
        tracing::warn!(error = %e, "shadow update failed");
    }
    (StatusCode::OK, Json(json!({"accepted": true}))).into_response()
}

fn extract_device_id(p: &WebhookPayload) -> Option<String> {
    let data = match &p.data {
        Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
        other => other.clone(),
    };
    data["deviceId"]
        .as_str()
        .or_else(|| data["device_id"].as_str())
        .map(str::to_string)
}

fn verify_signature(secret: &str, raw: &[u8], sig: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(raw);
    hex::encode(mac.finalize().into_bytes()) == sig
}

pub fn router(ws: WebhookState) -> axum::Router {
    axum::Router::new()
        .route("/webhook/tuya", axum::routing::post(receive))
        .with_state(ws)
}
