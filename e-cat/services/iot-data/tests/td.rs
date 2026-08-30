use iot_data::td::{escape_sql_string, parse_points, schema_sqls, ts_to_ms};
use serde_json::json;

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
    let sqls = schema_sqls();
    assert_eq!(sqls.len(), 2);
    assert!(sqls[0].contains("CREATE DATABASE IF NOT EXISTS iot"));
    assert!(sqls[1].contains("CREATE STABLE IF NOT EXISTS iot.devdata"));
    assert!(sqls[1].contains("TAGS"));
}

#[test]
fn ts_to_ms_accepts_number_and_string() {
    assert_eq!(ts_to_ms(&json!(1690000000000i64)), Some(1690000000000));
    assert_eq!(ts_to_ms(&json!("1690000000000")), Some(1690000000000));
    // 时间字符串（TDengine REST 字符串格式）可解析；2023-07-22 09:46:40Z = 1690019200000
    assert_eq!(ts_to_ms(&json!("2023-07-22 09:46:40.000")), Some(1690019200000));
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
