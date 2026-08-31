use ecat_data_tdengine::TdengineClient;
use ecat_data_tdengine::sql::escape_sql_string;
use std::sync::Arc;

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

/// 时序存储后端（B-4）：env `TSDB_KIND` 取值 `tdengine`（默认）| `clickhouse`。
pub fn tsdb_kind() -> String {
    std::env::var("TSDB_KIND")
        .unwrap_or_else(|_| "tdengine".into())
        .to_ascii_lowercase()
}

/// 按 `TSDB_KIND` 选择并建立时序存储客户端，返回 trait 对象。
/// - tdengine（默认）：`TDENGINE_URL` / `TDENGINE_USER` / `TDENGINE_PASS`
/// - clickhouse：`CLICKHOUSE_URL`（含 database query 参数或 `/` 后为库名）
///
/// 注意：当前 history/export 的查询 SQL 为 TDengine 方言（INTERVAL 聚合、
/// `parse_points`），切到 clickhouse 后需按 ClickHouse 语法重写查询层。
/// 此处先落地"客户端选择 + 幂等建表"的抽象，全链路 SQL 对齐属验证型后续。
pub async fn connect_tsdb() -> Arc<dyn ecat_data::TsdbClient> {
    match tsdb_kind().as_str() {
        "clickhouse" => {
            let url = std::env::var("CLICKHOUSE_URL")
                .unwrap_or_else(|_| "http://localhost:8123".into());
            let db = std::env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "iot".into());
            Arc::new(ecat_data_clickhouse::ClickhouseClient::new(url, db))
        }
        _ => {
            let url = std::env::var("TDENGINE_URL").unwrap_or_else(|_| "http://localhost:6041".into());
            let user = std::env::var("TDENGINE_USER").unwrap_or_else(|_| "root".into());
            let pass = std::env::var("TDENGINE_PASS").unwrap_or_else(|_| "taosdata".into());
            Arc::new(connect(&url, &user, &pass))
        }
    }
}
