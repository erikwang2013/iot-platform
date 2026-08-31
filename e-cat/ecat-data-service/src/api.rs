use crate::models::HistoryPoint;
use crate::td::Dialect;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use ecat_data::TsdbClient;
use ecat_data_tdengine::sql::{escape_sql_string, parse_points};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiState {
    /// TSDB_KIND 选出的后端客户端（tdengine | clickhouse，见 td::connect_tsdb）
    pub td: Arc<dyn TsdbClient>,
    /// 查询方言：决定 SQL 组装与响应解析（td::dialect()）
    pub dialect: Dialect,
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
        .route("/export", axum::routing::get(export))
        .with_state(api)
}

/// GET /api/data/history?device_id=&code=&start=&end=&agg=&interval=&limit=&offset=
pub async fn history(
    State(api): State<ApiState>,
    axum::Extension(tenant_id): axum::Extension<String>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    validate(&q)?;
    let sql = build_sql(api.dialect, &tenant_id, &q);
    let resp = api
        .td
        .query(&sql)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("tsdb: {e}")))?;
    let points: Vec<HistoryPoint> = parse_resp(api.dialect, &resp)
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
        if !["avg", "max", "min", "last", "count"].contains(&agg.as_str()) {
            return Err((StatusCode::BAD_REQUEST, "agg must be avg|max|min|last|count".into()));
        }
        if q.interval.is_none() {
            return Err((StatusCode::BAD_REQUEST, "interval required when agg set".into()));
        }
    }
    if let Some(interval) = &q.interval {
        if !valid_interval(interval) {
            return Err((
                StatusCode::BAD_REQUEST,
                "interval must match [0-9]+(s|m|h|d)".into(),
            ));
        }
    }
    Ok(())
}

/// 白名单校验 TDengine INTERVAL 语法（如 5m/1h/30s），防语句塑形注入。
fn valid_interval(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2
        && matches!(b.last(), Some(b's' | b'm' | b'h' | b'd'))
        && b[..b.len() - 1].iter().all(u8::is_ascii_digit)
}

/// GET /api/data/export?device_id=&code=&start=&end=&format=csv|xlsx
/// 导出与 history 同一 SQL 的数据（无分页上限，limit 固定 100000）。
pub async fn export(
    State(api): State<ApiState>,
    axum::Extension(tenant_id): axum::Extension<String>,
    Query(q): Query<HistoryQuery>,
    axum::extract::RawQuery(raw): axum::extract::RawQuery,
) -> axum::response::Response {
    let mut q = q;
    if let Err(e) = validate(&q) {
        return (e.0, e.1).into_response();
    }
    q.limit = 100_000;
    let fmt = raw
        .as_deref()
        .and_then(|r| {
            r.split('&')
                .find(|kv| kv.starts_with("format="))
                .map(|kv| kv.trim_start_matches("format="))
        })
        .unwrap_or("csv");
    let sql = build_sql(api.dialect, &tenant_id, &q);
    let resp = match api.td.query(&sql).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("tsdb: {e}"),
            )
                .into_response()
        }
    };
    let points = match parse_resp(api.dialect, &resp) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "export: parse points failed");
            return (StatusCode::BAD_GATEWAY, "tsdb query failed".to_string()).into_response();
        }
    };
    match fmt {
        "xlsx" => match crate::export::xlsx_of_points(&points) {
            Ok(buf) => axum::response::Response::builder()
                .status(StatusCode::OK)
                .header(
                    axum::http::header::CONTENT_TYPE,
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                )
                .header(
                    axum::http::header::CONTENT_DISPOSITION,
                    "attachment; filename=\"export.xlsx\"",
                )
                .body(axum::body::Body::from(buf))
                .unwrap(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        },
        _ => {
            let csv = crate::export::csv_of_points(&points);
            axum::response::Response::builder()
                .status(StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8")
                .header(
                    axum::http::header::CONTENT_DISPOSITION,
                    "attachment; filename=\"export.csv\"",
                )
                .body(axum::body::Body::from(csv))
                .unwrap()
        }
    }
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

/// 按方言分发 SQL 组装（TDengine 默认，ClickHouse 见 [`build_ch_history_sql`]）。
pub fn build_sql(dialect: Dialect, tenant_id: &str, q: &HistoryQuery) -> String {
    match dialect {
        Dialect::Clickhouse => build_ch_history_sql(tenant_id, q),
        Dialect::Tdengine => build_history_sql(tenant_id, q),
    }
}

/// 按方言分发响应解析：TDengine REST 列序数组 ↔ ClickHouse JSONEachRow 行对象。
fn parse_resp(dialect: Dialect, resp: &serde_json::Value) -> Result<Vec<HistoryPoint>, String> {
    match dialect {
        Dialect::Clickhouse => crate::td::parse_ch_points(resp),
        Dialect::Tdengine => parse_points(resp),
    }
}

/// ClickHouse 方言版历史查询（TSDB_KIND=clickhouse 时使用）：行为与
/// `build_history_sql` 一致（同租户/设备/属性过滤、epoch 毫秒 ts、value 优先数值列）。
/// 聚合桶用 intDiv 对齐 epoch（等价 TDengine `_wstart` 语义）；FINAL 使
/// ReplacingMergeTree 同 ts 覆盖幂等（重复写入仅保留最后版本）。
pub fn build_ch_history_sql(tenant_id: &str, q: &HistoryQuery) -> String {
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
            let bucket_ms = interval_ms(interval);
            // last：TDengine last(value) = 最新 ts 的值 → argMax(value, ts)
            let expr = match agg.as_str() {
                "last" => "argMax(value, ts)".to_string(),
                other => format!("{}(value)", other.to_uppercase()),
            };
            format!(
                "SELECT intDiv(ts, {bucket_ms}) * {bucket_ms} AS ts, {expr} AS value \
                 FROM iot.devdata FINAL \
                 WHERE {where_clause} GROUP BY ts ORDER BY ts \
                 LIMIT {} OFFSET {}",
                q.limit, q.offset,
            )
        }
        _ => format!(
            "SELECT ts, value, value_str FROM iot.devdata FINAL \
             WHERE {where_clause} ORDER BY ts LIMIT {} OFFSET {}",
            q.limit, q.offset,
        ),
    }
}

