use crate::engine::to_alert_record;
use crate::models::{AlertMessage, AlertRecord, NewRule, Rule};
use ecat_data::{RdbmsClient, Row};
use ecat_data_sqlx::SqlxClient;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct RuleStore {
    pub db: Arc<SqlxClient>,
}

pub const OPERATORS: [&str; 6] = ["gt", "gte", "lt", "lte", "eq", "neq"];

pub fn validate_rule(r: &NewRule) -> Result<(), String> {
    if r.name.trim().is_empty() || r.name.len() > 128 {
        return Err("name must be 1..128 chars".into());
    }
    if r.device_id.trim().is_empty() || r.device_id.len() > 64 {
        return Err("device_id must be 1..64 chars".into());
    }
    if r.code.trim().is_empty() || r.code.len() > 64 {
        return Err("code must be 1..64 chars".into());
    }
    if !OPERATORS.contains(&r.operator.as_str()) {
        return Err(format!("operator must be one of {OPERATORS:?}"));
    }
    if !r.threshold.is_finite() {
        return Err("threshold must be finite".into());
    }
    if let Some(u) = &r.webhook_url {
        if u.len() > 512 || !u.starts_with("http") {
            return Err("webhook_url must start with http and be <= 512 chars".into());
        }
    }
    Ok(())
}

/// 迁移执行（复用 ecat-data-sqlx::execute_script 逐条执行）。
/// 编译期 include_str! 内联，无运行时文件依赖。
const MIGRATION_SQL: &str = include_str!("../migrations/0001_rule_tables.sql");

