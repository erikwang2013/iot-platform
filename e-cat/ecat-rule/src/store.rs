use crate::engine::to_alert_record;
use crate::models::{AlertMessage, AlertRecord, NewRule, NotifyChannel, NewNotifyChannel, Rule};
use ecat_data::{RdbmsClient, Row};
use ecat_data_sqlx::SqlxClient;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct RuleStore {
    pub db: Arc<SqlxClient>,
}

pub const OPERATORS: [&str; 6] = ["gt", "gte", "lt", "lte", "eq", "neq"];

pub const CHANNELS: [&str; 4] = ["email", "dingtalk", "wecom", "sms"];

fn config_str(c: &serde_json::Value, key: &str) -> Option<String> {
    c.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
}

fn validate_email_config(c: &serde_json::Value) -> Result<(), String> {
    let host = config_str(c, "smtp_host").ok_or("smtp_host required")?;
    if host.len() > 255 || host.contains(' ') {
        return Err("smtp_host invalid".into());
    }
    if let Some(p) = c.get("smtp_port").and_then(|v| v.as_i64()) {
        if !(1..=65535).contains(&p) {
            return Err("smtp_port must be 1..65535".into());
        }
    }
    for key in ["mail_from", "mail_to"] {
        let addr = config_str(c, key).ok_or(format!("{key} required"))?;
        if addr.len() > 255 || !addr.contains('@') {
            return Err(format!("{key} must be a valid email address"));
        }
    }
    Ok(())
}

fn validate_webhook_config(c: &serde_json::Value) -> Result<(), String> {
    let url = config_str(c, "webhook_url").ok_or("webhook_url required")?;
    if url.len() > 512 || !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("webhook_url must start with http(s):// and be <= 512 chars".into());
    }
    Ok(())
}

/// 短信渠道（A-1）：经 HTTP API 发送（阿里云/腾讯云短信等，POST JSON）。
/// config 字段：api_url（短信服务商 HTTP 端点）、phone（接收手机号）、
/// sign（短信签名）、template_id（模板 ID）。
fn validate_sms_config(c: &serde_json::Value) -> Result<(), String> {
    let url = config_str(c, "api_url").ok_or("api_url required")?;
    if url.len() > 512 || !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("api_url must start with http(s):// and be <= 512 chars".into());
    }
    let phone = config_str(c, "phone").ok_or("phone required")?;
    if phone.len() > 20 || !phone.chars().all(|ch| ch.is_ascii_digit() || ch == '+') {
        return Err("phone must be a valid phone number".into());
    }
    if config_str(c, "sign").is_none() {
        return Err("sign required".into());
    }
    if config_str(c, "template_id").is_none() {
        return Err("template_id required".into());
    }
    Ok(())
}

/// 通知渠道校验：渠道名白名单 + 按渠道校验 config 结构。
pub fn validate_channel(channel: &str, c: &serde_json::Value) -> Result<(), String> {
    if !c.is_object() {
        return Err("config must be a JSON object".into());
    }
    match channel {
        "email" => validate_email_config(c),
        "dingtalk" | "wecom" => validate_webhook_config(c),
        "sms" => validate_sms_config(c),
        _ => Err(format!("channel must be one of {CHANNELS:?}")),
    }
}

/// 迁移执行（复用 ecat-data-sqlx::execute_script 逐条执行）。
/// 编译期 include_str! 内联，无运行时文件依赖。
const MIGRATION_SQL: [&str; 2] = [
    include_str!("../migrations/0001_rule_tables.sql"),
    include_str!("../migrations/0002_notify_channels.sql"),
];

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
    // D-3 联动：action 三件套必须齐全（device_id + code + value）或不配置
    let action_fields = [
        r.action_device_id.is_some(),
        r.action_code.is_some(),
        r.action_value.is_some(),
    ];
    let any_action = action_fields.iter().any(|&f| f);
    let all_action = action_fields.iter().all(|&f| f);
    if any_action && !all_action {
        return Err("action requires action_device_id, action_code, action_value together".into());
    }
    if let Some(id) = &r.action_device_id {
        if id.len() > 64 {
            return Err("action_device_id must be <= 64 chars".into());
        }
    }
    if let Some(code) = &r.action_code {
        if code.len() > 64 {
            return Err("action_code must be <= 64 chars".into());
        }
    }
    Ok(())
}