/// "5m"/"1h"/"30s"/"2d" → 毫秒（validate 已保证 [0-9]+(s|m|h|d) 格式）。
fn interval_ms(interval: &str) -> i64 {
    let (n, unit) = interval.split_at(interval.len() - 1);
    let n: i64 = n.parse().unwrap_or(1);
    let mult = match unit {
        "s" => 1000,
        "m" => 60_000,
        "h" => 3_600_000,
        _ => 86_400_000, // d
    };
    n * mult
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_agg_builds_count_sql() {
        let q = HistoryQuery {
            device_id: "d1".into(),
            code: "c1".into(),
            start: 0,
            end: 86400_000,
            agg: Some("count".into()),
            interval: Some("1d".into()),
            limit: 100,
            offset: 0,
        };
        let sql = build_history_sql("t1", &q);
        assert!(sql.contains("COUNT(value)"));
        assert!(sql.contains("INTERVAL(1d)"));
        assert!(sql.contains("tenant_id = 't1'"));
    }

    #[test]
    fn count_agg_validation_passes() {
        let q = HistoryQuery {
            device_id: "d1".into(),
            code: "c1".into(),
            start: 0,
            end: 1,
            agg: Some("count".into()),
            interval: Some("1h".into()),
            limit: 100,
            offset: 0,
        };
        assert!(validate(&q).is_ok());
    }

    #[test]
    fn ch_raw_query_uses_final_and_qualified() {
        let q = HistoryQuery {
            device_id: "d1".into(),
            code: "c1".into(),
            start: 0,
            end: 86400_000,
            agg: None,
            interval: None,
            limit: 100,
            offset: 0,
        };
        let sql = build_ch_history_sql("t1", &q);
        assert!(sql.contains("FROM iot.devdata FINAL"));
        assert!(sql.contains("tenant_id = 't1'"));
        assert!(sql.contains("ORDER BY ts"));
        assert!(sql.contains("LIMIT 100 OFFSET 0"));
        assert!(!sql.contains("INTERVAL"), "raw 查询不得含聚合: {sql}");
    }

    #[test]
    fn ch_agg_query_uses_intdiv_buckets() {
        let q = HistoryQuery {
            device_id: "d1".into(),
            code: "c1".into(),
            start: 0,
            end: 86400_000,
            agg: Some("avg".into()),
            interval: Some("5m".into()),
            limit: 100,
            offset: 0,
        };
        let sql = build_ch_history_sql("t1", &q);
        // 5m = 300000ms：桶起点 intDiv 对齐 epoch（等价 TDengine _wstart 语义）
        assert!(sql.contains("intDiv(ts, 300000) * 300000 AS ts"));
        assert!(sql.contains("AVG(value) AS value"));
        assert!(sql.contains("GROUP BY ts"));
        assert!(sql.contains("FINAL"));
    }

    #[test]
    fn ch_last_agg_maps_to_argmax() {
        let q = HistoryQuery {
            device_id: "d1".into(),
            code: "c1".into(),
            start: 0,
            end: 1,
            agg: Some("last".into()),
            interval: Some("1h".into()),
            limit: 10,
            offset: 0,
        };
        let sql = build_ch_history_sql("t1", &q);
        assert!(sql.contains("argMax(value, ts) AS value"));
    }

    #[test]
    fn interval_ms_parses_units() {
        assert_eq!(interval_ms("5m"), 300_000);
        assert_eq!(interval_ms("1h"), 3_600_000);
        assert_eq!(interval_ms("30s"), 30_000);
        assert_eq!(interval_ms("2d"), 172_800_000);
    }
}
