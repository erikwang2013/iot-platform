use ecat_data::RdbmsClient;
use ecat_security::crypto::{decrypt, derive_key, encrypt};
use ecat_data_sqlx::SqlxClient;
use serde_json::{Value, json};
use std::sync::Arc;

/// 凭据密文 JSON 序列化：字段顺序固定，VendorCreds 的存盘形态。
pub fn creds_json(cfg: &Value) -> Vec<u8> {
    serde_json::to_vec(cfg).unwrap_or_default()
}

#[derive(Clone)]
pub struct Store {
    pub db: Arc<SqlxClient>,
    pub key: [u8; 32],
}

impl Store {
    pub fn new(db: Arc<SqlxClient>, enc_key_env: &str) -> Self {
        Self { db, key: derive_key(enc_key_env) }
    }

    /// 保存（或更新）租户在某厂商的凭据；失败返回 Err(String)。
    pub async fn save_creds(
        &self,
        tenant_id: &str,
        vendor: &str,
        cfg: &Value,
    ) -> Result<(), String> {
        let enc = encrypt(&self.key, &creds_json(cfg)).map_err(|e| e.to_string())?;
        let id = uuid::Uuid::new_v4().to_string();
        let sql = "INSERT INTO vendor_credentials (id, tenant_id, vendor, config_encrypted, status) \
                   VALUES (?, ?, ?, ?, 'active') \
                   ON DUPLICATE KEY UPDATE config_encrypted = VALUES(config_encrypted)";
        self.db
            .execute_with(sql, &[json!(id), json!(tenant_id), json!(vendor), json!(enc)])
            .await
            .map_err(|e| format!("save creds: {e}"))?;
        Ok(())
    }

    /// 读取并解密凭据；无记录返回 Err("no credentials")。
    pub async fn load_creds(&self, tenant_id: &str, vendor: &str) -> Result<Value, String> {
        let rows = self
            .db
            .query_with(
                "SELECT config_encrypted FROM vendor_credentials WHERE tenant_id = ? AND vendor = ?",
                &[json!(tenant_id), json!(vendor)],
            )
            .await
            .map_err(|e| format!("load creds: {e}"))?;
        let enc = rows
            .first()
            .and_then(|r| r.get("config_encrypted"))
            .and_then(Value::as_str)
            .ok_or_else(|| "no credentials".to_string())?;
        let plain = decrypt(&self.key, enc)?;
        serde_json::from_slice(&plain).map_err(|e| format!("creds json: {e}"))
    }

    /// 按厂商设备 ID 找平台设备（device_links 查询）。
    pub async fn find_device_by_vendor_id(
        &self,
        vendor: &str,
        vendor_id: &str,
    ) -> Result<Option<String>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT device_id FROM device_links WHERE vendor = ? AND vendor_id = ?",
                &[json!(vendor), json!(vendor_id)],
            )
            .await
            .map_err(|e| format!("find device: {e}"))?;
        Ok(rows
            .first()
            .and_then(|r| r.get("device_id"))
            .and_then(Value::as_str)
            .map(str::to_string))
    }

    /// 查设备所属租户（webhook/MQTT 事件归属用）。
    pub async fn tenant_of_device(&self, device_id: &str) -> Result<String, String> {
        let rows = self
            .db
            .query_with(
                "SELECT tenant_id FROM devices WHERE id = ?",
                &[json!(device_id)],
            )
            .await
            .map_err(|e| format!("tenant of device: {e}"))?;
        rows.first()
            .and_then(|r| r.get("tenant_id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "device not found".to_string())
    }

    /// 拉取直连设备（vendor='direct'）列表，mqtt.rs 订阅用。
    pub async fn list_direct_devices(&self) -> Result<Vec<(String, String)>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT id, tenant_id FROM devices WHERE vendor = 'direct'",
                &[],
            )
            .await
            .map_err(|e| format!("list direct devices: {e}"))?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let id = r.get("id").and_then(Value::as_str)?;
                let t = r.get("tenant_id").and_then(Value::as_str)?;
                Some((id.to_string(), t.to_string()))
            })
            .collect())
    }

    pub async fn device_name(&self, device_id: &str) -> Result<String, String> {
        let rows = self
            .db
            .query_with("SELECT name FROM devices WHERE id = ?", &[json!(device_id)])
            .await
            .map_err(|e| format!("device name: {e}"))?;
        rows.first()
            .and_then(|r| r.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "device not found".to_string())
    }

    /// 查设备链接信息：(vendor, vendor_id)；未链接返回 None。
    pub async fn find_link(&self, device_id: &str) -> Result<Option<(String, String)>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT vendor, vendor_id FROM device_links WHERE device_id = ?",
                &[json!(device_id)],
            )
            .await
            .map_err(|e| format!("find link: {e}"))?;
        Ok(rows.first().and_then(|r| {
            Some((
                r.get("vendor")?.as_str()?.to_string(),
                r.get("vendor_id")?.as_str()?.to_string(),
            ))
        }))
    }

    /// 导入设备（Task 5 用）：platform_id 已存在则复用，否则新建。
    pub async fn upsert_device(
        &self,
        tenant_id: &str,
        vendor: &str,
        vendor_id: &str,
        name: &str,
        category: &str,
        online: bool,
    ) -> Result<String, String> {
        if let Some(existing) = self.find_device_by_vendor_id(vendor, vendor_id).await? {
            return Ok(existing);
        }
        let platform_id = uuid::Uuid::new_v4().to_string();
        let status = if online { "online" } else { "offline" };
        self.db
            .execute_with(
                "INSERT INTO devices (id, tenant_id, name, vendor, status) VALUES (?, ?, ?, ?, ?)",
                &[
                    json!(platform_id),
                    json!(tenant_id),
                    json!(name),
                    json!(vendor),
                    json!(status),
                ],
            )
            .await
            .map_err(|e| format!("insert device: {e}"))?;
        self.db
            .execute_with(
                "INSERT INTO device_links (device_id, tenant_id, vendor, vendor_id, vendor_name, category) \
                 VALUES (?, ?, ?, ?, ?, ?)",
                &[
                    json!(platform_id),
                    json!(tenant_id),
                    json!(vendor),
                    json!(vendor_id),
                    json!(name),
                    json!(category),
                ],
            )
            .await
            .map_err(|e| format!("insert link: {e}"))?;
        Ok(platform_id)
    }
}
