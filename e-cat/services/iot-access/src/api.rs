use crate::adapter::adapter_for;
use crate::store::Store;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use ecat_data_redis::RedisCache;
use ecat_mq_kafka::KafkaMq;
use ecat_mq_mqtt::MqttMq;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiState {
    pub store: Arc<Store>,
    pub kafka: Arc<KafkaMq>,
    pub redis: Arc<RedisCache>,
    pub mqtt: Arc<MqttMq>,
}

/// 构造受保护 API 路由（挂载见 main.rs，路径前缀 /api/access）。
pub fn router(api: ApiState) -> axum::Router {
    axum::Router::new()
        .route("/vendors/{vendor}/import", axum::routing::post(import_devices))
        .route("/devices/{device_id}/command", axum::routing::post(send_command))
        .with_state(api)
}

/// POST /api/access/vendors/{vendor}/import（受保护）
/// 拉取厂商设备列表 → 入库 devices + device_links。
pub async fn import_devices(
    State(api): State<ApiState>,
    axum::Extension(tenant_id): axum::Extension<String>,
    Path(vendor): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let adapter = adapter_for(&vendor).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let creds = api
        .store
        .load_creds(&tenant_id, &vendor)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let creds: crate::adapter::VendorCreds =
        serde_json::from_value(creds).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let devices = adapter
        .list_devices(&creds)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    let mut imported = Vec::new();
    for d in &devices {
        let platform_id = api
            .store
            .upsert_device(&tenant_id, &vendor, &d.vendor_id, &d.name, &d.category, d.online)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        imported.push(json!({ "platform_id": platform_id, "vendor_id": d.vendor_id, "name": d.name }));
    }
    Ok(Json(json!({ "imported": imported, "count": imported.len() })))
}

#[derive(Deserialize)]
pub struct CommandReq {
    pub code: String,
    pub value: Value,
}

/// POST /api/access/devices/{id}/command（受保护）
/// 查 device_links → 适配器 send_command → 厂商 OpenAPI / 直连 MQTT 下发。
pub async fn send_command(
    State(api): State<ApiState>,
    axum::Extension(tenant_id): axum::Extension<String>,
    Path(device_id): Path<String>,
    Json(req): Json<CommandReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let link = api
        .store
        .find_link(&device_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let (vendor, vendor_id) = link
        .ok_or_else(|| (StatusCode::NOT_FOUND, "device not linked".to_string()))?;
    if vendor == "direct" {
        // 直连设备：MQTT 下发
        crate::mqtt::publish_command(&device_id, &req.code, &req.value)
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
        return Ok(Json(json!({ "ok": true, "channel": "mqtt" })));
    }
    let adapter = adapter_for(&vendor).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let creds = api
        .store
        .load_creds(&tenant_id, &vendor)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let creds: crate::adapter::VendorCreds =
        serde_json::from_value(creds).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    adapter
        .send_command(&creds, &vendor_id, &req.code, req.value.clone())
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(json!({ "ok": true, "channel": vendor })))
}