pub async fn migrate(db: &SqlxClient) -> Result<(), String> {
    db.execute_script(MIGRATION_SQL)
        .await
        .map_err(|e| format!("migrate: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_sql_non_empty() {
        assert!(!MIGRATION_SQL.trim().is_empty());
    }

    #[test]
    fn stats_from_counts_totals() {
        let c = vec![
            ("active".to_string(), 3),
            ("acknowledged".to_string(), 2),
        ];
        assert_eq!(stats_from_counts(&c), (5, 3));
        assert_eq!(stats_from_counts(&[]), (0, 0));
    }
}

impl RuleStore {
    pub fn new(db: Arc<SqlxClient>) -> Self {
        Self { db }
    }

    pub async fn list_rules(&self, tenant_id: &str) -> Result<Vec<Rule>, String> {
        let rows = self
            .db
            .query_with(
                // sqlx Any 不支持 Timestamp 类型，时间列须 CAST 成文本
                "SELECT id, tenant_id, name, device_id, code, operator, threshold, webhook_url, \
                 enabled, CAST(created_at AS CHAR), CAST(updated_at AS CHAR) \
                 FROM rules WHERE tenant_id = ? ORDER BY created_at DESC",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("list rules: {e}"))?;
        Ok(rows.iter().map(rule_from_row).collect())
    }

    pub async fn insert_rule(&self, tenant_id: &str, r: &NewRule) -> Result<Rule, String> {
        validate_rule(r)?;
        let rule = Rule {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id.to_string(),
            name: r.name.clone(),
            device_id: r.device_id.clone(),
            code: r.code.clone(),
            operator: r.operator.clone(),
            threshold: r.threshold,
            webhook_url: r.webhook_url.clone(),
            enabled: r.enabled.unwrap_or(true),
            created_at: String::new(),
            updated_at: String::new(),
        };
        self.db
            .execute_with(
                "INSERT INTO rules (id, tenant_id, name, device_id, code, operator, threshold, webhook_url, enabled) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                &[
                    json!(rule.id),
                    json!(rule.tenant_id),
                    json!(rule.name),
                    json!(rule.device_id),
                    json!(rule.code),
                    json!(rule.operator),
                    json!(rule.threshold),
                    json!(rule.webhook_url),
                    json!(rule.enabled as i64),
                ],
            )
            .await
            .map_err(|e| format!("insert rule: {e}"))?;
        Ok(rule)
    }

    pub async fn update_rule(&self, tenant_id: &str, id: &str, r: &NewRule) -> Result<bool, String> {
        validate_rule(r)?;
        let n = self
            .db
            .execute_with(
                "UPDATE rules SET name = ?, device_id = ?, code = ?, operator = ?, threshold = ?, \
                 webhook_url = ?, enabled = ? WHERE id = ? AND tenant_id = ?",
                &[
                    json!(r.name),
                    json!(r.device_id),
                    json!(r.code),
                    json!(r.operator),
                    json!(r.threshold),
                    json!(r.webhook_url),
                    json!(r.enabled.unwrap_or(true) as i64),
                    json!(id),
                    json!(tenant_id),
                ],
            )
            .await
            .map_err(|e| format!("update rule: {e}"))?;
        Ok(n > 0)
    }

    pub async fn delete_rule(&self, tenant_id: &str, id: &str) -> Result<bool, String> {
        let n = self
            .db
            .execute_with(
                "DELETE FROM rules WHERE id = ? AND tenant_id = ?",
                &[json!(id), json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("delete rule: {e}"))?;
        Ok(n > 0)
    }

    pub async fn insert_alert(&self, msg: &AlertMessage) -> Result<(), String> {
        let rec = to_alert_record(msg);
        self.db
            .execute_with(
                "INSERT INTO alert_records (id, rule_id, tenant_id, device_id, code, operator, threshold, value, status) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active')",
                &[
                    json!(rec.id),
                    json!(rec.rule_id),
                    json!(rec.tenant_id),
                    json!(rec.device_id),
                    json!(rec.code),
                    json!(rec.operator),
                    json!(rec.threshold),
                    json!(rec.value.to_string()),
                ],
            )
            .await
            .map_err(|e| format!("insert alert: {e}"))?;
        Ok(())
    }

    pub async fn list_alerts(&self, tenant_id: &str, status: Option<&str>) -> Result<Vec<AlertRecord>, String> {
        let (sql, params) = match status {
            Some(s) => (
                "SELECT id, rule_id, tenant_id, device_id, code, operator, threshold, value, status, \
                 CAST(created_at AS CHAR) \
                 FROM alert_records WHERE tenant_id = ? AND status = ? ORDER BY created_at DESC LIMIT 100",
                vec![json!(tenant_id), json!(s)],
            ),
            None => (
                "SELECT id, rule_id, tenant_id, device_id, code, operator, threshold, value, status, \
                 CAST(created_at AS CHAR) \
                 FROM alert_records WHERE tenant_id = ? ORDER BY created_at DESC LIMIT 100",
                vec![json!(tenant_id)],
            ),
        };
        let rows = self
            .db
            .query_with(sql, &params)
            .await
            .map_err(|e| format!("list alerts: {e}"))?;
        Ok(rows.iter().map(alert_from_row).collect())
    }

    /// 告警统计：总数 + 未处理（active）数。
    pub async fn stats(&self, tenant_id: &str) -> Result<(i64, i64), String> {
        let rows = self
            .db
            .query_with(
                "SELECT status, COUNT(*) AS n FROM alert_records WHERE tenant_id = ? GROUP BY status",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("alert stats: {e}"))?;
        let counts: Vec<(String, i64)> = rows
            .iter()
            .map(|r| {
                (
                    r.get("status").and_then(Value::as_str).unwrap_or_default().to_string(),
                    r.get("n").and_then(Value::as_i64).unwrap_or(0),
                )
            })
            .collect();
        Ok(stats_from_counts(&counts))
    }

    pub async fn ack_alert(&self, tenant_id: &str, id: &str) -> Result<bool, String> {
        let n = self
            .db
            .execute_with(
                "UPDATE alert_records SET status = 'acknowledged' WHERE id = ? AND tenant_id = ?",
                &[json!(id), json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("ack alert: {e}"))?;
        Ok(n > 0)
    }
}

/// status 计数 → (总数, 未处理数)。acknowledged 只计入总数。
fn stats_from_counts(counts: &[(String, i64)]) -> (i64, i64) {
    let mut total = 0;
    let mut active = 0;
    for (status, n) in counts {
        total += n;
        if status == "active" {
            active += n;
        }
    }
    (total, active)
}

fn rule_from_row(r: &Row) -> Rule {
    Rule {
        id: r.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
        tenant_id: r.get("tenant_id").and_then(Value::as_str).unwrap_or_default().to_string(),
        name: r.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
        device_id: r.get("device_id").and_then(Value::as_str).unwrap_or_default().to_string(),
        code: r.get("code").and_then(Value::as_str).unwrap_or_default().to_string(),
        operator: r.get("operator").and_then(Value::as_str).unwrap_or_default().to_string(),
        threshold: r.get("threshold").and_then(Value::as_f64).unwrap_or(0.0),
        webhook_url: r.get("webhook_url").and_then(Value::as_str).map(str::to_string),
        enabled: r.get("enabled").and_then(Value::as_i64).unwrap_or(0) != 0,
        created_at: r.get("created_at").and_then(Value::as_str).unwrap_or_default().to_string(),
        updated_at: r.get("updated_at").and_then(Value::as_str).unwrap_or_default().to_string(),
    }
}

fn alert_from_row(r: &Row) -> AlertRecord {
    AlertRecord {
        id: r.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
        rule_id: r.get("rule_id").and_then(Value::as_str).unwrap_or_default().to_string(),
        tenant_id: r.get("tenant_id").and_then(Value::as_str).unwrap_or_default().to_string(),
        device_id: r.get("device_id").and_then(Value::as_str).unwrap_or_default().to_string(),
        code: r.get("code").and_then(Value::as_str).unwrap_or_default().to_string(),
        operator: r.get("operator").and_then(Value::as_str).unwrap_or_default().to_string(),
        threshold: r.get("threshold").and_then(Value::as_f64).unwrap_or(0.0),
        value: r
            .get("value")
            .and_then(Value::as_str)
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(Value::Null),
        status: r.get("status").and_then(Value::as_str).unwrap_or_default().to_string(),
        created_at: r.get("created_at").and_then(Value::as_str).unwrap_or_default().to_string(),
    }
}
