use axum::extract::State;
use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct Db(pub Arc<SqlxClient>);

/// 单一副本存于 ecat-access/migrations/，编译期 include_str! 内联，无运行时文件依赖。
const MIGRATION_SQL: [&str; 2] = [
    include_str!("../../ecat-access/migrations/0001_init.sql"),
    include_str!("../../ecat-access/migrations/0002_vendor_auth.sql"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_sql_non_empty() {
        assert!(!MIGRATION_SQL[0].trim().is_empty());
        assert!(!MIGRATION_SQL[1].trim().is_empty());
    }
}
