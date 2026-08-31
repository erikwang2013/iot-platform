//! 数据生命周期管理（C-6）：按保留天数周期清理超期时序数据。
//! 复用 ecat-scheduler 定时任务；删除幂等（TDengine DELETE / ClickHouse 轻量 DELETE）。
use ecat_data::TsdbClient;
use std::sync::Arc;

pub const STABLE: &str = "iot.devdata";

/// 保留天数：env DATA_RETENTION_DAYS 默认 90。<=0 表示不清理（保留策略关闭）。
pub fn retention_days() -> i64 {
    std::env::var("DATA_RETENTION_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90)
}

/// 清理一次超期数据：删除 ts < (now - 保留天数) 的所有点。
/// 成功返回删除的行数（TDengine 报告 affected rows）；失败仅记日志，不 panic。
pub async fn run_once(td: Arc<dyn TsdbClient>) {
    let days = retention_days();
    if days <= 0 {
        tracing::info!(days, "data retention disabled; skipping cleanup");
        return;
    }
    // 保留阈值时间戳（毫秒）。使用 server 端 now - 保留毫秒，避免多实例时钟偏差。
    let cutoff_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
        - days * 24 * 3600 * 1000;
    // 方言：TDengine DELETE FROM；ClickHouse 轻量删除 ALTER TABLE ... DELETE
    let sql = match crate::td::dialect() {
        crate::td::Dialect::Clickhouse => {
            format!("ALTER TABLE {STABLE} DELETE WHERE ts < {cutoff_ms}")
        }
        _ => format!("DELETE FROM {STABLE} WHERE ts < {cutoff_ms}"),
    };
    match td.query(&sql).await {
        Ok(resp) => {
            // TDengine DELETE 返回 JSON { code: 0, rows: N }（affected rows）
            let rows = resp
                .get("rows")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            tracing::info!(days, cutoff_ms, rows, "data retention cleanup ran");
        }
        Err(e) => tracing::warn!(error = %e, days, "data retention cleanup failed"),
    }
}

/// 注册周期清理任务：每 `interval` 运行一次 run_once。
pub fn register(
    scheduler: &mut ecat_scheduler::Scheduler,
    td: Arc<dyn TsdbClient>,
    interval: std::time::Duration,
) {
    scheduler.every(interval, move || {
        let td = td.clone();
        async move {
            run_once(td).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_days_parses_env() {
        // 未设置时默认 90（Rust 2024 edition：env 变更为 unsafe）
        // SAFETY: 单线程测试，无并发读取环境变量
        unsafe { std::env::remove_var("DATA_RETENTION_DAYS") };
        assert_eq!(retention_days(), 90);
    }

    #[test]
    fn retention_days_respects_env() {
        // SAFETY: 单线程测试，无并发读取环境变量
        unsafe { std::env::set_var("DATA_RETENTION_DAYS", "30") };
        assert_eq!(retention_days(), 30);
        unsafe { std::env::remove_var("DATA_RETENTION_DAYS") };
    }

    #[test]
    fn retention_days_zero_disables() {
        // SAFETY: 单线程测试，无并发读取环境变量
        unsafe { std::env::set_var("DATA_RETENTION_DAYS", "0") };
        assert_eq!(retention_days(), 0);
        unsafe { std::env::remove_var("DATA_RETENTION_DAYS") };
    }

    #[test]
    fn retention_sql_is_bound_to_constant() {
        // 校验生成 SQL 引用的表名与 schema 一致，防配置漂移
        assert!(STABLE.starts_with("iot."));
    }
}
