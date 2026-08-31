use ecat_data::RdbmsClient;
use ecat_security::crypto::{decrypt, derive_key, encrypt};
use ecat_data_sqlx::SqlxClient;
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: String,
    pub tenant_id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TenantRow {
    pub id: String,
    pub name: String,
    pub quota: i64,
    /// 当前设备数（用量；配额强制 C-5）
    pub device_count: i64,
}

/// 凭据密文 JSON 序列化：字段顺序固定，VendorCreds 的存盘形态。
pub fn creds_json(cfg: &Value) -> Vec<u8> {
    serde_json::to_vec(cfg).unwrap_or_default()
}

/// 开放 API 密钥行（api_keys 表）。app_secret 仅在创建时返回一次，
/// 库中只存 SHA-256 哈希。
#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyRow {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub created_at: String,
    pub revoked: bool,
}

/// 审计日志行（audit_log 表，由网关写操作审计中间件写入）。
#[derive(Debug, Clone, Serialize)]
pub struct AuditRow {
    pub id: i64,
    pub tenant_id: String,
    pub role: String,
    pub method: String,
    pub path: String,
    pub status: i64,
    pub created_at: String,
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

    /// 将一条指令写入离线指令队列（D-2）。expires_at 过期后不再补发。
    pub async fn enqueue_command(
        &self,
        tenant_id: &str,
        device_id: &str,
        code: &str,
        value: &serde_json::Value,
        expires_after_secs: i64,
    ) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.db
            .execute_with(
                "INSERT INTO command_queue (id, tenant_id, device_id, code, value_json, expires_at) \
                 VALUES (?, ?, ?, ?, ?, DATE_ADD(NOW(), INTERVAL ? SECOND))",
                &[
                    json!(id),
                    json!(tenant_id),
                    json!(device_id),
                    json!(code),
                    value.clone(),
                    json!(expires_after_secs),
                ],
            )
            .await
            .map_err(|e| format!("enqueue command: {e}"))?;
        Ok(id)
    }

    /// 取设备未过期待补发指令（按入队时间升序）。同时删除已过期的占位（不返回）。
    pub async fn pending_commands(
        &self,
        device_id: &str,
    ) -> Result<Vec<(String, String, serde_json::Value)>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT id, code, value_json FROM command_queue \
                 WHERE device_id = ? AND (expires_at IS NULL OR expires_at > NOW()) \
                 ORDER BY created_at ASC",
                &[json!(device_id)],
            )
            .await
            .map_err(|e| format!("pending commands: {e}"))?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let id = r.get("id")?.as_str()?.to_string();
                let code = r.get("code")?.as_str()?.to_string();
                let value = r
                    .get("value_json")
                    .and_then(|v| serde_json::from_str(v.as_str()?).ok())?;
                Some((id, code, value))
            })
            .collect())
    }

    /// 补发完成后删除已下发的指令。
    pub async fn delete_command(&self, id: &str) -> Result<(), String> {
        self.db
            .execute_with("DELETE FROM command_queue WHERE id = ?", &[json!(id)])
            .await
            .map_err(|e| format!("delete command: {e}"))?;
        Ok(())
    }

    /// 取某厂商当前已入库设备列表（B-2 熔断降级的缓存源）。
    pub async fn list_vendor_devices(
        &self,
        tenant_id: &str,
        vendor: &str,
    ) -> Result<Vec<crate::models::DeviceRecord>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT l.vendor_id, l.vendor_name AS name, l.category, d.status \
                 FROM device_links l JOIN devices d ON d.id = l.device_id \
                 WHERE l.tenant_id = ? AND l.vendor = ?",
                &[json!(tenant_id), json!(vendor)],
            )
            .await
            .map_err(|e| format!("list vendor devices: {e}"))?;
        Ok(rows
            .iter()
            .map(|r| crate::models::DeviceRecord {
                id: String::new(),
                vendor_id: r.get("vendor_id").and_then(Value::as_str).unwrap_or("").to_string(),
                name: r.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                category: r.get("category").and_then(Value::as_str).unwrap_or("").to_string(),
                online: r.get("status").and_then(Value::as_str).unwrap_or("") == "online",
                properties: Vec::new(),
            })
            .collect())
    }

    /// 将设备落库状态标记为 offline（离线巡检 B-1 用）。幂等。
    pub async fn set_device_offline(&self, device_id: &str) -> Result<(), String> {
        self.db
            .execute_with(
                "UPDATE devices SET status = 'offline' WHERE id = ?",
                &[json!(device_id)],
            )
            .await
            .map_err(|e| format!("set device offline: {e}"))?;
        Ok(())
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

    pub async fn ensure_tenant(&self, id: &str, name: &str) -> Result<(), String> {
        self.db
            .execute_with(
                "INSERT IGNORE INTO tenants (id, name) VALUES (?, ?)",
                &[json!(id), json!(name)],
            )
            .await
            .map_err(|e| format!("ensure tenant: {e}"))?;
        Ok(())
    }

    pub async fn find_user(&self, username: &str) -> Result<Option<UserRow>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT id, tenant_id, username, password_hash, role FROM users WHERE username = ?",
                &[json!(username)],
            )
            .await
            .map_err(|e| format!("find user: {e}"))?;
        Ok(rows.first().map(|r| UserRow {
            id: r.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
            tenant_id: r.get("tenant_id").and_then(Value::as_str).unwrap_or("").to_string(),
            username: r.get("username").and_then(Value::as_str).unwrap_or("").to_string(),
            password_hash: r.get("password_hash").and_then(Value::as_str).unwrap_or("").to_string(),
            role: r.get("role").and_then(Value::as_str).unwrap_or("").to_string(),
        }))
    }

    pub async fn create_user(
        &self,
        tenant_id: &str,
        username: &str,
        password_hash: &str,
        role: &str,
    ) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.db
            .execute_with(
                "INSERT INTO users (id, tenant_id, username, password_hash, role) VALUES (?, ?, ?, ?, ?)",
                &[json!(id), json!(tenant_id), json!(username), json!(password_hash), json!(role)],
            )
            .await
            .map_err(|e| format!("create user: {e}"))?;
        Ok(id)
    }

    pub async fn list_users(&self, tenant_id: &str) -> Result<Vec<UserRow>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT id, tenant_id, username, password_hash, role FROM users WHERE tenant_id = ?",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("list users: {e}"))?;
        Ok(rows
            .iter()
            .map(|r| UserRow {
                id: r.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
                tenant_id: r.get("tenant_id").and_then(Value::as_str).unwrap_or("").to_string(),
                username: r.get("username").and_then(Value::as_str).unwrap_or("").to_string(),
                password_hash: r.get("password_hash").and_then(Value::as_str).unwrap_or("").to_string(),
                role: r.get("role").and_then(Value::as_str).unwrap_or("").to_string(),
            })
            .collect())
    }

    pub async fn delete_user(&self, tenant_id: &str, user_id: &str) -> Result<bool, String> {
        let n = self
            .db
            .execute_with(
                "DELETE FROM users WHERE id = ? AND tenant_id = ?",
                &[json!(user_id), json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("delete user: {e}"))?;
        Ok(n > 0)
    }

    pub async fn list_tenants(&self) -> Result<Vec<TenantRow>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT t.id, t.name, t.quota, \
                 (SELECT COUNT(*) FROM devices d WHERE d.tenant_id = t.id) AS device_count \
                 FROM tenants t ORDER BY t.created_at",
                &[],
            )
            .await
            .map_err(|e| format!("list tenants: {e}"))?;
        Ok(rows
            .iter()
            .map(|r| TenantRow {
                id: r.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
                name: r.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                quota: r.get("quota").and_then(Value::as_i64).unwrap_or(0),
                device_count: r
                    .get("device_count")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
            })
            .collect())
    }

    pub async fn create_tenant(&self, name: &str, quota: i64) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.db
            .execute_with(
                "INSERT INTO tenants (id, name, quota) VALUES (?, ?, ?)",
                &[json!(id), json!(name), json!(quota)],
            )
            .await
            .map_err(|e| format!("create tenant: {e}"))?;
        Ok(id)
    }

    /// 级联清理：用户/设备/凭据等 FK 引用先删，再删租户本体。
    pub async fn delete_tenant(&self, tenant_id: &str) -> Result<bool, String> {
        for table in ["users", "device_links", "devices", "vendor_credentials", "thing_models"] {
            self.db
                .execute_with(
                    &format!("DELETE FROM {table} WHERE tenant_id = ?"),
                    &[json!(tenant_id)],
                )
                .await
                .map_err(|e| format!("delete tenant {table}: {e}"))?;
        }
        let n = self
            .db
            .execute_with("DELETE FROM tenants WHERE id = ?", &[json!(tenant_id)])
            .await
            .map_err(|e| format!("delete tenant: {e}"))?;
        Ok(n > 0)
    }

    /// schema_json 是 JSON 列，经 sqlx Any 层按文本（Blob→UTF-8）返回，需再解析。
    fn parse_schema(cell: Option<Value>) -> Option<Value> {
        let s = cell?.as_str()?.to_owned();
        serde_json::from_str(&s).ok()
    }

    pub async fn list_models(&self, tenant_id: &str) -> Result<Vec<(String, Value)>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT id, schema_json FROM thing_models WHERE tenant_id = ? ORDER BY created_at",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("list models: {e}"))?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let id = r.get("id").and_then(Value::as_str)?.to_string();
                Some((id, Self::parse_schema(r.get("schema_json").cloned())?))
            })
            .collect())
    }

    /// 设备物模型：全局（device_id 为空）+ 该设备私有，按创建序合并。
    pub async fn device_models(
        &self,
        tenant_id: &str,
        device_id: &str,
    ) -> Result<Vec<Value>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT schema_json FROM thing_models \
                 WHERE tenant_id = ? AND (device_id IS NULL OR device_id = ?) ORDER BY created_at",
                &[json!(tenant_id), json!(device_id)],
            )
            .await
            .map_err(|e| format!("device models: {e}"))?;
        Ok(rows
            .iter()
            .filter_map(|r| Self::parse_schema(r.get("schema_json").cloned()))
            .collect())
    }

    pub async fn create_model(
        &self,
        tenant_id: &str,
        device_id: Option<&str>,
        schema: &Value,
    ) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let schema_str = serde_json::to_string(schema).map_err(|e| e.to_string())?;
        self.db
            .execute_with(
                "INSERT INTO thing_models (id, tenant_id, device_id, schema_json) VALUES (?, ?, ?, ?)",
                &[json!(id), json!(tenant_id), json!(device_id), json!(schema_str)],
            )
            .await
            .map_err(|e| format!("create model: {e}"))?;
        Ok(id)
    }

    pub async fn delete_model(&self, tenant_id: &str, model_id: &str) -> Result<bool, String> {
        let n = self
            .db
            .execute_with(
                "DELETE FROM thing_models WHERE id = ? AND tenant_id = ?",
                &[json!(model_id), json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("delete model: {e}"))?;
        Ok(n > 0)
    }

    /// 租户设备配额与当前用量：返回 (quota, 已用)。quota=0 视为不限制。
    pub async fn tenant_quota(&self, tenant_id: &str) -> Result<(i64, i64), String> {
        let rows = self
            .db
            .query_with(
                "SELECT quota FROM tenants WHERE id = ?",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("tenant quota: {e}"))?;
        let quota = rows
            .first()
            .and_then(|r| r.get("quota"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let rows = self
            .db
            .query_with(
                "SELECT COUNT(*) AS n FROM devices WHERE tenant_id = ?",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("count devices: {e}"))?;
        let used = rows
            .first()
            .and_then(|r| r.get("n"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        Ok((quota, used))
    }

    /// 校验配额：quota>0 且已用>=quota 时返回 Err（409 语义，调用方映射状态码）。
    /// 新增 n 台前调用；quota=0 表示不限制。
    pub async fn check_quota(&self, tenant_id: &str, adding: i64) -> Result<(), String> {
        let (quota, used) = self.tenant_quota(tenant_id).await?;
        if quota > 0 && used + adding > quota {
            return Err(format!(
                "device quota exceeded: {used}/{quota} (need {adding} more)"
            ));
        }
        Ok(())
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
        // 配额强制：新设备入库前校验（C-5）
        self.check_quota(tenant_id, 1).await?;
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

    /// 分页查询审计日志（网关写操作审计写入）。created_at 是 TIMESTAMP，
    /// sqlx Any 不支持时间类型，按 CHAR 取回。
    pub async fn list_audit(
        &self,
        tenant_id: &str,
        page: u32,
        size: u32,
    ) -> Result<Vec<AuditRow>, String> {
        let limit = size.min(200) as i64;
        let offset = (page.saturating_sub(1) as i64) * limit;
        let rows = self
            .db
            .query_with(
                "SELECT id, tenant_id, role, method, path, status, \
                 CAST(created_at AS CHAR) AS created_at \
                 FROM audit_log WHERE tenant_id = ? \
                 ORDER BY id DESC LIMIT ? OFFSET ?",
                &[json!(tenant_id), json!(limit), json!(offset)],
            )
            .await
            .map_err(|e| format!("list audit: {e}"))?;
        Ok(rows
            .iter()
            .map(|r| AuditRow {
                id: r.get("id").and_then(Value::as_i64).unwrap_or(0),
                tenant_id: r.get("tenant_id").and_then(Value::as_str).unwrap_or("").to_string(),
                role: r.get("role").and_then(Value::as_str).unwrap_or("").to_string(),
                method: r.get("method").and_then(Value::as_str).unwrap_or("").to_string(),
                path: r.get("path").and_then(Value::as_str).unwrap_or("").to_string(),
                status: r.get("status").and_then(Value::as_i64).unwrap_or(0),
                created_at: r.get("created_at").and_then(Value::as_str).unwrap_or("").to_string(),
            })
            .collect())
    }

    // ---- 开放 API 密钥（api_keys 表）----

    /// 创建开放 API 密钥：app_secret 明文仅此一次返回，库中只存
    /// HMAC-SHA256 哈希（app_id 即主键，作为客户端的 API 标识）。
    pub async fn create_api_key(
        &self,
        tenant_id: &str,
        name: &str,
    ) -> Result<(String, String), String> {
        let id = uuid::Uuid::new_v4().to_string();
        let secret = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let hash = ecat_security::crypto::hmac_sha256_hex(&secret, id.as_bytes());
        self.db
            .execute_with(
                "INSERT INTO api_keys (id, tenant_id, name, secret_hash) VALUES (?, ?, ?, ?)",
                &[json!(id), json!(tenant_id), json!(name), json!(hash)],
            )
            .await
            .map_err(|e| format!("create api key: {e}"))?;
        Ok((id, secret))
    }

    pub async fn list_api_keys(&self, tenant_id: &str) -> Result<Vec<ApiKeyRow>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT id, tenant_id, name, CAST(created_at AS CHAR) AS created_at, \
                 CAST(revoked_at AS CHAR) AS revoked_at \
                 FROM api_keys WHERE tenant_id = ? ORDER BY created_at",
                &[json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("list api keys: {e}"))?;
        Ok(rows
            .iter()
            .map(|r| ApiKeyRow {
                id: r.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
                tenant_id: r.get("tenant_id").and_then(Value::as_str).unwrap_or("").to_string(),
                name: r.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                created_at: r.get("created_at").and_then(Value::as_str).unwrap_or("").to_string(),
                revoked: r.get("revoked_at").and_then(Value::as_str).is_some(),
            })
            .collect())
    }

    /// 吊销密钥（幂等：已吊销返回 false）。仅本租户可吊销。
    pub async fn revoke_api_key(&self, tenant_id: &str, id: &str) -> Result<bool, String> {
        let n = self
            .db
            .execute_with(
                "UPDATE api_keys SET revoked_at = NOW() \
                 WHERE id = ? AND tenant_id = ? AND revoked_at IS NULL",
                &[json!(id), json!(tenant_id)],
            )
            .await
            .map_err(|e| format!("revoke api key: {e}"))?;
        Ok(n > 0)
    }

    /// 校验开放 API 密钥：存在且未吊销且摘要匹配 → 返回租户 ID；否则 None。
    pub async fn verify_api_key(
        &self,
        app_id: &str,
        secret: &str,
    ) -> Result<Option<String>, String> {
        let rows = self
            .db
            .query_with(
                "SELECT tenant_id, secret_hash, CAST(revoked_at AS CHAR) AS revoked_at \
                 FROM api_keys WHERE id = ?",
                &[json!(app_id)],
            )
            .await
            .map_err(|e| format!("verify api key: {e}"))?;
        let row = rows.first().ok_or_else(|| "api key not found".to_string())?;
        if row.get("revoked_at").and_then(Value::as_str).is_some() {
            return Ok(None);
        }
        let hash = row
            .get("secret_hash")
            .and_then(Value::as_str)
            .ok_or_else(|| "api key row malformed".to_string())?;
        let tenant_id = row
            .get("tenant_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "api key row malformed".to_string())?;
        if !ecat_security::crypto::verify_hmac_sha256_hex(secret, app_id.as_bytes(), hash) {
            return Ok(None);
        }
        Ok(Some(tenant_id.to_string()))
    }
}
