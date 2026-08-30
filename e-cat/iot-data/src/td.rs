use crate::models::HistoryPoint;
use ecat_data_tdengine::TdengineClient;

/// 库/超级表名。写入与查询一律全限定（TdengineClient 用 /rest/sql 无默认库）。
pub const DB: &str = "iot";
pub const STABLE: &str = "devdata";

/// TDengine 字符串字面量转义（单引号包裹）：反斜杠 + 单引号。
pub fn escape_sql_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

/// 幂等建库建表。超级表列：ts 主时间列，value 数值列，value_str 字符串列
/// （非数值 JSON 序列化后存入）；TAGS 用于租户/设备/属性过滤。
pub fn schema_sqls() -> Vec<String> {
    vec![
        format!("CREATE DATABASE IF NOT EXISTS {DB} KEEP 365 DAYS"),
        format!(
            "CREATE STABLE IF NOT EXISTS {DB}.{STABLE} \
             (ts TIMESTAMP, value DOUBLE, value_str NCHAR(255)) \
             TAGS (tenant_id NCHAR(64), device_id NCHAR(64), code NCHAR(64))"
        ),
    ]
}

/// 事件 → 超级表 INSERT 语句。子表名由哈希生成（确定性、仅需唯一）。
/// 数值写 value 列；字符串/布尔/其他 JSON 写 value_str 列（JSON 文本）。
pub fn event_to_insert(ev: &crate::models::EventMessage) -> String {
    let (val, val_str) = match &ev.value {
        serde_json::Value::Number(n) => (n.to_string(), "NULL".to_string()),
        other => {
            let s = if other.is_null() { "null".to_string() } else { other.to_string() };
            ("NULL".to_string(), format!("'{}'", escape_sql_string(&s)))
        }
    };
    // 子表名：device_id+code 的确定性哈希（仅需唯一；DefaultHasher 跨版本输出不保证稳定）
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    format!("{}_{}", ev.device_id, ev.code).hash(&mut h);
    let tbl = format!("d_{:016x}", h.finish());
    format!(
        "INSERT INTO {tbl} USING {DB}.{STABLE} TAGS ('{}', '{}', '{}') \
         VALUES ({}, {}, {})",
        escape_sql_string(&ev.tenant_id),
        escape_sql_string(&ev.device_id),
        escape_sql_string(&ev.code),
        ev.ts,
        val,
        val_str,
    )
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
pub fn parse_points(resp: &serde_json::Value) -> Result<Vec<HistoryPoint>, String> {
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
        points.push(HistoryPoint { ts, value });
    }
    Ok(points)
}

/// 连接（TdengineClient 内部为 TDengine REST /rest/sql + basic auth）。
pub fn connect(url: &str, user: &str, pass: &str) -> TdengineClient {
    TdengineClient::new(url, user, pass)
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
