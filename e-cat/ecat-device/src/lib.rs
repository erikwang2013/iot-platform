use axum::extract::{Query, State};
use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct Db(pub Arc<SqlxClient>);

/// 单一副本存于 ecat-access/migrations/，此处经 CARGO_MANIFEST_DIR 锚定跨包引用，
/// 不依赖进程 CWD（iot-device 启动即调 migrate）。
const MIGRATION_SQL: [&str; 2] = [
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ecat-access/migrations/0001_init.sql"
    ),
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ecat-access/migrations/0002_vendor_auth.sql"
    ),
];

pub async fn migrate(db: &SqlxClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for file in MIGRATION_SQL {
        db.execute_script(&std::fs::read_to_string(file)?).await?;
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct TenantFilter {
    pub tenant_id: Option<String>,
}

#[derive(Serialize)]
struct DeviceRow {
    id: String,
    name: String,
    vendor: String,
    status: String,
}

pub async fn list_devices(
    State(db): State<Db>,
    Query(filter): Query<TenantFilter>,
) -> Result<axum::Json<Value>, (axum::http::StatusCode, String)> {
    // ponytail: P0 支持任意/按租户过滤；参数化查询防注入，租户强制隔离 P1 随鉴权一起做
    let (sql, params): (&str, Vec<Value>) = match &filter.tenant_id {
        Some(t) => (
            "SELECT id, name, vendor, status FROM devices WHERE tenant_id = ?",
            vec![json!(t)],
        ),
        None => ("SELECT id, name, vendor, status FROM devices", vec![]),
    };
    let rows = db.0.query_with(sql, &params).await.map_err(db_err)?;
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
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("db error: {e}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_sql_files_exist() {
        for f in MIGRATION_SQL {
            assert!(std::path::Path::new(f).is_file(), "{f} missing");
        }
    }
}
