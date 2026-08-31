use ecat_data_tdengine::sql::{escape_sql_string, parse_points, ts_to_ms};
use ecat_data_service::td::{dialect, parse_ch_points, schema_sqls, tsdb_kind, Dialect};
use serde_json::json;
use std::sync::{Mutex, MutexGuard};

/// TSDB_KIND 是进程级全局 env，同二进制并行测试线程互踩会串扰（flaky）。
/// 文件内串行化所有触碰它的测试：guard 持有锁直到 Drop，并还原旧值。
static TSDB_ENV_LOCK: Mutex<()> = Mutex::new(());

struct TsdbEnvGuard {
    _lock: MutexGuard<'static, ()>,
    old: Option<String>,
}

impl TsdbEnvGuard {
    /// 取锁后设置 TSDB_KIND（None 清除）；Drop 时还原旧值。
    fn with(kind: Option<&str>) -> Self {
        let _lock = TSDB_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let old = std::env::var("TSDB_KIND").ok();
        match kind {
            Some(k) => unsafe { std::env::set_var("TSDB_KIND", k) },
            None => unsafe { std::env::remove_var("TSDB_KIND") },
        }
        Self { _lock, old }
    }
}

impl Drop for TsdbEnvGuard {
    fn drop(&mut self) {
        match &self.old {
            Some(k) => unsafe { std::env::set_var("TSDB_KIND", k) },
            None => unsafe { std::env::remove_var("TSDB_KIND") },
        }
    }
}

#[test]
fn escape_blocks_sql_injection() {
    assert_eq!(escape_sql_string("o'brien"), "o\\'brien");
    assert_eq!(escape_sql_string("a\\b"), "a\\\\b");
    assert_eq!(escape_sql_string("safe"), "safe");
    // 注入载荷不得逃出字面量
    assert_eq!(
        escape_sql_string("x' OR 1=1 --"),
        "x\\' OR 1=1 --"
    );
}

#[test]
fn schema_sqls_are_idempotent_and_qualified() {
    // 默认方言（tdengine）下生成 DDL
    let _env = TsdbEnvGuard::with(None);
    let sqls = schema_sqls();
    assert_eq!(sqls.len(), 2);
    assert!(sqls[0].contains("CREATE DATABASE IF NOT EXISTS iot"));
    assert!(sqls[1].contains("CREATE STABLE IF NOT EXISTS iot.devdata"));
    assert!(sqls[1].contains("TAGS"));
}

#[test]
fn clickhouse_schema_sqls_are_idempotent_and_qualified() {
    let _env = TsdbEnvGuard::with(Some("clickhouse"));
    let sqls = schema_sqls();
    assert_eq!(sqls.len(), 2);
    assert!(sqls[0].contains("CREATE DATABASE IF NOT EXISTS iot"));
    assert!(sqls[1].contains("CREATE TABLE IF NOT EXISTS iot.devdata"));
    assert!(sqls[1].contains("ReplacingMergeTree"));
    assert!(sqls[1].contains("ORDER BY (tenant_id, device_id, code, ts)"));
}

#[test]
fn dialect_follows_tsdb_kind() {
    // 顺序取两个 guard：drop 第一个再取第二个（同一锁上重入会死锁）
    let clickhouse = TsdbEnvGuard::with(Some("clickhouse"));
    assert_eq!(dialect(), Dialect::Clickhouse);
    drop(clickhouse);
    let _default = TsdbEnvGuard::with(None);
    assert_eq!(dialect(), Dialect::Tdengine);
}

#[test]
fn parse_ch_points_extracts_rows() {
    // ClickHouse JSONEachRow 行对象数组（TsdbClient::query 返回格式）
    let resp = json!([
        {"ts": 1690000000000i64, "value": 23.5, "value_str": ""},
        {"ts": 1690000000001i64, "value": null, "value_str": "true"},
        {"ts": 1690000000002i64, "value": 1.0, "value_str": "x"}
    ]);
    let points = parse_ch_points(&resp).unwrap();
    assert_eq!(points.len(), 3);
    assert_eq!(points[0].ts, 1690000000000);
    assert_eq!(points[0].value, json!(23.5));
    assert_eq!(points[1].ts, 1690000000001);
    assert_eq!(points[1].value, json!("true"));
    assert_eq!(points[2].value, json!(1.0));
}

#[test]
fn parse_ch_points_rejects_bad_input() {
    assert!(parse_ch_points(&json!({"code": 1})).is_err(), "非数组报错");
    assert!(parse_ch_points(&json!([{"ts": "nope", "value": null}])).is_err(), "坏 ts 报错");
}

#[test]
fn ts_to_ms_accepts_number_and_string() {
    assert_eq!(ts_to_ms(&json!(1690000000000i64)), Some(1690000000000i64));
    assert_eq!(ts_to_ms(&json!("1690000000000")), Some(1690000000000i64));
    // 时间字符串（TDengine REST 字符串格式）可解析；2023-07-22 09:46:40Z = 1690019200000
    assert_eq!(ts_to_ms(&json!("2023-07-22 09:46:40.000")), Some(1690019200000i64));
    assert_eq!(ts_to_ms(&json!("nope")), None);
}

#[test]
fn parse_points_extracts_rows_in_column_order() {
    // REST /rest/sql 响应：column_meta 定义列序 ts/value/value_str
    let resp = json!({
        "code": 0,
        "column_meta": [
            ["ts", "TIMESTAMP", 8],
            ["value", "DOUBLE", 8],
            ["value_str", "NCHAR", 8]
        ],
        "data": [
            [1690000000000i64, 23.5, null],
            [1690000000001i64, null, "true"]
        ]
    });
    let points = parse_points(&resp).unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(points[0].ts, 1690000000000);
    assert_eq!(points[0].value, json!(23.5));
    assert_eq!(points[1].ts, 1690000000001);
    assert_eq!(points[1].value, json!("true"));
}

#[test]
fn parse_points_rejects_error_response() {
    let resp = json!({"code": 1, "desc": "syntax error"});
    assert!(parse_points(&resp).is_err());
}

#[test]
fn tsdb_kind_defaults_to_tdengine() {
    let _env = TsdbEnvGuard::with(None);
    assert_eq!(tsdb_kind(), "tdengine");
}

#[test]
fn tsdb_kind_respects_env_case_insensitive() {
    let _env = TsdbEnvGuard::with(Some("ClickHouse"));
    assert_eq!(tsdb_kind(), "clickhouse");
}
