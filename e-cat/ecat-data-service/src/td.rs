use ecat_data_tdengine::TdengineClient;
use ecat_data_tdengine::sql::{TsPoint, escape_sql_string, ts_to_ms};
use std::sync::Arc;

/// 库/超级表名。写入与查询一律全限定（TdengineClient 用 /rest/sql 无默认库）。
pub const DB: &str = "iot";
pub const STABLE: &str = "devdata";

/// 幂等建库建表（按 `TSDB_KIND` 选方言）。TDengine 超级表列：ts 主时间列，
/// value 数值列，value_str 字符串列（非数值 JSON 序列化后存入）；TAGS 用于
/// 租户/设备/属性过滤。ClickHouse 见 [`ch_schema_sqls`]。
pub fn schema_sqls() -> Vec<String> {
    match tsdb_kind().as_str() {
        "clickhouse" => ch_schema_sqls(),
        _ => vec![
            format!("CREATE DATABASE IF NOT EXISTS {DB} KEEP 365 DAYS"),
            format!(
                "CREATE STABLE IF NOT EXISTS {DB}.{STABLE} \
                 (ts TIMESTAMP, value DOUBLE, value_str NCHAR(255)) \
                 TAGS (tenant_id NCHAR(64), device_id NCHAR(64), code NCHAR(64))"
            ),
        ],
    }
}

/// ClickHouse 建库建表（幂等，B-4）。列与 TDengine 超级表对齐（ts/value/value_str/
/// 三个 tag 列）；ReplacingMergeTree + ORDER BY 全维度：同 (tenant, device, code, ts)
/// 重复写入合并后仅保留最后版本——等价 TDengine 同 ts 覆盖幂等，查询侧用 FINAL 读取。
pub fn ch_schema_sqls() -> Vec<String> {
    vec![
        format!("CREATE DATABASE IF NOT EXISTS {DB}"),
        format!(
            "CREATE TABLE IF NOT EXISTS {DB}.{STABLE} \
             (ts Int64, value Nullable(Float64), value_str String, \
              tenant_id String, device_id String, code String) \
             ENGINE = ReplacingMergeTree \
             ORDER BY (tenant_id, device_id, code, ts)"
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

/// 查询方言（与 `TSDB_KIND` 联动）。TDengine 用 INTERVAL/_wstart 与 REST 列序响应，
/// ClickHouse 用 intDiv 桶对齐 + FINAL。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialect {
    Tdengine,
    Clickhouse,
}

pub fn dialect() -> Dialect {
    match tsdb_kind().as_str() {
        "clickhouse" => Dialect::Clickhouse,
        _ => Dialect::Tdengine,
    }
}

/// ClickHouse JSONEachRow 响应（`TsdbClient::query` 返回行对象数组）→ 历史点。
/// 行格式 `{"ts":<epoch 毫秒>,"value":<数值|null>,"value_str":"..."}`，与 TDengine
/// `parse_points` 语义一致：value 非空取 value，否则取 value_str。
pub fn parse_ch_points(resp: &serde_json::Value) -> Result<Vec<TsPoint>, String> {
    let arr = resp
        .as_array()
        .ok_or_else(|| format!("clickhouse: expected row array, got: {resp}"))?;
    let mut points = Vec::new();
    for row in arr {
        let obj = row
            .as_object()
            .ok_or_else(|| "clickhouse: row is not an object".to_string())?;
        let ts = obj
            .get("ts")
            .and_then(ts_to_ms)
            .ok_or_else(|| "clickhouse: bad ts in row".to_string())?;
        let value = match obj.get("value") {
            Some(v) if !v.is_null() => v.clone(),
            _ => obj
                .get("value_str")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        };
        points.push(TsPoint { ts, value });
    }
    Ok(points)
}

/// 按 `TSDB_KIND` 选择并建立时序存储客户端，返回 trait 对象。
/// - tdengine（默认）：`TDENGINE_URL` / `TDENGINE_USER` / `TDENGINE_PASS`
/// - clickhouse：`CLICKHOUSE_URL`（含 database query 参数或 `/` 后为库名）
///
/// 查询层（api.rs history/export）按 [`dialect()`] 分发两种方言 SQL；写入侧
/// ingest 目前仍走 TDengine 直连 SQL，ClickHouse 写入路径为验证型后续。
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
