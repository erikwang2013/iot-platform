use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct Db(pub Arc<SqlxClient>);

/// 单一副本存于 ecat-access/migrations/，编译期 include_str! 内联，无运行时文件依赖。
const MIGRATION_SQL: [&str; 3] = [
    include_str!("../../ecat-access/migrations/0001_init.sql"),
    include_str!("../../ecat-access/migrations/0002_vendor_auth.sql"),
    include_str!("../../ecat-access/migrations/0003_platform.sql"),
];

pub async fn migrate(db: &SqlxClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for sql in MIGRATION_SQL {
        db.execute_script(sql).await?;
    }
    Ok(())
}

#[derive(Serialize)]
struct DeviceRow {
    id: String,
    name: String,
    vendor: String,
    status: String,
}

/// 租户强制隔离：tenant 由 tenant_from_header 中间件从 x-tenant-id 校验后写入 extensions
/// （main.rs 挂载），handler 不再接受客户端自报租户。
pub async fn list_devices(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
) -> Result<axum::Json<Value>, (axum::http::StatusCode, String)> {
    let rows = db
        .0
        .query_with(
            "SELECT id, name, vendor, status FROM devices WHERE tenant_id = ?",
            &[json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    let devices: Vec<DeviceRow> = rows
        .iter()
        .map(|r| DeviceRow {
            id: r.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
            name: r.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
            vendor: r.get("vendor").and_then(Value::as_str).unwrap_or("").to_string(),
            status: r.get("status").and_then(Value::as_str).unwrap_or("").to_string(),
        })
        .collect();
    Ok(axum::Json(json!({"devices": devices})))
}

fn db_err(e: ecat_data::RdbmsError) -> (axum::http::StatusCode, String) {
    // 细节只进日志，客户端只收通用文案
    tracing::warn!(error = %e, "devices query failed");
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "devices query failed".to_string(),
    )
}

#[derive(Deserialize)]
pub struct StatusReq {
    pub status: String,
}

/// PUT /api/devices/{id}：启用/停用（status: enabled/disabled）。
pub async fn update_device(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
    Json(req): Json<StatusReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let status = req.status.trim().to_string();
    if !["online", "offline", "enabled", "disabled"].contains(&status.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "status must be online/offline/enabled/disabled".into()));
    }
    let n = db
        .0
        .execute_with(
            "UPDATE devices SET status = ? WHERE id = ? AND tenant_id = ?",
            &[json!(status), json!(id), json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, "device not found".into()));
    }
    Ok(Json(json!({ "ok": true, "status": status })))
}

