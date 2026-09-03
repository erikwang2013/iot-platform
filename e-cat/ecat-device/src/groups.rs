//! 设备分组 / 标签 + 批量操作（#59）。
//! 表：device_groups / device_group_members / device_tags（0005_groups.sql）。
//! 全部按租户隔离：tenant 由 tenant_from_header 中间件注入。

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use ecat_data::RdbmsClient;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::Db;

fn db_err(e: ecat_data::RdbmsError) -> (StatusCode, String) {
    tracing::warn!(error = %e, "groups query failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "groups query failed".to_string())
}

/// GET /api/devices/groups：分组列表 + 成员数（租户隔离）。
pub async fn list_groups(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let rows = db
        .0
        .query_with(
            "SELECT g.id, g.name, COUNT(m.device_id) AS member_count \
             FROM device_groups g \
             LEFT JOIN device_group_members m ON m.group_id = g.id \
             WHERE g.tenant_id = ? \
             GROUP BY g.id, g.name ORDER BY g.created_at DESC",
            &[json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    let groups: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get("id").and_then(Value::as_i64).map(|n| n.to_string()).unwrap_or_default(),
                "name": r.get("name").and_then(Value::as_str).unwrap_or_default(),
                "member_count": r.get("member_count").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .collect();
    Ok(Json(json!({ "groups": groups })))
}

#[derive(Deserialize)]
pub struct GroupReq {
    pub name: String,
}

/// POST /api/devices/groups：新建分组（同名同租户 409）。
pub async fn create_group(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Json(req): Json<GroupReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 64 {
        return Err((StatusCode::BAD_REQUEST, "name required, max 64 chars".into()));
    }
    let exists = db
        .0
        .query_with(
            "SELECT COUNT(*) AS n FROM device_groups WHERE tenant_id = ? AND name = ?",
            &[json!(tenant.as_str()), json!(name)],
        )
        .await
        .map_err(db_err)?;
    if exists.first().and_then(|r| r.get("n")).and_then(Value::as_i64).unwrap_or(0) > 0 {
        return Err((StatusCode::CONFLICT, "group name exists".into()));
    }
    let id = ecat::ids::next_id();
    db.0.execute_with(
        "INSERT INTO device_groups (id, tenant_id, name) VALUES (?, ?, ?)",
        &[json!(id), json!(tenant.as_str()), json!(name)],
    )
    .await
    .map_err(db_err)?;
    Ok(Json(json!({ "id": id.to_string(), "name": name })))
}

/// DELETE /api/devices/groups/{id}：删除分组（成员级联删，设备不受影响）。
pub async fn delete_group(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let group_n: i64 = id.parse().map_err(|_| (StatusCode::BAD_REQUEST, "invalid group id".into()))?;
    let n = db
        .0
        .execute_with(
            "DELETE FROM device_groups WHERE id = ? AND tenant_id = ?",
            &[json!(group_n), json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, "group not found".into()));
    }
    db.0.execute_with(
        "DELETE FROM device_group_members WHERE group_id = ?",
        &[json!(group_n)],
    )
    .await
    .map_err(db_err)?;
    Ok(Json(json!({ "ok": true })))
}

/// 校验设备 ID 属于本租户；返回不存在的设备列表（空 = 全部有效）。
async fn tenant_devices(db: &Db, tenant: &str, ids: &[String]) -> Result<Vec<String>, (StatusCode, String)> {
    if ids.is_empty() || ids.len() > 500 {
        return Err((StatusCode::BAD_REQUEST, "device_ids required, max 500".into()));
    }
    let mut missing = Vec::new();
    for id in ids {
        // 非数字 id（或非本租户设备）一律视为无效（devices.id 为 BIGINT 列）
        let Ok(id_n) = id.parse::<i64>() else {
            missing.push(id.clone());
            continue;
        };
        let rows = db
            .0
            .query_with(
                "SELECT COUNT(*) AS n FROM devices WHERE id = ? AND tenant_id = ?",
                &[json!(id_n), json!(tenant)],
            )
            .await
            .map_err(db_err)?;
        if rows.first().and_then(|r| r.get("n")).and_then(Value::as_i64).unwrap_or(0) == 0 {
            missing.push(id.clone());
        }
    }
    Ok(missing)
}

#[derive(Deserialize)]
pub struct MembersReq {
    pub device_ids: Vec<String>,
}

/// POST /api/devices/groups/{id}/members：批量加入（忽略已加入；设备必须属于本租户）。
pub async fn add_members(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
    Json(req): Json<MembersReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // device_groups/device_group_members 的 id 列为 BIGINT：绑定数字
    let group_n: i64 = id.parse().map_err(|_| (StatusCode::BAD_REQUEST, "invalid group id".into()))?;
    let missing = tenant_devices(&db, &tenant, &req.device_ids).await?;
    if !missing.is_empty() {
        return Err((StatusCode::BAD_REQUEST, json!({"error": "devices not in tenant", "device_ids": missing}).to_string()));
    }
    let group_ok = db
        .0
        .query_with(
            "SELECT COUNT(*) AS n FROM device_groups WHERE id = ? AND tenant_id = ?",
            &[json!(group_n), json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    if group_ok.first().and_then(|r| r.get("n")).and_then(Value::as_i64).unwrap_or(0) == 0 {
        return Err((StatusCode::NOT_FOUND, "group not found".into()));
    }
    let mut affected = 0u64;
    for device_id in &req.device_ids {
        let device_n: i64 = device_id
            .parse()
            .map_err(|_| (StatusCode::BAD_REQUEST, "invalid device id".into()))?;
        affected += db
            .0
            .execute_with(
                "INSERT IGNORE INTO device_group_members (group_id, device_id) VALUES (?, ?)",
                &[json!(group_n), json!(device_n)],
            )
            .await
            .map_err(db_err)?;
    }
    Ok(Json(json!({ "ok": true, "affected": affected })))
}

/// DELETE /api/devices/groups/{id}/members：批量移出分组。
pub async fn remove_members(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
    Json(req): Json<MembersReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let group_n: i64 = id.parse().map_err(|_| (StatusCode::BAD_REQUEST, "invalid group id".into()))?;
    let mut affected = 0u64;
    for device_id in &req.device_ids {
        let device_n: i64 = device_id
            .parse()
            .map_err(|_| (StatusCode::BAD_REQUEST, "invalid device id".into()))?;
        affected += db
            .0
            .execute_with(
                "DELETE FROM device_group_members WHERE group_id = ? AND device_id = ? \
                 AND EXISTS (SELECT 1 FROM device_groups g WHERE g.id = ? AND g.tenant_id = ?)",
                &[json!(group_n), json!(device_n), json!(group_n), json!(tenant.as_str())],
            )
            .await
            .map_err(db_err)?;
    }
    Ok(Json(json!({ "ok": true, "affected": affected })))
}

#[derive(Deserialize)]
pub struct BatchReq {
    pub action: String,
    pub device_ids: Vec<String>,
    /// action=tag/untag 时必填
    pub tags: Option<Vec<String>>,
    /// action=bind_group/unbind_group 时必填
    pub group_id: Option<String>,
}

const MAX_TAGS_PER_DEVICE: usize = 10;

/// POST /api/devices/batch：批量操作。
/// action：tag（打标签）| untag（去标签）| bind_group（加入分组）|
///         unbind_group（移出分组）| delete（删除设备）。
/// 单次 ≤500 台；设备必须属于本租户；不存在的设备整批拒绝（防部分生效）。
pub async fn batch_devices(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Json(req): Json<BatchReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if req.device_ids.is_empty() || req.device_ids.len() > 500 {
        return Err((StatusCode::BAD_REQUEST, "device_ids required, max 500".into()));
    }
    let missing = tenant_devices(&db, &tenant, &req.device_ids).await?;
    if !missing.is_empty() {
        return Err((StatusCode::BAD_REQUEST, json!({"error": "devices not in tenant", "device_ids": missing}).to_string()));
    }
    match req.action.as_str() {
        "tag" | "untag" => {
            let tags: Vec<String> = req
                .tags
                .unwrap_or_default()
                .into_iter()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty() && t.len() <= 32)
                .collect();
            if tags.is_empty() {
                return Err((StatusCode::BAD_REQUEST, "tags required, max 32 chars each".into()));
            }
            if tags.len() > MAX_TAGS_PER_DEVICE {
                return Err((StatusCode::BAD_REQUEST, format!("max {MAX_TAGS_PER_DEVICE} tags per operation").into()));
            }
            let mut affected = 0u64;
            for device_id in &req.device_ids {
                let device_n: i64 = device_id
                    .parse()
                    .map_err(|_| (StatusCode::BAD_REQUEST, "invalid device id".into()))?;
                // 单设备标签总数上限：现有 + 新增（保守判定，含重复也拒绝）防标签爆炸
                if req.action == "tag" {
                    let n = db
                        .0
                        .query_with(
                            "SELECT COUNT(*) AS n FROM device_tags WHERE device_id = ?",
                            &[json!(device_n)],
                        )
                        .await
                        .map_err(db_err)?;
                    let existing = n.first().and_then(|r| r.get("n")).and_then(Value::as_i64).unwrap_or(0);
                    if existing + tags.len() as i64 > MAX_TAGS_PER_DEVICE as i64 {
                        return Err((StatusCode::BAD_REQUEST, format!("device {device_id} would exceed {MAX_TAGS_PER_DEVICE} tags").into()));
                    }
                }
                for tag in &tags {
                    let sql = if req.action == "tag" {
                        "INSERT IGNORE INTO device_tags (device_id, tag) VALUES (?, ?)"
                    } else {
                        "DELETE FROM device_tags WHERE device_id = ? AND tag = ?"
                    };
                    affected += db.0.execute_with(sql, &[json!(device_n), json!(tag)]).await.map_err(db_err)?;
                }
            }
            Ok(Json(json!({ "ok": true, "affected": affected })))
        }
        "bind_group" | "unbind_group" => {
            let group_id = req.group_id.ok_or_else(|| (StatusCode::BAD_REQUEST, "group_id required".into()))?;
            let group_n: i64 = group_id.parse().map_err(|_| (StatusCode::BAD_REQUEST, "invalid group id".into()))?;
            let group_ok = db
                .0
                .query_with(
                    "SELECT COUNT(*) AS n FROM device_groups WHERE id = ? AND tenant_id = ?",
                    &[json!(group_n), json!(tenant.as_str())],
                )
                .await
                .map_err(db_err)?;
            if group_ok.first().and_then(|r| r.get("n")).and_then(Value::as_i64).unwrap_or(0) == 0 {
                return Err((StatusCode::NOT_FOUND, "group not found".into()));
            }
            let mut affected = 0u64;
            for device_id in &req.device_ids {
                let device_n: i64 = device_id
                    .parse()
                    .map_err(|_| (StatusCode::BAD_REQUEST, "invalid device id".into()))?;
                let sql = if req.action == "bind_group" {
                    "INSERT IGNORE INTO device_group_members (group_id, device_id) VALUES (?, ?)"
                } else {
                    "DELETE FROM device_group_members WHERE group_id = ? AND device_id = ?"
                };
                affected += db.0.execute_with(sql, &[json!(group_n), json!(device_n)]).await.map_err(db_err)?;
            }
            Ok(Json(json!({ "ok": true, "affected": affected })))
        }
        "delete" => {
            let mut affected = 0u64;
            for device_id in &req.device_ids {
                let device_n: i64 = device_id
                    .parse()
                    .map_err(|_| (StatusCode::BAD_REQUEST, "invalid device id".into()))?;
                affected += db
                    .0
                    .execute_with(
                        "DELETE FROM devices WHERE id = ? AND tenant_id = ?",
                        &[json!(device_n), json!(tenant.as_str())],
                    )
                    .await
                    .map_err(db_err)?;
                db.0.execute_with(
                    "DELETE FROM device_tags WHERE device_id = ?",
                    &[json!(device_n)],
                )
                .await
                .map_err(db_err)?;
                db.0.execute_with(
                    "DELETE FROM device_group_members WHERE device_id = ?",
                    &[json!(device_n)],
                )
                .await
                .map_err(db_err)?;
            }
            Ok(Json(json!({ "ok": true, "affected": affected })))
        }
        other => Err((StatusCode::BAD_REQUEST, format!("unknown action: {other}").into())),
    }
}

/// GET /api/devices/{id}/tags：设备标签列表。
pub async fn list_device_tags(
    State(db): State<Db>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let device_n: i64 = id.parse().map_err(|_| (StatusCode::BAD_REQUEST, "invalid device id".into()))?;
    let rows = db
        .0
        .query_with(
            "SELECT tag FROM device_tags \
             WHERE device_id = ? AND EXISTS (SELECT 1 FROM devices d WHERE d.id = ? AND d.tenant_id = ?) \
             ORDER BY tag",
            &[json!(device_n), json!(device_n), json!(tenant.as_str())],
        )
        .await
        .map_err(db_err)?;
    let tags: Vec<String> = rows
        .iter()
        .filter_map(|r| r.get("tag").and_then(Value::as_str).map(String::from))
        .collect();
    Ok(Json(json!({ "device_id": id, "tags": tags })))
}
