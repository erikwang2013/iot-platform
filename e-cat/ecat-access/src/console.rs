use crate::store::Store;
use axum::{
    Json,
    Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
};
use ecat_security::crypto::hmac_sha256_hex;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct ConsoleState {
    pub store: Arc<Store>,
}

/// 写操作需租户管理员；网关代理把 JWT role 透传为 x-tenant-role
/// （受保护路由过 x-gateway-secret 门禁，客户端无法伪造）。
fn require_admin(headers: &axum::http::HeaderMap) -> Result<(), (StatusCode, String)> {
    let role = headers
        .get("x-tenant-role")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if role == "admin" {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, "admin role required".into()))
    }
}

fn tenant_of(axum::Extension(t): axum::Extension<String>) -> String {
    t
}

pub fn router(state: ConsoleState) -> Router {
    Router::new()
        .nest(
            "/tenants",
            Router::new()
                .route("/", get(list_tenants).post(create_tenant))
                .route("/{id}", delete(delete_tenant)),
        )
        .nest(
            "/users",
            Router::new()
                .route("/", get(list_users).post(create_user))
                .route("/{id}", delete(delete_user)),
        )
        .nest(
            "/models/things",
            Router::new()
                .route("/", get(list_models).post(create_model))
                .route("/{id}", get(device_model).delete(delete_model)),
        )
        .with_state(state)
}

/// GET /api/tenants：租户列表（平台超管职能；skeleton 全量返回，租户级
/// 数据隔离对 users/models/devices 生效）。
pub async fn list_tenants(
    State(s): State<ConsoleState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let tenants = s
        .store
        .list_tenants()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "tenants": tenants })))
}

#[derive(Deserialize)]
pub struct TenantReq {
    pub name: String,
    pub quota: Option<i64>,
}

pub async fn create_tenant(
    State(s): State<ConsoleState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<TenantReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&headers)?;
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name required".into()));
    }
    let id = s
        .store
        .create_tenant(&name, req.quota.unwrap_or(100))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "id": id })))
}

pub async fn delete_tenant(
    State(s): State<ConsoleState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&headers)?;
    if !s
        .store
        .delete_tenant(&id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        return Err((StatusCode::NOT_FOUND, "tenant not found".into()));
    }
    Ok(Json(json!({ "ok": true })))
}

/// GET /api/users：本租户成员（不带密码哈希）。
pub async fn list_users(
    State(s): State<ConsoleState>,
    tenant: axum::Extension<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let users = s
        .store
        .list_users(&tenant_of(tenant))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let users: Vec<Value> = users
        .iter()
        .map(|u| {
            json!({ "id": u.id, "username": u.username, "role": u.role, "tenant_id": u.tenant_id })
        })
        .collect();
    Ok(Json(json!({ "users": users })))
}

#[derive(Deserialize)]
pub struct UserReq {
    pub username: String,
    pub password: String,
    pub tenant_id: String,
    pub role: String,
}

pub async fn create_user(
    State(s): State<ConsoleState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<UserReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&headers)?;
    let username = req.username.trim().to_string();
    if username.is_empty() || req.password.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "username and password required".into()));
    }
    if !["admin", "operator", "readonly"].contains(&req.role.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "role must be admin/operator/readonly".into()));
    }
    if req.tenant_id.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "tenant_id required".into()));
    }
    let pepper = std::env::var("IOT_PASSWORD_PEPPER")
        .unwrap_or_else(|_| "iot-password-pepper-v1".into());
    let hash = hmac_sha256_hex(&pepper, req.password.as_bytes());
    let id = s
        .store
        .create_user(&req.tenant_id, &username, &hash, &req.role)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "id": id })))
}

pub async fn delete_user(
    State(s): State<ConsoleState>,
    headers: axum::http::HeaderMap,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&headers)?;
    if !s
        .store
        .delete_user(&tenant_of(tenant), &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        return Err((StatusCode::NOT_FOUND, "user not found".into()));
    }
    Ok(Json(json!({ "ok": true })))
}

/// GET /api/models/things：本租户全部物模型条目（条目带 id 供删除）。
pub async fn list_models(
    State(s): State<ConsoleState>,
    tenant: axum::Extension<String>,
) -> Result<Json<Vec<Value>>, (StatusCode, String)> {
    let models = s
        .store
        .list_models(&tenant_of(tenant))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let mut out: Vec<Value> = models
        .iter()
        .map(|(id, schema)| {
            let mut v = schema.clone();
            v["id"] = json!(id);
            v
        })
        .collect();
    out.sort_by_key(|m| m["type"].as_str().unwrap_or("").to_string());
    Ok(Json(out))
}

#[derive(Deserialize)]
pub struct ModelReq {
    pub identifier: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub data_type: Option<String>,
    pub unit: Option<String>,
    pub rw: Option<String>,
    pub device_id: Option<String>,
}

pub async fn create_model(
    State(s): State<ConsoleState>,
    tenant: axum::Extension<String>,
    Json(req): Json<ModelReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let tenant_id = tenant_of(tenant);
    if req.identifier.trim().is_empty() || req.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "identifier and name required".into()));
    }
    if !["property", "event", "service"].contains(&req.kind.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "type must be property/event/service".into()));
    }
    let schema = json!({
        "identifier": req.identifier.trim(),
        "name": req.name.trim(),
        "type": req.kind,
        "data_type": req.data_type.unwrap_or_default(),
        "unit": req.unit.unwrap_or_default(),
        "rw": req.rw.unwrap_or_else(|| "rw".into()),
    });
    let id = s
        .store
        .create_model(&tenant_id, req.device_id.as_deref(), &schema)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "id": id })))
}

pub async fn delete_model(
    State(s): State<ConsoleState>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if !s
        .store
        .delete_model(&tenant_of(tenant), &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        return Err((StatusCode::NOT_FOUND, "model not found".into()));
    }
    Ok(Json(json!({ "ok": true })))
}

/// GET /api/models/things/{device_id}：客户端详情用，全局+设备私有合并后
/// 按属性/事件/服务分组。
pub async fn device_model(
    State(s): State<ConsoleState>,
    tenant: axum::Extension<String>,
    Path(device_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let models = s
        .store
        .device_models(&tenant_of(tenant), &device_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let mut grouped = json!({ "properties": [], "events": [], "services": [] });
    for m in models {
        let kind = m["type"].as_str().unwrap_or("property");
        let list = match kind {
            "event" => &mut grouped["events"],
            "service" => &mut grouped["services"],
            _ => &mut grouped["properties"],
        };
        if let Some(arr) = list.as_array_mut() {
            arr.push(m);
        }
    }
    Ok(Json(grouped))
}
