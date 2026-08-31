pub mod groups;

use axum::{
    Json,
    extract::{Path, Query, State},
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
const MIGRATION_SQL: [&str; 5] = [
    include_str!("../../ecat-access/migrations/0001_init.sql"),
    include_str!("../../ecat-access/migrations/0002_vendor_auth.sql"),
    include_str!("../../ecat-access/migrations/0003_platform.sql"),
    include_str!("../../ecat-access/migrations/0004_audit.sql"),
    include_str!("../../ecat-access/migrations/0005_groups.sql"),
];

pub async fn migrate(db: &SqlxClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for sql in MIGRATION_SQL {
        db.execute_script(sql).await?;
    }
    // MySQL 8 无 ADD COLUMN IF NOT EXISTS，老库补列需先查 information_schema；
    // 新装库由 0003 CREATE TABLE 直接建全列，此处查询返回 1 跳过
    for (column, ddl) in OTA_TASK_EXTRA_COLUMNS {
        let exists = db
            .query_with(
                "SELECT COUNT(*) AS n FROM information_schema.COLUMNS \
                 WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'ota_upgrade_tasks' \
                 AND COLUMN_NAME = ?",
                &[json!(column)],
            )
            .await?;
        let n = exists
            .first()
            .and_then(|r| r.get("n"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if n == 0 {
            db.execute_with(&format!("ALTER TABLE ota_upgrade_tasks ADD COLUMN {ddl}"), &[])
                .await?;
        }
    }
    Ok(())
}

const OTA_TASK_EXTRA_COLUMNS: [(&str, &str); 2] = [
    ("progress", "progress INT NOT NULL DEFAULT 0"),
    ("message", "message VARCHAR(512) NOT NULL DEFAULT ''"),
];

#[derive(Serialize)]
struct DeviceRow {
    id: String,
    name: String,
    vendor: String,
    status: String,
}

/// 租户强制隔离：tenant 由 tenant_from_header 中间件从 x-tenant-id 校验后写入 extensions
/// （main.rs 挂载），handler 不再接受客户端自报租户。
/// 可选过滤：?group_id=&tag=（分组/标签，见 groups.rs 表）。
#[derive(Deserialize)]
pub struct DeviceListQuery {
    pub group_id: Option<String>,
    pub tag: Option<String>,
}

pub async fn list_devices(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Query(q): Query<DeviceListQuery>,
) -> Result<axum::Json<Value>, (axum::http::StatusCode, String)> {
    let mut sql =
        String::from("SELECT id, name, vendor, status FROM devices WHERE tenant_id = ?");
    let mut params: Vec<Value> = vec![json!(tenant.as_str())];
    if let Some(g) = &q.group_id {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM device_group_members m \
             WHERE m.device_id = devices.id AND m.group_id = ?)",
        );
        params.push(json!(g));
    }
    if let Some(t) = &q.tag {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM device_tags t2 \
             WHERE t2.device_id = devices.id AND t2.tag = ?)",
        );
        params.push(json!(t));
    }
    let rows = db.0.query_with(&sql, &params).await.map_err(db_err)?;
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

