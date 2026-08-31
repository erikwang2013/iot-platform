use crate::adapter::{adapter_for, VendorCreds};
use crate::breaker::{BreakerConfig, CircuitBreaker};
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
    /// 厂商 API 熔断器（B-2）：按 vendor 隔离，open 时降级到缓存设备列表。
    pub breakers: Arc<std::collections::HashMap<String, CircuitBreaker>>,
}

impl ApiState {
    /// 取指定厂商的熔断器（不存在则惰性创建）。
    pub fn breaker(&self, vendor: &str) -> CircuitBreaker {
        self.breakers
            .get(vendor)
            .cloned()
            .unwrap_or_else(|| CircuitBreaker::new(BreakerConfig::default()))
    }
}

/// 构造受保护 API 路由（挂载见 main.rs，路径前缀 /api/access）。
pub fn router(api: ApiState) -> axum::Router {
    axum::Router::new()
        .route("/vendors/{vendor}/creds", axum::routing::put(save_creds))
        .route("/vendors/{vendor}/import", axum::routing::post(import_devices))
        .route("/devices/{device_id}/command", axum::routing::post(send_command))
        .with_state(api)
}

/// PUT /api/access/vendors/{vendor}/creds（受保护）
/// 直填凭据（AK/SK 类厂商无 OAuth：huawei/aws/azure；涂鸦/小米也可用），AES 加密落库。
pub async fn save_creds(
    State(api): State<ApiState>,
    axum::Extension(tenant_id): axum::Extension<String>,
    Path(vendor): Path<String>,
    Json(creds): Json<VendorCreds>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // 输入校验：系统边界，未知厂商/空凭据直接拒绝
    if adapter_for(&vendor).is_err() {
        return Err((StatusCode::BAD_REQUEST, format!("unknown vendor {vendor}")));
    }
    if creds.client_id.trim().is_empty() || creds.client_secret.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "client_id and client_secret required".into()));
    }
    api.store
        .save_creds(&tenant_id, &vendor, &serde_json::to_value(&creds).unwrap())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "ok": true, "vendor": vendor })))
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
    // B-2 熔断：open 期间跳过上游调用，降级返回当前已入库设备列表（缓存）
    let breaker = api.breaker(&vendor);
    let (devices, degraded) = if breaker.allow() {
        match adapter.list_devices(&creds).await {
            Ok(devs) => {
                breaker.record(true);
                (devs, false)
            }
            Err(e) => {
                breaker.record(false);
                // 失败但未熔断：仍降级到缓存并标记（避免整批失败）
                let cached = api
                    .store
                    .list_vendor_devices(&tenant_id, &vendor)
                    .await
                    .unwrap_or_default();
                if cached.is_empty() {
                    return Err((StatusCode::BAD_GATEWAY, e.to_string()));
                }
                (cached, true)
            }
        }
    } else {
        // 熔断 open：直接返回缓存
        (api.store.list_vendor_devices(&tenant_id, &vendor).await.unwrap_or_default(), true)
    };
    // 配额校验（C-5）：仅统计"确实会新增"的设备（已存在复用不占配额），
    // 超限整批拒绝（409），防部分生效。
    let mut adding = 0i64;
    for d in &devices {
        if api
            .store
            .find_device_by_vendor_id(&vendor, &d.vendor_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
            .is_none()
        {
            adding += 1;
        }
    }
    api.store
        .check_quota(&tenant_id, adding)
        .await
        .map_err(|e| (StatusCode::CONFLICT, e))?;
    let mut imported = Vec::new();
    for d in &devices {
        let platform_id = api
            .store
            .upsert_device(&tenant_id, &vendor, &d.vendor_id, &d.name, &d.category, d.online)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        imported.push(json!({ "platform_id": platform_id, "vendor_id": d.vendor_id, "name": d.name }));
    }
    // degraded=true 表示本次来自缓存（熔断或上游失败降级），供前端展示
    Ok(Json(json!({ "imported": imported, "count": imported.len(), "degraded": degraded })))
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
        // 直连设备：在线则 MQTT 下发；离线则入队（D-2），上线后补发。
        if crate::command_queue::device_online(&api.redis, &device_id).await {
            crate::mqtt::publish_command(&api.mqtt, &device_id, &req.code, &req.value)
                .await
                .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
            return Ok(Json(json!({ "ok": true, "channel": "mqtt" })));
        }
        let id = api
            .store
            .enqueue_command(
                &tenant_id,
                &device_id,
                &req.code,
                &req.value,
                crate::command_queue::command_expire_secs(),
            )
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        return Ok(Json(
            json!({ "ok": true, "queued": true, "channel": "mqtt", "queue_id": id }),
        ));
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
