//! 每日汇总报表（C-线）：定时生成前一日的设备/告警/规则汇总（tenant 维度），
//! 存 daily_reports 表，供管理端报表页查询（GET /api/rule/reports）。
//!
//! 生成幂等（同租户同日期已存在则跳过），每小时兜底跑一次——重启或错过
//! 生成时间都会自动补上。日期取数据库侧 CURDATE()，无时区换算问题。
//! 设备在线/离线为生成时刻的快照，告警/规则计数为该自然日累计。

use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use serde_json::{Value, json};
use std::sync::Arc;

/// 报表行（daily_reports 表）。summary 为 JSON：devices_total/online/offline、
/// alerts_total/active、rules_total/enabled。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DailyReport {
    pub id: String,
    pub tenant_id: String,
    pub report_date: String,
    pub summary: Value,
    pub created_at: String,
}

/// 注册周期任务：每小时兜底生成前一日报表（存在则跳过）。
pub fn register(
    scheduler: &mut ecat_scheduler::Scheduler,
    db: Arc<SqlxClient>,
    interval: std::time::Duration,
) {
    scheduler.every(interval, move || {
        let db = db.clone();
        async move {
            let n = run_once(db).await;
            if n > 0 {
                tracing::info!(created = n, "daily report generated");
            }
        }
    });
}

/// 生成前一日报表（所有租户）；已存在跳过。返回新建数。失败仅记日志。
pub async fn run_once(db: Arc<SqlxClient>) -> u64 {
    let Some(date) = yesterday(&db).await else {
        return 0;
    };
    let tenants = match list_tenants(&db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "report: list tenants failed");
            return 0;
        }
    };
    let mut created = 0u64;
    for tenant in tenants {
        match generate(&db, &tenant, &date).await {
            Ok(true) => created += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!(tenant = %tenant, error = %e, "report: generate failed"),
        }
    }
    created
}

/// 本租户报表列表（倒序，最多 30 条；date 可选过滤）。
pub async fn list_reports(
    db: &SqlxClient,
    tenant: &str,
    date: Option<&str>,
) -> Result<Vec<DailyReport>, String> {
    let (sql, params) = match date {
        Some(d) => (
            "SELECT id, tenant_id, CAST(report_date AS CHAR) AS report_date, summary, \
             CAST(created_at AS CHAR) AS created_at \
             FROM daily_reports WHERE tenant_id = ? AND report_date = ? \
             ORDER BY report_date DESC",
            vec![json!(tenant), json!(d)],
        ),
        None => (
            "SELECT id, tenant_id, CAST(report_date AS CHAR) AS report_date, summary, \
             CAST(created_at AS CHAR) AS created_at \
             FROM daily_reports WHERE tenant_id = ? ORDER BY report_date DESC LIMIT 30",
            vec![json!(tenant)],
        ),
    };
    let rows = db
        .query_with(sql, &params)
        .await
        .map_err(|e| format!("list reports: {e}"))?;
    Ok(rows
        .iter()
        .map(|r| DailyReport {
            id: r
                .get("id")
                .and_then(Value::as_i64)
                .map(|n| n.to_string())
                .unwrap_or_default(),
            tenant_id: r.get("tenant_id").and_then(Value::as_str).unwrap_or("").to_string(),
            report_date: r.get("report_date").and_then(Value::as_str).unwrap_or("").to_string(),
            summary: r.get("summary").cloned().unwrap_or(Value::Null),
            created_at: r.get("created_at").and_then(Value::as_str).unwrap_or("").to_string(),
        })
        .collect())
}

/// 数据库侧取前一自然日（YYYY-MM-DD），避免 Rust 端时区换算。
async fn yesterday(db: &SqlxClient) -> Option<String> {
    let rows = db
        .query_with(
            "SELECT DATE_FORMAT(DATE_SUB(CURDATE(), INTERVAL 1 DAY), '%Y-%m-%d') AS d",
            &[],
        )
        .await
        .ok()?;
    rows.first()
        .and_then(|r| r.get("d"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

async fn list_tenants(db: &SqlxClient) -> Result<Vec<String>, String> {
    let rows = db
        .query_with("SELECT id FROM tenants", &[])
        .await
        .map_err(|e| format!("list tenants: {e}"))?;
    Ok(rows
        .iter()
        .filter_map(|r| r.get("id").and_then(Value::as_str).map(str::to_string))
        .collect())
}

/// 单租户单日报表：已存在返回 Ok(false)，否则统计并落库。
async fn generate(db: &SqlxClient, tenant: &str, date: &str) -> Result<bool, String> {
    let exists = db
        .query_with(
            "SELECT COUNT(*) AS n FROM daily_reports WHERE tenant_id = ? AND report_date = ?",
            &[json!(tenant), json!(date)],
        )
        .await
        .map_err(|e| format!("report exists check: {e}"))?;
    let n = exists
        .first()
        .and_then(|r| r.get("n"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if n > 0 {
        return Ok(false);
    }
    let (d_total, d_online) = counts(
        db,
        "SELECT COUNT(*) AS total, COALESCE(SUM(status = 'online'), 0) AS extra \
         FROM devices WHERE tenant_id = ?",
        tenant,
        None,
    )
    .await?;
    let (a_total, a_active) = counts(
        db,
        "SELECT COUNT(*) AS total, COALESCE(SUM(status = 'active'), 0) AS extra \
         FROM alert_records WHERE tenant_id = ? \
         AND created_at >= CONCAT(?, ' 00:00:00') AND created_at < DATE_ADD(?, INTERVAL 1 DAY)",
        tenant,
        Some(date),
    )
    .await?;
    let (r_total, r_enabled) = counts(
        db,
        "SELECT COUNT(*) AS total, COALESCE(SUM(enabled = 1), 0) AS extra \
         FROM rules WHERE tenant_id = ?",
        tenant,
        None,
    )
    .await?;
    let summary = json!({
        "devices_total": d_total,
        "devices_online": d_online,
        "devices_offline": d_total - d_online,
        "alerts_total": a_total,
        "alerts_active": a_active,
        "rules_total": r_total,
        "rules_enabled": r_enabled,
    });
    // daily_reports.id 为 BIGINT 列：绑定数字
    let id = ecat::ids::next_id();
    db.execute_with(
        "INSERT INTO daily_reports (id, tenant_id, report_date, summary) VALUES (?, ?, ?, ?)",
        &[json!(id), json!(tenant), json!(date), json!(summary)],
    )
    .await
    .map_err(|e| format!("insert report: {e}"))?;
    Ok(true)
}

/// 执行带租户参数的统计查询（date 为 Some 时追加两个日期参数），返回 (total, extra)。
async fn counts(
    db: &SqlxClient,
    sql: &str,
    tenant: &str,
    date: Option<&str>,
) -> Result<(i64, i64), String> {
    let mut params = vec![json!(tenant)];
    if let Some(d) = date {
        params.push(json!(d));
        params.push(json!(d));
    }
    let rows = db
        .query_with(sql, &params)
        .await
        .map_err(|e| format!("count query: {e}"))?;
    let row = rows.first().ok_or_else(|| "count query returned no row".to_string())?;
    let total = row.get("total").and_then(Value::as_i64).unwrap_or(0);
    let extra = row.get("extra").and_then(Value::as_i64).unwrap_or(0);
    Ok((total, extra))
}
