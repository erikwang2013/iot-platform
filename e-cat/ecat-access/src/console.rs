use crate::store::Store;
use axum::{
    Json,
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
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
        .nest(
            "/audit",
            Router::new().route("/", get(list_audit)),
        )
        .nest(
            "/api-keys",
            Router::new()
                .route("/", get(list_api_keys).post(create_api_key))
                .route("/{id}", delete(revoke_api_key)),
        )
        .route("/keys/rotate", post(rotate_creds_key))
        .with_state(state)
}

#[derive(Deserialize)]
pub struct AuditQuery {
    pub page: Option<u32>,
    pub size: Option<u32>,
}

/// GET /api/audit?page=1&size=20：本租户审计日志（分页倒序）。
/// 仅 admin 可见（read-only 角色 403）——审计含谁/何时/改了什么，属敏感数据。
pub async fn list_audit(
    State(s): State<ConsoleState>,
    headers: axum::http::HeaderMap,
    tenant: axum::Extension<String>,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&headers)?;
    let tenant_id = tenant_of(tenant);
    let page = q.page.unwrap_or(1).max(1);
    let size = q.size.unwrap_or(20).clamp(1, 200);
    let rows = s
        .store
        .list_audit(&tenant_id, page, size)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let events: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.id, "tenant_id": r.tenant_id, "role": r.role,
                "method": r.method, "path": r.path, "status": r.status,
                "created_at": r.created_at,
            })
        })
        .collect();
    Ok(Json(json!({ "page": page, "size": size, "total": events.len(), "events": events })))
}

/// POST /api/keys/rotate：凭据加密密钥轮换（仅 admin）。
/// 生成新随机密钥成为 current（新数据用新密钥，key_version=2），旧密钥移入
/// 宽限窗口（旧数据仍可解密），并持久化 cred_keys.json（重启不丢）。
/// 返回新密钥，供运维同步到 IOT_CRED_ENCRYPT_KEY 环境变量。
pub async fn rotate_creds_key(
    State(s): State<ConsoleState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&headers)?;
    let new_key = s
        .store
        .rotate_key()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({
        "ok": true,
        "key_version": s.store.key_version(),
        "key": new_key,
        "hint": "set IOT_CRED_ENCRYPT_KEY=<key> (previous key stays decryptable via grace window; remove IOT_CRED_ENCRYPT_KEY_OLD only after re-encryption)",
    })))
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

/// GET /api/api-keys：本租户开放 API 密钥列表（只含元数据，secret 不回显）。
pub async fn list_api_keys(
    State(s): State<ConsoleState>,
    tenant: axum::Extension<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let keys = s
        .store
        .list_api_keys(&tenant_of(tenant))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "keys": keys })))
}

#[derive(Deserialize)]
pub struct ApiKeyReq {
    pub name: String,
    /// 密钥 scope：read（默认，仅读端点）| write | command（写端点）。
    #[serde(default)]
    pub scope: Option<String>,
}

/// POST /api/api-keys：创建开放 API 密钥。app_secret 仅此一次返回
/// （库中只存哈希），丢失需吊销重建。
pub async fn create_api_key(
    State(s): State<ConsoleState>,
    headers: axum::http::HeaderMap,
    tenant: axum::Extension<String>,
    Json(req): Json<ApiKeyReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&headers)?;
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name required".into()));
    }
    let scope = req.scope.as_deref().unwrap_or("read").to_string();
    if !["read", "write", "command"].contains(&scope.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "scope must be read|write|command".into()));
    }
    let (app_id, app_secret) = s
        .store
        .create_api_key(&tenant_of(tenant), &name, &scope)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "app_id": app_id, "app_secret": app_secret, "scope": scope })))
}

/// DELETE /api/api-keys/{id}：吊销密钥（幂等；已吊销返回 404）。
pub async fn revoke_api_key(
    State(s): State<ConsoleState>,
    headers: axum::http::HeaderMap,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_admin(&headers)?;
    if !s
        .store
        .revoke_api_key(&tenant_of(tenant), &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        return Err((StatusCode::NOT_FOUND, "api key not found or already revoked".into()));
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

/// 内建品类模板（category → 物模型行）：设备创建时按 category 一键填充。
/// 元组：identifier, name, type, data_type, unit, rw
#[allow(clippy::type_complexity)]
const TEMPLATE_ROWS: [(&str, &[(&str, &str, &str, &str, &str, &str)]); 3] = [
    ("thermo-hygrometer", &[
        ("temperature", "温度", "property", "float", "°C", "r"),
        ("humidity", "湿度", "property", "float", "%RH", "r"),
        ("temp_alarm", "温度告警", "event", "string", "", ""),
        ("hum_alarm", "湿度告警", "event", "string", "", ""),
    ]),
    ("switch_panel", &[
        ("switch_1", "开关 1", "property", "bool", "", "rw"),
        ("switch_2", "开关 2", "property", "bool", "", "rw"),
        ("switch_3", "开关 3", "property", "bool", "", "rw"),
        ("switch_event", "开关事件", "event", "string", "", ""),
    ]),
    ("smart_plug", &[
        ("power", "功率", "property", "float", "W", "r"),
        ("on_off", "通断", "property", "bool", "", "rw"),
        ("overload_alarm", "过载告警", "event", "string", "", ""),
    ]),
];

/// 品类模板 → 物模型行（带 version: 1）；未知品类返回 None。
fn category_templates(category: &str) -> Option<Vec<Value>> {
    let rows = TEMPLATE_ROWS.iter().find(|(name, _)| *name == category)?.1;
    Some(
        rows.iter()
            .map(|(identifier, name, kind, dt, unit, rw)| {
                json!({
                    "identifier": identifier,
                    "name": name,
                    "type": kind,
                    "data_type": dt,
                    "unit": unit,
                    "rw": rw,
                    "version": 1,
                })
            })
            .collect(),
    )
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
    /// 品类模板：传入时一键填充该品类全部属性/事件（忽略 identifier 等单行字段）
    pub category: Option<String>,
}

pub async fn create_model(
    State(s): State<ConsoleState>,
    tenant: axum::Extension<String>,
    Json(req): Json<ModelReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let tenant_id = tenant_of(tenant);
    if let Some(cat) = req.category.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        let items = category_templates(cat)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("unknown category '{cat}'")))?;
        let ids = s
            .store
            .create_model_template(&tenant_id, req.device_id.as_deref(), &items)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        return Ok(Json(json!({ "ids": ids })));
    }
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
        "version": 1,
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

#[cfg(test)]
mod tests {
    use super::category_templates;

    #[test]
    fn templates_exist_for_known_categories() {
        for cat in ["thermo-hygrometer", "switch_panel", "smart_plug"] {
            let rows = category_templates(cat).expect("known category");
            assert!(!rows.is_empty());
            for r in &rows {
                assert_eq!(r["version"], 1, "模板行默认 version=1");
                assert!(r["type"].as_str().is_some());
            }
        }
        assert_eq!(category_templates("bogus"), None);
    }
}
