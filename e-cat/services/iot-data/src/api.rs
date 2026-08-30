use crate::models::HistoryPoint;
use crate::td::{escape_sql_string, parse_points};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use ecat_data::TsdbClient;
use ecat_data_tdengine::TdengineClient;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiState {
    pub td: Arc<TdengineClient>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct HistoryQuery {
    pub device_id: String,
    pub code: String,
    /// epoch 毫秒
    pub start: i64,
    /// epoch 毫秒
    pub end: i64,
    /// 聚合：avg|max|min|last；缺省原始值
    pub agg: Option<String>,
    /// 聚合桶宽（TDengine INTERVAL 语法，如 "5m"/"1h"）；agg 存在时必填
    pub interval: Option<String>,
    /// 每页条数，默认 1000，上限 10000
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    1000
}

/// 受保护路由（挂载见 main.rs，路径前缀 /api/data）。
pub fn router(api: ApiState) -> axum::Router {
    axum::Router::new()
        .route("/history", axum::routing::get(history))
        .with_state(api)
}

/// GET /api/data/history?device_id=&code=&start=&end=&agg=&interval=&limit=&offset=
pub async fn history(
    State(api): State<ApiState>,
    axum::Extension(tenant_id): axum::Extension<String>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    validate(&q)?;
    let sql = build_history_sql(&tenant_id, &q);
    let resp = api
        .td
        .query(&sql)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("tdengine: {e}")))?;
    let points: Vec<HistoryPoint> = parse_points(&resp)
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(json!({
        "device_id": q.device_id,
        "code": q.code,
        "count": points.len(),
        "points": points,
    })))
}

fn validate(q: &HistoryQuery) -> Result<(), (StatusCode, String)> {
    if q.device_id.is_empty() || q.code.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "device_id and code required".into()));
    }
    if q.start >= q.end {
        return Err((StatusCode::BAD_REQUEST, "start must be < end".into()));
    }
    if !(1..=10000).contains(&q.limit) {
        return Err((StatusCode::BAD_REQUEST, "limit must be 1..10000".into()));
    }
    if let Some(agg) = &q.agg {
        if !["avg", "max", "min", "last"].contains(&agg.as_str()) {
            return Err((StatusCode::BAD_REQUEST, "agg must be avg|max|min|last".into()));
        }
        if q.interval.is_none() {
            return Err((StatusCode::BAD_REQUEST, "interval required when agg set".into()));
        }
    }
    Ok(())
}

/// 组装查询 SQL。租户/设备/属性全部经 escape_sql_string，防注入。
pub fn build_history_sql(tenant_id: &str, q: &HistoryQuery) -> String {
    let where_clause = format!(
        "tenant_id = '{}' AND device_id = '{}' AND code = '{}' \
         AND ts >= {} AND ts <= {}",
        escape_sql_string(tenant_id),
        escape_sql_string(&q.device_id),
        escape_sql_string(&q.code),
        q.start,
        q.end,
    );
    match (&q.agg, &q.interval) {
        (Some(agg), Some(interval)) => {
            let fn_name = agg.to_uppercase();
            format!(
                "SELECT _wstart AS ts, {fn_name}(value) AS value FROM iot.devdata \
                 WHERE {where_clause} INTERVAL({interval}) ORDER BY ts \
                 LIMIT {} OFFSET {}",
                q.limit, q.offset,
            )
        }
        _ => format!(
            "SELECT ts, value, value_str FROM iot.devdata \
             WHERE {where_clause} ORDER BY ts LIMIT {} OFFSET {}",
            q.limit, q.offset,
        ),
    }
}
