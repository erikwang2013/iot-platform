// Copyright (c) 2026 erik <erik@erik.xyz> — https://erik.xyz

/// 单引号字符串字面量转义（TDengine/MySQL 方言）：反斜杠 + 单引号。
pub fn escape_sql_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

/// 历史曲线单点：ts 为 epoch 毫秒，value 为原始值（数值或字符串）。
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct TsPoint {
    pub ts: i64,
    pub value: serde_json::Value,
}

/// ts 毫秒解析：REST 返回数字毫秒或数字字符串，兼容 "YYYY-MM-DD HH:MM:SS.mmm"。
pub fn ts_to_ms(v: &serde_json::Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(n);
    }
    let s = v.as_str()?;
    if let Ok(n) = s.trim().parse::<i64>() {
        return Some(n);
    }
    // "2023-07-22 09:46:40.000" → epoch 毫秒（UTC 语义由 TDengine 决定）
    if s.len() >= 19 {
        let mut it = s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty());
        let (y, mo, d, h, mi, se) = (
            it.next()?.parse::<i64>().ok()?,
            it.next()?.parse::<i64>().ok()?,
            it.next()?.parse::<i64>().ok()?,
            it.next()?.parse::<i64>().ok()?,
            it.next()?.parse::<i64>().ok()?,
            it.next()?.parse::<i64>().ok()?,
        );
        let ms = s.rsplit('.').next().and_then(|p| p.parse::<i64>().ok()).unwrap_or(0);
        // 精确的 days-from-civil（Howard Hinnant 算法），正确处理闰年/月长
        let days = days_from_civil(y, mo, d);
        Some((((days * 24 + h) * 60 + mi) * 60 + se) * 1000 + ms)
    } else {
        None
    }
}

/// REST 响应 → 历史点。列序固定 ts/value/value_str（见 column_meta）。
pub fn parse_points(resp: &serde_json::Value) -> Result<Vec<TsPoint>, String> {
    if resp["code"].as_i64() != Some(0) {
        return Err(format!("tdengine error: {}", resp["desc"]));
    }
    let mut points = Vec::new();
    for row in resp["data"].as_array().unwrap_or(&Vec::new()) {
        let arr = match row.as_array() {
            Some(a) if a.len() >= 3 => a,
            _ => continue,
        };
        let ts = ts_to_ms(&arr[0]).ok_or_else(|| "bad ts in row".to_string())?;
        let value = if arr[1].is_null() { arr[2].clone() } else { arr[1].clone() };
        points.push(TsPoint { ts, value });
    }
    Ok(points)
}

/// 公历日期 → 距 1970-01-01 的天数（Howard Hinnant days_from_civil，精确）。
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn escape_sql_string_escapes_backslash_and_quote() {
        assert_eq!(escape_sql_string("a\\b'c"), "a\\\\b\\'c");
        assert_eq!(escape_sql_string("plain"), "plain");
        // 注入载荷不得逃出字面量
        assert_eq!(
            escape_sql_string("x' OR 1=1 --"),
            "x\\' OR 1=1 --"
        );
    }

    #[test]
    fn ts_to_ms_numeric_and_string() {
        assert_eq!(ts_to_ms(&json!(1700000000000i64)), Some(1700000000000));
        assert_eq!(ts_to_ms(&json!("1700000000000")), Some(1700000000000));
        assert_eq!(ts_to_ms(&json!(null)), None);
        assert_eq!(ts_to_ms(&json!("not a ts")), None);
    }

    #[test]
    fn ts_to_ms_parses_datetime_string() {
        // 2023-07-22 09:46:40.000 UTC
        assert_eq!(ts_to_ms(&json!("2023-07-22 09:46:40.000")), Some(1690019200000i64));
        // 闰年 2024-02-29 可解析
        assert_eq!(ts_to_ms(&json!("2024-02-29 00:00:00.000")), Some(1709164800000i64));
    }

    #[test]
    fn parse_points_extracts_ts_and_value() {
        let resp = json!({
            "code": 0,
            "column_meta": [["ts", "TIMESTAMP", 8], ["value", "DOUBLE", 8], ["value_str", "NCHAR", 64]],
            "data": [
                [1690019200000i64, 12.5, null],
                ["2023-07-22 09:46:41.000", null, "offline"]
            ]
        });
        let points = parse_points(&resp).unwrap();
        assert_eq!(
            points,
            vec![
                TsPoint { ts: 1690019200000i64, value: json!(12.5) },
                TsPoint { ts: 1690019201000i64, value: json!("offline") },
            ]
        );
    }

    #[test]
    fn parse_points_reports_tdengine_error() {
        let resp = json!({"code": -1, "desc": "syntax error near x"});
        let err = parse_points(&resp).unwrap_err();
        assert!(err.contains("syntax error"), "got: {err}");
    }

    #[test]
    fn parse_points_skips_short_rows_and_fails_on_bad_ts() {
        let resp = json!({"code": 0, "data": [[1, 2.0, null], ["short"]]});
        let points = parse_points(&resp).unwrap();
        assert_eq!(points, vec![TsPoint { ts: 1, value: json!(2.0) }]);

        let bad = json!({"code": 0, "data": [[1, 2.0, null], ["not-a-ts", 1.0, null]]});
        assert!(parse_points(&bad).is_err());
    }
}
