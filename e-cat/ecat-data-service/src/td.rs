use ecat_data_tdengine::TdengineClient;
use ecat_data_tdengine::sql::escape_sql_string;

/// 库/超级表名。写入与查询一律全限定（TdengineClient 用 /rest/sql 无默认库）。
pub const DB: &str = "iot";
pub const STABLE: &str = "devdata";

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

/// 连接（TdengineClient 内部为 TDengine REST /rest/sql + basic auth）。
pub fn connect(url: &str, user: &str, pass: &str) -> TdengineClient {
    TdengineClient::new(url, user, pass)
}
