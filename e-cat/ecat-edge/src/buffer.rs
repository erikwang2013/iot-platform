//! 本地 SQLite 缓冲（A-2，docs/edge-protocol.md §2）。
//! 单表 buffered_points，主键 (device_id, code, ts) 天然按时间戳去重。
//! 采集先落库再发；发送成功删除；断网补传按 ts 升序。
use rusqlite::{Connection, params};
use std::path::Path;

/// 缓冲表 DDL（与协议文档一致）。
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS buffered_points (\
    device_id TEXT NOT NULL,\
    code TEXT NOT NULL,\
    value_json TEXT NOT NULL,\
    ts INTEGER NOT NULL,\
    PRIMARY KEY (device_id, code, ts)\
)";

/// 单条缓冲点。
#[derive(Debug, Clone, PartialEq)]
pub struct BufferedPoint {
    pub device_id: String,
    pub code: String,
    pub value_json: String,
    pub ts: i64,
}

/// SQLite 缓冲存储。内部 `Connection` 用 `Mutex` 保护（rusqlite `Connection`
/// 非 Sync；边缘量级单连接 + 互斥足够，无需连接池）。
pub struct Buffer {
    conn: std::sync::Mutex<Connection>,
}

impl Buffer {
    /// 打开/创建数据库文件并建表。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("open sqlite: {e}"))?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| format!("init schema: {e}"))?;
        Ok(Self {
            conn: std::sync::Mutex::new(conn),
        })
    }

    /// 内存库（测试用）。
    pub fn in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| format!("open memory: {e}"))?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| format!("init schema: {e}"))?;
        Ok(Self {
            conn: std::sync::Mutex::new(conn),
        })
    }

    /// 写入一条（事务）。主键冲突（同 device/code/ts 已存在）时幂等覆盖 value。
    pub fn insert(&self, p: &BufferedPoint) -> Result<(), String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT INTO buffered_points (device_id, code, value_json, ts) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(device_id, code, ts) DO UPDATE SET value_json = excluded.value_json",
            params![p.device_id, p.code, p.value_json, p.ts],
        )
        .map_err(|e| format!("insert point: {e}"))?;
        Ok(())
    }

    /// 按 ts 升序取最多 `limit` 条未发送数据（断网补传用）。
    pub fn drain_pending(&self, limit: usize) -> Result<Vec<BufferedPoint>, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT device_id, code, value_json, ts FROM buffered_points \
                 ORDER BY ts ASC LIMIT ?1",
            )
            .map_err(|e| format!("prepare drain: {e}"))?;
        let rows = stmt
            .query_map([limit as i64], |r| {
                Ok(BufferedPoint {
                    device_id: r.get(0)?,
                    code: r.get(1)?,
                    value_json: r.get(2)?,
                    ts: r.get(3)?,
                })
            })
            .map_err(|e| format!("query drain: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("collect drain: {e}"))
    }

    /// 发送成功后删除一条（主键定位）。
    pub fn remove(&self, p: &BufferedPoint) -> Result<(), String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "DELETE FROM buffered_points WHERE device_id = ?1 AND code = ?2 AND ts = ?3",
            params![p.device_id, p.code, p.ts],
        )
        .map_err(|e| format!("remove point: {e}"))?;
        Ok(())
    }

    /// 积压条数（心跳上报用）。
    pub fn pending_count(&self) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM buffered_points", [], |r| r.get(0))
            .map_err(|e| format!("count pending: {e}"))?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(device: &str, code: &str, ts: i64, value: &str) -> BufferedPoint {
        BufferedPoint {
            device_id: device.into(),
            code: code.into(),
            value_json: value.into(),
            ts,
        }
    }

    #[test]
    fn insert_drain_remove_roundtrip_in_ts_order() {
        let b = Buffer::in_memory().unwrap();
        // 乱序写入（ts 3,1,2）
        b.insert(&pt("d1", "temp", 3, "3")).unwrap();
        b.insert(&pt("d1", "temp", 1, "1")).unwrap();
        b.insert(&pt("d1", "temp", 2, "2")).unwrap();
        assert_eq!(b.pending_count().unwrap(), 3);
        // 补发按 ts 升序
        let pending = b.drain_pending(10).unwrap();
        let ts: Vec<i64> = pending.iter().map(|p| p.ts).collect();
        assert_eq!(ts, vec![1, 2, 3]);
        // 发送成功删除后计数减少
        b.remove(&pending[0]).unwrap();
        assert_eq!(b.pending_count().unwrap(), 2);
    }

    #[test]
    fn duplicate_same_ts_is_idempotent() {
        let b = Buffer::in_memory().unwrap();
        b.insert(&pt("d1", "temp", 100, "23.5")).unwrap();
        // 同 ts 再次写入 → 覆盖 value 而非新增（幂等去重）
        b.insert(&pt("d1", "temp", 100, "23.6")).unwrap();
        assert_eq!(b.pending_count().unwrap(), 1);
        let pending = b.drain_pending(10).unwrap();
        assert_eq!(pending[0].value_json, "23.6");
    }

    #[test]
    fn drain_respects_limit() {
        let b = Buffer::in_memory().unwrap();
        for ts in 0..10 {
            b.insert(&pt("d1", "temp", ts, &ts.to_string())).unwrap();
        }
        let first = b.drain_pending(4).unwrap();
        assert_eq!(first.len(), 4);
        assert_eq!(first[0].ts, 0);
        assert_eq!(first[3].ts, 3);
    }

    #[test]
    fn file_db_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("ecat-edge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("edge.db");
        {
            let b = Buffer::open(&path).unwrap();
            b.insert(&pt("d1", "temp", 5, "x")).unwrap();
        }
        // 重新打开仍能读到已落库数据（断网不丢）
        let b = Buffer::open(&path).unwrap();
        assert_eq!(b.pending_count().unwrap(), 1);
        assert_eq!(b.drain_pending(10).unwrap()[0].ts, 5);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