/// POST /api/devices/{id}/unbind：解绑厂商链接，设备标记 unbound。
pub async fn unbind_device(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    db.0
        .execute_with(
            "DELETE FROM device_links WHERE device_id = ? AND tenant_id = ?",
            &[json!(id), json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    let n = db
        .0
        .execute_with(
            "UPDATE devices SET status = 'unbound' WHERE id = ? AND tenant_id = ?",
            &[json!(id), json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, "device not found".into()));
    }
    Ok(Json(json!({ "ok": true, "status": "unbound" })))
}

/// DELETE /api/devices/{id}：先删链接（FK 引用）再删设备。
pub async fn delete_device(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    db.0
        .execute_with(
            "DELETE FROM device_links WHERE device_id = ? AND tenant_id = ?",
            &[json!(id), json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    let n = db
        .0
        .execute_with(
            "DELETE FROM devices WHERE id = ? AND tenant_id = ?",
            &[json!(id), json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, "device not found".into()));
    }
    Ok(Json(json!({ "ok": true })))
}

// ---------- OTA 骨架：固件版本表 + 升级任务 ----------
// 固件 URL 指向 S3/CDN 签名下载地址（设计文档 §7 分发链路），
// 真实设备回环升级依赖设备端实现，属环境依赖。

#[derive(Serialize)]
struct FirmwareRow {
    id: String,
    name: String,
    version: String,
    url: String,
    description: String,
}

pub async fn list_firmwares(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = db
        .0
        .query_with(
            "SELECT id, name, version, url, description FROM ota_firmwares WHERE tenant_id = ? ORDER BY created_at",
            &[json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    let items: Vec<FirmwareRow> = rows
        .iter()
        .map(|r| FirmwareRow {
            id: r.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
            name: r.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
            version: r.get("version").and_then(Value::as_str).unwrap_or("").to_string(),
            url: r.get("url").and_then(Value::as_str).unwrap_or("").to_string(),
            description: r.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
        })
        .collect();
    Ok(Json(json!({ "firmwares": items })))
}

#[derive(Deserialize)]
pub struct FirmwareReq {
    pub name: String,
    pub version: String,
    pub url: String,
    pub description: Option<String>,
}

pub async fn create_firmware(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Json(req): Json<FirmwareReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if req.name.trim().is_empty() || req.version.trim().is_empty() || req.url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name/version/url required".into()));
    }
    let id = uuid::Uuid::new_v4().to_string();
    db.0
        .execute_with(
            "INSERT INTO ota_firmwares (id, tenant_id, name, version, url, description) VALUES (?, ?, ?, ?, ?, ?)",
            &[
                json!(id),
                json!(tenant.as_str()),
                json!(req.name.trim()),
                json!(req.version.trim()),
                json!(req.url.trim()),
                json!(req.description.unwrap_or_default()),
            ],
        )
        .await
        .map_err(db_err)?;
    Ok(Json(json!({ "id": id })))
}

pub async fn delete_firmware(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let n = db
        .0
        .execute_with(
            "DELETE FROM ota_firmwares WHERE id = ? AND tenant_id = ?",
            &[json!(id), json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, "firmware not found".into()));
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Serialize)]
struct TaskRow {
    id: String,
    device_id: String,
    firmware_id: String,
    status: String,
}

pub async fn list_ota_tasks(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = db
        .0
        .query_with(
            "SELECT id, device_id, firmware_id, status FROM ota_upgrade_tasks \
             WHERE tenant_id = ? ORDER BY created_at DESC",
            &[json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    let items: Vec<TaskRow> = rows
        .iter()
        .map(|r| TaskRow {
            id: r.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
            device_id: r.get("device_id").and_then(Value::as_str).unwrap_or("").to_string(),
            firmware_id: r.get("firmware_id").and_then(Value::as_str).unwrap_or("").to_string(),
            status: r.get("status").and_then(Value::as_str).unwrap_or("").to_string(),
        })
        .collect();
    Ok(Json(json!({ "tasks": items })))
}

#[derive(Deserialize)]
pub struct OtaTaskReq {
    pub device_id: String,
    pub firmware_id: String,
}

/// POST /api/ota/tasks：设备与固件都必须属于本租户（租户隔离边界校验）。
pub async fn create_ota_task(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Json(req): Json<OtaTaskReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let owned = db
        .0
        .query_with(
            "SELECT id FROM devices WHERE id = ? AND tenant_id = ? \
             UNION SELECT id FROM ota_firmwares WHERE id = ? AND tenant_id = ?",
            &[
                json!(req.device_id),
                json!(tenant.as_str()),
                json!(req.firmware_id),
                json!(tenant.as_str()),
            ],
        )
        .await
        .map_err(db_err)?;
    if owned.len() < 2 {
        return Err((StatusCode::FORBIDDEN, "device or firmware not in tenant".into()));
    }
    let id = uuid::Uuid::new_v4().to_string();
    db.0
        .execute_with(
            "INSERT INTO ota_upgrade_tasks (id, tenant_id, device_id, firmware_id, status) \
             VALUES (?, ?, ?, ?, 'pending')",
            &[json!(id), json!(tenant.as_str()), json!(req.device_id), json!(req.firmware_id)],
        )
        .await
        .map_err(db_err)?;
    Ok(Json(json!({ "id": id, "status": "pending" })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_sql_non_empty() {
        assert!(!MIGRATION_SQL[0].trim().is_empty());
        assert!(!MIGRATION_SQL[1].trim().is_empty());
        assert!(!MIGRATION_SQL[2].trim().is_empty());
    }
}
