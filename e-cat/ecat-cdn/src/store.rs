use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use ecat_security::crypto::{decrypt, derive_key, encrypt};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::models::{CdnTask, Provider, UpdateProvider};

#[derive(Clone)]
pub struct CdnStore {
    pub db: Arc<SqlxClient>,
    pub key: [u8; 32],
}

impl CdnStore {
    pub fn new(db: Arc<SqlxClient>, enc_key_env: &str) -> Self {
        Self { db, key: derive_key(enc_key_env) }
    }

    /// 校验 vendor 已知且 config 为对象后创建供应商。
    pub async fn create(
        &self,
        tenant_id: &str,
        name: &str,
        vendor: &str,
        domain: &str,
        config: &Value,
    ) -> Result<String, String> {
        if !config.is_object() {
            return Err("config must be a JSON object".into());
        }
        let enc = encrypt(&self.key, &serde_json::to_vec(config).unwrap_or_default())
            .map_err(|e| e.to_string())?;
        // cdn_providers.id 为 BIGINT 列：绑定数字，返回十进制字符串
        let id = ecat::ids::next_id();
        self.db
            .execute_with(
                "INSERT INTO cdn_providers (id, tenant_id, name, vendor, domain, config_encrypted) \
                 VALUES (?, ?, ?, ?, ?, ?)",
                &[json!(id), json!(tenant_id), json!(name), json!(vendor), json!(domain), json!(enc)],
            )
            .await
            .map_err(|e| format!("insert provider: {e}"))?;
        Ok(id.to_string())
    }

