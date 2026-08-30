use axum::{
    Router,
    extract::{Query, State},
    routing::get,
};
use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
struct Db(Arc<SqlxClient>);

async fn migrate(db: &SqlxClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for file in ["migrations/0001_init.sql", "migrations/0002_vendor_auth.sql"] {
        db.execute_script(&std::fs::read_to_string(file)?).await?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct TenantFilter {
    tenant_id: Option<String>,
}

#[derive(Serialize)]
struct DeviceRow {
    id: String,
    name: String,
    vendor: String,
    status: String,
}

async fn list_devices(
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
    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("db error: {e}"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://iot:iot@localhost:3306/iot".into());
    let db = SqlxClient::connect(&db_url).await?;
    migrate(&db).await?;
    let db = Arc::new(db);

    let health_router = ecat_health::HealthRegistry::new()
        .with_check(ecat_health::db_check(db.clone()))
        .into_router();

    let router = Router::new()
        .merge(health_router)
        .merge(
            Router::new()
                .route("/api/devices", get(list_devices))
                .with_state(Db(db)),
        );

    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8081").router(router);
    let mut app = ecat::App::builder()
        .name("iot-device")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