/// GET /api/devices/stats：设备总数/在线/离线 + 厂商分布（租户隔离）。
pub async fn device_stats(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = db
        .0
        .query_with(
            "SELECT status, COUNT(*) AS n FROM devices WHERE tenant_id = ? GROUP BY status",
            &[json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    let counts: Vec<(String, i64)> = rows
        .iter()
        .map(|r| {
            (
                r.get("status").and_then(Value::as_str).unwrap_or_default().to_string(),
                r.get("n").and_then(Value::as_i64).unwrap_or(0),
            )
        })
        .collect();
    let (total, online, offline) = stats_from_counts(&counts);
    let vendors = db
        .0
        .query_with(
            "SELECT vendor, COUNT(*) AS n FROM devices WHERE tenant_id = ? \
             GROUP BY vendor ORDER BY n DESC",
            &[json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    let vendors: Vec<Value> = vendors
        .iter()
        .map(|r| {
            json!({
                "vendor": r.get("vendor").and_then(Value::as_str).unwrap_or_default(),
                "count": r.get("n").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .collect();
    Ok(Json(json!({ "total": total, "online": online, "offline": offline, "vendors": vendors })))
}

/// status 计数 → (总数, 在线, 离线)。enabled/disabled/unbound 只计入总数。
fn stats_from_counts(counts: &[(String, i64)]) -> (i64, i64, i64) {
    let mut total = 0;
    let mut online = 0;
    let mut offline = 0;
    for (status, n) in counts {
        total += n;
        match status.as_str() {
            "online" => online += n,
            "offline" => offline += n,
            _ => {}
        }
    }
    (total, online, offline)
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
    // T2.8: 索引同步（覆盖式 PUT，先取回 name/vendor 补全 doc；失败仅 warn 不阻断）
    if let Some(search) = ecat_search::connect_search() {
        match db.0
            .query_with(
                "SELECT name, vendor FROM devices WHERE id = ? AND tenant_id = ?",
                &[json!(id), json!(tenant.as_str())],
            )
            .await
        {
            Ok(rows) => {
                if let Some(r) = rows.first() {
                    let doc = json!({
                        "tenant_id": tenant.as_str(),
                        "id": id,
                        "name": r.get("name").and_then(Value::as_str).unwrap_or(""),
                        "vendor": r.get("vendor").and_then(Value::as_str).unwrap_or(""),
                        "status": status,
                    });
                    if let Err(e) = search.index("devices", &id, &doc).await {
                        tracing::warn!(error = %e, "device index update failed");
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "device index lookup failed"),
        }
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
    // T2.8: 同步删除 OpenSearch 索引（失败仅 warn，不阻断删除）
    if let Some(search) = ecat_search::connect_search()
        && let Err(e) = search.delete("devices", &id).await
    {
        tracing::warn!(error = %e, "device index delete failed");
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
    firmware_version: String,
    status: String,
    progress: i64,
    message: String,
    updated_at: String,
}

pub async fn list_ota_tasks(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = db
        .0
        .query_with(
            "SELECT t.id, t.device_id, t.firmware_id, f.version AS firmware_version, \
             t.status, t.progress, t.message, CAST(t.updated_at AS CHAR) \
             FROM ota_upgrade_tasks t LEFT JOIN ota_firmwares f ON f.id = t.firmware_id \
             WHERE t.tenant_id = ? ORDER BY t.created_at DESC",
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
            firmware_version: r.get("firmware_version").and_then(Value::as_str).unwrap_or("").to_string(),
            status: r.get("status").and_then(Value::as_str).unwrap_or("").to_string(),
            progress: r.get("progress").and_then(Value::as_i64).unwrap_or(0),
            message: r.get("message").and_then(Value::as_str).unwrap_or("").to_string(),
            updated_at: r.get("updated_at").and_then(Value::as_str).unwrap_or("").to_string(),
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

/// OTA 任务状态机：pending → downloading → installing → success|failed。
/// 允许同状态重复上报（进度更新）；跨终态（success/failed）不可再流转。
pub fn transition_allowed(current: &str, next: &str) -> bool {
    if current == next {
        return matches!(current, "pending" | "downloading" | "installing" | "success" | "failed");
    }
    matches!(
        (current, next),
        ("pending", "downloading")
            | ("downloading", "installing")
            | ("installing", "success")
            | ("installing", "failed")
    )
}

pub const OTA_STATUSES: [&str; 5] = ["downloading", "installing", "success", "failed", "pending"];

#[derive(Deserialize)]
pub struct OtaReportReq {
    /// downloading|installing|success|failed
    pub status: String,
    pub progress: Option<u8>,
    pub message: Option<String>,
}

/// POST /api/ota/tasks/{id}/report：设备上报升级状态/进度（模拟设备可 mock）。
/// 校验：状态在状态机内、progress ≤ 100、message ≤ 512；任务必须属于本租户。
pub async fn report_ota_progress(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
    Json(req): Json<OtaReportReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let status = req.status.trim().to_string();
    if !["downloading", "installing", "success", "failed"].contains(&status.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "status must be downloading/installing/success/failed".into()));
    }
    let progress = req.progress.unwrap_or(0).min(100);
    let message = req.message.unwrap_or_default();
    if message.len() > 512 {
        return Err((StatusCode::BAD_REQUEST, "message must be <= 512 chars".into()));
    }
    let rows = db
        .0
        .query_with(
            "SELECT status FROM ota_upgrade_tasks WHERE id = ? AND tenant_id = ?",
            &[json!(id), json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    let current = rows
        .first()
        .and_then(|r| r.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if current.is_empty() {
        return Err((StatusCode::NOT_FOUND, "ota task not found".into()));
    }
    if !transition_allowed(&current, &status) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("invalid transition {current} -> {status}"),
        ));
    }
    let final_progress = if status == "success" { 100 } else { progress };
    let n = db
        .0
        .execute_with(
            "UPDATE ota_upgrade_tasks SET status = ?, progress = ?, message = ? \
             WHERE id = ? AND tenant_id = ?",
            &[
                json!(status),
                json!(final_progress as i64),
                json!(message),
                json!(id),
                json!(tenant.as_str()),
            ],
        )
        .await
        .map_err(db_err)?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, "ota task not found".into()));
    }
    Ok(Json(json!({ "ok": true, "status": status, "progress": final_progress })))
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

    #[test]
    fn stats_from_counts_totals() {
        let c = vec![
            ("online".to_string(), 3),
            ("offline".to_string(), 2),
            ("enabled".to_string(), 5),
        ];
        assert_eq!(stats_from_counts(&c), (10, 3, 2));
        assert_eq!(stats_from_counts(&[]), (0, 0, 0));
    }

    #[test]
    fn ota_transition_matrix() {
        // 正常流转
        assert!(transition_allowed("pending", "downloading"));
        assert!(transition_allowed("downloading", "installing"));
        assert!(transition_allowed("installing", "success"));
        assert!(transition_allowed("installing", "failed"));
        // 同状态重复上报（进度更新）
        assert!(transition_allowed("downloading", "downloading"));
        assert!(transition_allowed("installing", "installing"));
        // 跳步与逆向
        assert!(!transition_allowed("pending", "installing"));
        assert!(!transition_allowed("pending", "success"));
        assert!(!transition_allowed("downloading", "success"));
        // 终态冻结
        assert!(!transition_allowed("success", "failed"));
        assert!(!transition_allowed("failed", "success"));
        assert!(!transition_allowed("success", "installing"));
        // 未知状态
        assert!(!transition_allowed("pending", "bogus"));
        assert!(!transition_allowed("bogus", "downloading"));
    }
}