    pub async fn list(&self, tenant_id: &str) -> Result<Vec<Provider>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT id, tenant_id, name, vendor, domain, config_encrypted, status, created_at \
                 FROM cdn_providers WHERE tenant_id = ?",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("list providers: {e}"))?;
        rows.iter().map(|r| self.row_to_provider(r)).collect()
    }

    pub async fn get(&self, tenant_id: &str, id: &str) -> Result<Provider, String> {
        let id_n: i64 = id.parse().map_err(|_| "invalid provider id".to_string())?;
        let rows = self
            .db
            .query_with(
                "SELECT id, tenant_id, name, vendor, domain, config_encrypted, status, created_at \
                 FROM cdn_providers WHERE tenant_id = ? AND id = ?",
                &[json!(tenant_id), json!(id_n)],
            )
            .await
            .map_err(|e| format!("get provider: {e}"))?;
        match rows.first() {
            Some(r) => self.row_to_provider(r),
            None => Err("provider not found".into()),
        }
    }

    fn row_to_provider(&self, r: &ecat_data::Row) -> Result<Provider, String> {
        let id = r.get("id").and_then(Value::as_i64).map(|n| n.to_string()).unwrap_or_default();
        let tenant_id = r.get("tenant_id").and_then(Value::as_str).unwrap_or_default().to_string();
        let name = r.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        let vendor = r.get("vendor").and_then(Value::as_str).unwrap_or_default().to_string();
        let domain = r.get("domain").and_then(Value::as_str).unwrap_or_default().to_string();
        let status = r.get("status").and_then(Value::as_str).unwrap_or_default().to_string();
        let created_at = r.get("created_at").map(Value::to_string).unwrap_or_default();
        let enc = r.get("config_encrypted").and_then(Value::as_str).ok_or_else(|| "missing config".to_string())?;
        let plain = decrypt(&self.key, enc)?;
        let config: Value = serde_json::from_slice(&plain).map_err(|e| format!("config json: {e}"))?;
        Ok(Provider { id, tenant_id, name, vendor, domain, config, status, created_at })
    }

    pub async fn update(
        &self,
        tenant_id: &str,
        id: &str,
        upd: &UpdateProvider,
    ) -> Result<Provider, String> {
        if let Some(c) = &upd.config {
            if !c.is_object() {
                return Err("config must be a JSON object".into());
            }
        }
        let cur = self.get(tenant_id, id).await?;
        let name = upd.name.as_ref().map_or(cur.name, String::clone);
        let domain = upd.domain.as_ref().map_or(cur.domain, String::clone);
        let config = upd.config.as_ref().map_or(cur.config, Value::clone);
        let enc = encrypt(&self.key, &serde_json::to_vec(&config).unwrap_or_default())
            .map_err(|e| e.to_string())?;
        let id_n: i64 = id.parse().map_err(|_| "invalid provider id".to_string())?;
        self.db
            .execute_with(
                "UPDATE cdn_providers SET name = ?, domain = ?, config_encrypted = ? \
                 WHERE tenant_id = ? AND id = ?",
                &[json!(name), json!(domain), json!(enc), json!(tenant_id), json!(id_n)],
            )
            .await
            .map_err(|e| format!("update provider: {e}"))?;
        self.get(tenant_id, id).await
    }

    pub async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), String> {
        let id_n: i64 = id.parse().map_err(|_| "invalid provider id".to_string())?;
        let n = self
            .db
            .execute_with(
                "DELETE FROM cdn_providers WHERE tenant_id = ? AND id = ?",
                &[json!(tenant_id), json!(id_n)],
            )
            .await
            .map_err(|e| format!("delete provider: {e}"))?;
        if n == 0 {
            return Err("provider not found".into());
        }
        Ok(())
    }

    pub async fn set_status(&self, tenant_id: &str, id: &str, status: &str) -> Result<Provider, String> {
        let id_n: i64 = id.parse().map_err(|_| "invalid provider id".to_string())?;
        let n = self
            .db
            .execute_with(
                "UPDATE cdn_providers SET status = ? WHERE tenant_id = ? AND id = ?",
                &[json!(status), json!(tenant_id), json!(id_n)],
            )
            .await
            .map_err(|e| format!("set status: {e}"))?;
        if n == 0 {
            return Err("provider not found".into());
        }
        self.get(tenant_id, id).await
    }

    /// 记录刷新/预热任务（同步执行后落库，保留审计痕迹）。
    pub async fn record_task(
        &self,
        tenant_id: &str,
        provider_id: &str,
        kind: &str,
        urls: &[String],
        status: &str,
        error: &str,
    ) -> Result<(), String> {
        // cdn_tasks.id / provider_id 为 BIGINT 列：绑定数字
        let id = ecat::ids::next_id();
        let provider_n: i64 = provider_id.parse().map_err(|_| "invalid provider id".to_string())?;
        let urls_json = serde_json::to_string(urls).unwrap_or_default();
        self.db
            .execute_with(
                "INSERT INTO cdn_tasks (id, tenant_id, provider_id, kind, urls_json, status, error) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                &[json!(id), json!(tenant_id), json!(provider_n), json!(kind), json!(urls_json), json!(status), json!(error)],
            )
            .await
            .map_err(|e| format!("record task: {e}"))?;
        Ok(())
    }

    /// 供应商统计：总数 + 已启用数。
    pub async fn stats(&self, tenant_id: &str) -> Result<(i64, i64), String> {
        let rows = self
            .db
            .query_with(
                "SELECT status, COUNT(*) AS n FROM cdn_providers WHERE tenant_id = ? GROUP BY status",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("provider stats: {e}"))?;
        let mut total = 0;
        let mut enabled = 0;
        for r in &rows {
            let n = r.get("n").and_then(Value::as_i64).unwrap_or(0);
            total += n;
            if r.get("status").and_then(Value::as_str) == Some("enabled") {
                enabled += n;
            }
        }
        Ok((total, enabled))
    }

    pub async fn list_tasks(&self, tenant_id: &str) -> Result<Vec<CdnTask>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT id, tenant_id, provider_id, kind, urls_json, status, error, created_at \
                 FROM cdn_tasks WHERE tenant_id = ? ORDER BY created_at DESC LIMIT 100",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("list tasks: {e}"))?;
        Ok(rows
            .iter()
            .map(|r| CdnTask {
                id: r.get("id").and_then(Value::as_i64).map(|n| n.to_string()).unwrap_or_default(),
                tenant_id: r.get("tenant_id").and_then(Value::as_str).unwrap_or_default().to_string(),
                provider_id: r.get("provider_id").and_then(Value::as_i64).map(|n| n.to_string()).unwrap_or_default(),
                kind: r.get("kind").and_then(Value::as_str).unwrap_or_default().to_string(),
                urls: r
                    .get("urls_json")
                    .and_then(Value::as_str)
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default(),
                status: r.get("status").and_then(Value::as_str).unwrap_or_default().to_string(),
                error: r.get("error").and_then(Value::as_str).unwrap_or_default().to_string(),
                created_at: r.get("created_at").map(Value::to_string).unwrap_or_default(),
            })
            .collect())
    }
}