pub async fn migrate(db: &SqlxClient) -> Result<(), String> {
    for sql in MIGRATION_SQL {
        db.execute_script(sql)
            .await
            .map_err(|e| format!("migrate: {e}"))?;
    }
    // D-3 联动规则：老库补列（MySQL 8 无 ADD COLUMN IF NOT EXISTS，先查
    // information_schema，不存在才 ALTER；新装库由 0001 CREATE TABLE 直接建全列）。
    for (column, ddl) in RULE_ACTION_EXTRA_COLUMNS {
        let exists = db
            .query_with(
                "SELECT COUNT(*) AS n FROM information_schema.COLUMNS \
                 WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'rules' AND COLUMN_NAME = ?",
                &[json!(column)],
            )
            .await
            .map_err(|e| format!("check column {column}: {e}"))?;
        let n = exists
            .first()
            .and_then(|r| r.get("n"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if n == 0 {
            db.execute_with(&format!("ALTER TABLE rules ADD COLUMN {ddl}"), &[])
                .await
                .map_err(|e| format!("alter rules add {column}: {e}"))?;
        }
    }
    Ok(())
}

/// D-3 联动规则新增列（老库补列；新库由 0001 CREATE TABLE 直接建）。
const RULE_ACTION_EXTRA_COLUMNS: [(&str, &str); 3] = [
    ("action_device_id", "action_device_id VARCHAR(64) NULL"),
    ("action_code", "action_code VARCHAR(64) NULL"),
    ("action_value", "action_value JSON NULL"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_sql_non_empty() {
        for sql in MIGRATION_SQL {
            assert!(!sql.trim().is_empty());
        }
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
                 action_device_id, action_code, action_value, enabled, \
                 CAST(created_at AS CHAR), CAST(updated_at AS CHAR) \
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
            action_device_id: r.action_device_id.clone(),
            action_code: r.action_code.clone(),
            action_value: r.action_value.clone(),
            enabled: r.enabled.unwrap_or(true),
            created_at: String::new(),
            updated_at: String::new(),
        };
        self.db
            .execute_with(
                "INSERT INTO rules (id, tenant_id, name, device_id, code, operator, threshold, \
                 webhook_url, action_device_id, action_code, action_value, enabled) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                &[
                    json!(rule.id),
                    json!(rule.tenant_id),
                    json!(rule.name),
                    json!(rule.device_id),
                    json!(rule.code),
                    json!(rule.operator),
                    json!(rule.threshold),
                    json!(rule.webhook_url),
                    json!(rule.action_device_id),
                    json!(rule.action_code),
                    json!(rule.action_value),
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
                 webhook_url = ?, action_device_id = ?, action_code = ?, action_value = ?, \
                 enabled = ? WHERE id = ? AND tenant_id = ?",
                &[
                    json!(r.name),
                    json!(r.device_id),
                    json!(r.code),
                    json!(r.operator),
                    json!(r.threshold),
                    json!(r.webhook_url),
                    json!(r.action_device_id),
                    json!(r.action_code),
                    json!(r.action_value),
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

    pub async fn list_channels(&self, tenant_id: &str) -> Result<Vec<NotifyChannel>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT id, tenant_id, channel, config, enabled, CAST(created_at AS CHAR), \
                 CAST(updated_at AS CHAR) FROM notify_channels WHERE tenant_id = ? \
                 ORDER BY created_at",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("list channels: {e}"))?;
        Ok(rows.iter().map(channel_from_row).collect())
    }

    /// 单租户单渠道唯一（UNIQUE KEY），重复 PUT 即更新。
    pub async fn upsert_channel(
        &self,
        tenant_id: &str,
        channel: &str,
        req: &NewNotifyChannel,
    ) -> Result<NotifyChannel, String> {
        validate_channel(channel, &req.config)?;
        let id = uuid::Uuid::new_v4().to_string();
        self.db
            .execute_with(
                "INSERT INTO notify_channels (id, tenant_id, channel, config, enabled) \
                 VALUES (?, ?, ?, ?, ?) \
                 ON DUPLICATE KEY UPDATE config = VALUES(config), enabled = VALUES(enabled)",
                &[
                    json!(id),
                    json!(tenant_id),
                    json!(channel),
                    json!(req.config.to_string()),
                    json!(req.enabled.unwrap_or(true) as i64),
                ],
            )
            .await
            .map_err(|e| format!("upsert channel: {e}"))?;
        Ok(NotifyChannel {
            id,
            tenant_id: tenant_id.to_string(),
            channel: channel.to_string(),
            config: req.config.clone(),
            enabled: req.enabled.unwrap_or(true),
            created_at: String::new(),
            updated_at: String::new(),
        })
    }

    pub async fn delete_channel(&self, tenant_id: &str, channel: &str) -> Result<bool, String> {
        let n = self
            .db
            .execute_with(
                "DELETE FROM notify_channels WHERE tenant_id = ? AND channel = ?",
                &[json!(tenant_id), json!(channel)],
            )
            .await
            .map_err(|e| format!("delete channel: {e}"))?;
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
        action_device_id: r.get("action_device_id").and_then(Value::as_str).map(str::to_string),
        action_code: r.get("action_code").and_then(Value::as_str).map(str::to_string),
        action_value: r.get("action_value").and_then(Value::as_object).map(|o| Value::Object(o.clone())),
        enabled: r.get("enabled").and_then(Value::as_i64).unwrap_or(0) != 0,
        created_at: r.get("created_at").and_then(Value::as_str).unwrap_or_default().to_string(),
        updated_at: r.get("updated_at").and_then(Value::as_str).unwrap_or_default().to_string(),
    }
}

fn channel_from_row(r: &Row) -> NotifyChannel {
    NotifyChannel {
        id: r.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
        tenant_id: r.get("tenant_id").and_then(Value::as_str).unwrap_or_default().to_string(),
        channel: r.get("channel").and_then(Value::as_str).unwrap_or_default().to_string(),
        config: r
            .get("config")
            .and_then(Value::as_str)
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(Value::Null),
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
