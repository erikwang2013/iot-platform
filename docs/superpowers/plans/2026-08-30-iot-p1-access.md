# IoT 平台 P1 接入实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 接入层跑通——iot-access 服务（端口 8082）承载厂商接入：涂鸦 OAuth 授权 + 设备拉取入库 + Webhook 事件入 Kafka；直连 MQTT 设备上报入 Redis 影子 + Kafka；指令下发链路可用；网关转发 `/api/access/*` 并透传租户；冒烟全绿。

**Architecture:** iot-access 用 ecat::App + HttpServer（axum 0.8），依赖 ecat-data-sqlx（MySQL 凭据表）、ecat-data-redis（设备影子）、ecat-mq-kafka（事件发布，topic `iot.events`）、ecat-mq-mqtt（EMQX 直连订阅）。厂商适配器统一 `VendorAdapter` Trait（`list_devices / get_properties / send_command / subscribe_events`），首个实现为涂鸦（OAuth 授权码 → access_token 刷新 → OpenAPI → 统一物模型）。所有对外调涂鸦 OpenAPI 的 HTTP 用 reqwest（ecat-client 面向内部服务发现，不适合外部厂商 API）。网关 `/api/access/*` 反代到 iot-access：公开路径（OAuth callback、涂鸦 Webhook）豁免 JWT；受保护路径经 JwtAuthCompat 后从 request extensions 读取 `AuthClaims.sub` 作为租户 ID 注入 `x-tenant-id` header。

**Tech Stack:** Rust 2024 edition、e-cat v3.0.3（本地 `e-cat/` path 依赖，workspace 根为 `e-cat/Cargo.toml`）、axum 0.8、sqlx Any 驱动（mysql feature 内置）、reqwest 0.12、aes-gcm 0.10（AES-256-GCM）、hmac 0.12 + sha2 0.10（涂鸦签名）、MySQL 8、Redis 7、EMQX 5.8、Kafka 3.7（bitnami）。

**约定:** 所有 cargo 命令在 `/home/wwwroot/iot-platform/e-cat` 下运行（workspace 根）。新增服务须加入 workspace members。租户 ID 一律来自网关注入的 `x-tenant-id`（取 JWT sub）；iot-access 内部端点缺该 header 返回 401。统一物模型 `DeviceRecord`、事件 `EventMessage`、影子 JSON 结构在 Webhook/MQTT/Kafka/Redis 各处完全一致（见 Task 2）。AES 密钥从环境变量 `IOT_CRED_ENCRYPT_KEY` 读取（≥16 字符，缺省 dev 值仅限开发）。

---

### Task 1: iot-access 服务骨架（workspace 成员 + /health，端口 8082）

**Files:**
- Modify: `e-cat/Cargo.toml`（workspace members 加 `"services/iot-access"`）
- Create: `e-cat/services/iot-access/Cargo.toml`
- Create: `e-cat/services/iot-access/src/main.rs`
- Create: `e-cat/services/iot-access/src/lib.rs`

^- [x] **Step 1: 注册 workspace 成员**

`e-cat/Cargo.toml` 的 members 数组 `"services/iot-device"` 行后加一行：

```toml
    "services/iot-access",
```

^- [x] **Step 2: 写 Cargo.toml**

`e-cat/services/iot-access/Cargo.toml`:

```toml
[package]
name = "iot-access"
version = "0.1.0"
edition = "2024"

[dependencies]
ecat = { path = "../../ecat" }
ecat-transport-http.workspace = true
ecat-data-sqlx = { path = "../../ecat-data-sqlx" }
ecat-data.workspace = true
ecat-data-redis = { path = "../../ecat-data-redis" }
ecat-mq.workspace = true
ecat-mq-kafka = { path = "../../ecat-mq-kafka" }
ecat-mq-mqtt = { path = "../../ecat-mq-mqtt" }
axum.workspace = true
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio = { workspace = true, features = ["full"] }
tracing.workspace = true
reqwest = { version = "0.12", features = ["json"] }
aes-gcm = "0.10"
rand = "0.8"
base64 = "0.22"
sha2.workspace = true
hmac = "0.12"
hex = "0.4"
uuid = { version = "1", features = ["v4"] }
futures-util = "0.3"

[dev-dependencies]
tower-util = "0.3"
```

^- [x] **Step 3: 写最小 main.rs（仅 /health）**

`e-cat/services/iot-access/src/main.rs`:

```rust
use axum::{Router, routing::get};

async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let router = Router::new().route("/health", get(health));
    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8082").router(router);
    let mut app = ecat::App::builder()
        .name("iot-access")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
```

^- [x] **Step 4: 写 lib.rs（后续 Task 逐模块加入）**

`e-cat/services/iot-access/src/lib.rs`:

```rust
pub mod models;
```

注：`models.rs` 由 Task 2 创建；本步若编译报缺文件，先建空 `models.rs`（`// 见 Task 2`）占位，Task 2 覆盖。

^- [x] **Step 5: 构建验证**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo check -p iot-access`
Expected: 编译通过。

^- [x] **Step 6: Commit**

```bash
git add e-cat/Cargo.toml e-cat/services/iot-access/
git commit -m "feat(access): iot-access skeleton (port 8082, /health)"
```

---

### Task 2: 统一模型与厂商适配器 Trait

**Files:**
- Create: `e-cat/services/iot-access/src/models.rs`
- Create: `e-cat/services/iot-access/src/adapter.rs`
- Modify: `e-cat/services/iot-access/src/lib.rs`
- Create: `e-cat/services/iot-access/tests/adapter_trait.rs`

- [x] **Step 1: 写失败测试**

`e-cat/services/iot-access/tests/adapter_trait.rs`:

```rust
use async_trait::async_trait;
use iot_access::adapter::{AdapterError, VendorAdapter, VendorCreds};
use iot_access::models::{DeviceRecord, EventMessage, PropertyValue};
use serde_json::json;

struct Dummy;

#[async_trait]
impl VendorAdapter for Dummy {
    async fn list_devices(&self, _c: &VendorCreds) -> Result<Vec<DeviceRecord>, AdapterError> {
        Ok(vec![DeviceRecord {
            id: "dev-1".into(),
            vendor_id: "tuya-dev-1".into(),
            name: "sensor".into(),
            category: "temp".into(),
            online: true,
            properties: vec![PropertyValue { code: "temp".into(), value: json!(23.5) }],
        }])
    }
    async fn get_properties(
        &self,
        _c: &VendorCreds,
        _vendor_id: &str,
    ) -> Result<Vec<PropertyValue>, AdapterError> {
        Ok(vec![])
    }
    async fn send_command(
        &self,
        _c: &VendorCreds,
        _vendor_id: &str,
        _code: &str,
        _value: serde_json::Value,
    ) -> Result<(), AdapterError> {
        Ok(())
    }
    async fn subscribe_events(&self, _c: &VendorCreds) -> Result<(), AdapterError> {
        Ok(())
    }
}

#[tokio::test]
async fn trait_object_roundtrip() {
    let adapter: Box<dyn VendorAdapter> = Box::new(Dummy);
    let creds = VendorCreds {
        client_id: "c".into(),
        client_secret: "s".into(),
        uid: "u".into(),
        access_token: "at".into(),
        refresh_token: "rt".into(),
        expires_at: 0,
    };
    let devs = adapter.list_devices(&creds).await.unwrap();
    assert_eq!(devs.len(), 1);
    assert_eq!(devs[0].properties[0].value, json!(23.5));
}

#[test]
fn event_message_json_shape() {
    let ev = EventMessage {
        device_id: "d1".into(),
        tenant_id: "t1".into(),
        kind: "property".into(),
        code: "temp".into(),
        value: json!(23.5),
        ts: 1690000000000,
    };
    let s = serde_json::to_string(&ev).unwrap();
    // 字段名固定：Kafka 消费者（P2 iot-data）依赖此形状
    assert!(s.contains("\"device_id\":\"d1\""));
    assert!(s.contains("\"kind\":\"property\""));
    assert!(s.contains("\"ts\":1690000000000"));
}
```

- [x] **Step 2: 运行确认失败**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test adapter_trait`
Expected: 编译失败（`iot_access::adapter`、`iot_access::models` 不存在）。

- [x] **Step 3: 实现 models.rs（统一模型，全平台唯一真源）**

`e-cat/services/iot-access/src/models.rs`:

```rust
use serde::{Deserialize, Serialize};

/// 物模型属性值。code 为厂商属性 code（涂鸦）或设备自定义 code（直连）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PropertyValue {
    pub code: String,
    pub value: serde_json::Value,
}

/// 统一设备记录：厂商设备拉取后的中间形态，入库前转换。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceRecord {
    /// 平台侧设备 UUID（device_links.device_id）
    pub id: String,
    /// 厂商侧设备 ID（涂鸦 devId；直连设备等于平台 UUID）
    pub vendor_id: String,
    pub name: String,
    /// 厂商品类（涂鸦 category；直连设备为 "direct"）
    pub category: String,
    pub online: bool,
    pub properties: Vec<PropertyValue>,
}

/// 统一事件消息：Webhook、MQTT 直连、Kafka `iot.events`、Redis 影子共用。
/// kind 取值：`"property"` | `"online"` | `"offline"`。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EventMessage {
    pub device_id: String,
    pub tenant_id: String,
    pub kind: String,
    /// property 时为属性 code；online/offline 时为 "online"/"offline"
    pub code: String,
    pub value: serde_json::Value,
    /// epoch 毫秒
    pub ts: i64,
}
```

- [x] **Step 4: 实现 adapter.rs（Trait + 凭据结构 + 注册表）**

`e-cat/services/iot-access/src/adapter.rs`:

```rust
use crate::models::{DeviceRecord, PropertyValue};
use async_trait::async_trait;

/// 解密后的厂商凭据（DB 中存 AES 密文，见 crypto.rs / store.rs）
#[derive(Clone, Debug)]
pub struct VendorCreds {
    pub client_id: String,
    pub client_secret: String,
    pub uid: String,
    pub access_token: String,
    pub refresh_token: String,
    /// access_token 过期 epoch 秒；过期时由适配器用 refresh_token 刷新
    pub expires_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("vendor api error: {0}")]
    Vendor(String),
    #[error("token expired and refresh failed: {0}")]
    Refresh(String),
    #[error("unknown vendor: {0}")]
    UnknownVendor(String),
    #[error("internal: {0}")]
    Internal(String),
}

/// 厂商适配器统一接口。subscribe_events 的语义：注册厂商侧事件推送
/// （涂鸦为控制台配置 Webhook URL，事件由 webhook.rs 消费，故返回 Ok）；
/// 直连设备的"订阅"在 mqtt.rs 中按设备逐个建立，不经过本 Trait。
#[async_trait]
pub trait VendorAdapter: Send + Sync {
    async fn list_devices(&self, creds: &VendorCreds) -> Result<Vec<DeviceRecord>, AdapterError>;
    async fn get_properties(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
    ) -> Result<Vec<PropertyValue>, AdapterError>;
    async fn send_command(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
        code: &str,
        value: serde_json::Value,
    ) -> Result<(), AdapterError>;
    async fn subscribe_events(&self, creds: &VendorCreds) -> Result<(), AdapterError>;
}

/// 注册表：vendor 名（devices.vendor 列的值）→ 适配器。P4 补 miot/huawei/aws/azure。
pub fn adapter_for(vendor: &str) -> Result<Box<dyn VendorAdapter>, AdapterError> {
    match vendor {
        "tuya" => Ok(Box::new(crate::adapters::tuya::TuyaAdapter::new())),
        v => Err(AdapterError::UnknownVendor(v.to_string())),
    }
}
```

- [x] **Step 5: 更新 lib.rs 与适配器目录骨架**

`e-cat/services/iot-access/src/lib.rs`（替换）：

```rust
pub mod adapter;
pub mod adapters;
pub mod models;
```

`e-cat/services/iot-access/src/adapters/mod.rs`:

```rust
pub mod tuya;
```

注：`tuya.rs` 由 Task 5 实现；本步先建同目录 `tuya.rs` 空文件占位（`// 见 Task 5`），保证编译。

- [x] **Step 6: 运行测试确认通过**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test adapter_trait`
Expected: 2 个测试全 PASS。

- [x] **Step 7: Commit**

```bash
git add e-cat/services/iot-access/
git commit -m "feat(access): unified models (DeviceRecord/EventMessage) + VendorAdapter trait"
```

---

### Task 3: 凭据加密与数据库迁移（vendor_credentials + device_links）

**Files:**
- Create: `e-cat/services/iot-access/src/crypto.rs`
- Modify: `e-cat/services/iot-access/src/lib.rs`
- Create: `e-cat/services/iot-device/migrations/0002_vendor_auth.sql`
- Modify: `e-cat/services/iot-device/src/main.rs`
- Create: `e-cat/services/iot-access/tests/crypto.rs`

- [ ] **Step 1: 写失败测试**

`e-cat/services/iot-access/tests/crypto.rs`:

```rust
use iot_access::crypto::{decrypt, encrypt, derive_key};

#[test]
fn roundtrip() {
    let key = derive_key("dev-key-0123456789abcdef");
    let enc = encrypt(&key, br#"{"client_id":"c","secret":"s"}"#).unwrap();
    assert_ne!(enc.as_bytes(), br#"{"client_id":"c","secret":"s"}"#);
    let dec = decrypt(&key, &enc).unwrap();
    assert_eq!(dec, br#"{"client_id":"c","secret":"s"}"#);
}

#[test]
fn wrong_key_fails() {
    let k1 = derive_key("key-one-key-one-key-one");
    let k2 = derive_key("key-two-key-two-key-two");
    let enc = encrypt(&k1, b"data").unwrap();
    assert!(decrypt(&k2, &enc).is_err());
}

#[test]
fn ciphertext_has_nonce_and_tag() {
    let key = derive_key("dev-key-0123456789abcdef");
    let enc = encrypt(&key, b"data").unwrap();
    // base64(12 字节 nonce + 16 字节 tag + 密文) 解码后长度 ≥ 28
    let raw = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &enc).unwrap();
    assert!(raw.len() >= 28);
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test crypto`
Expected: 编译失败（`iot_access::crypto` 不存在）。

- [ ] **Step 3: 实现 crypto.rs（AES-256-GCM）**

`e-cat/services/iot-access/src/crypto.rs`:

```rust
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::Engine;

/// 密钥派生：SHA-256(环境变量 IOT_CRED_ENCRYPT_KEY 的值)，任意长度输入 → 32 字节。
pub fn derive_key(env_value: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(env_value.as_bytes()).into()
}

pub fn encrypt(key: &[u8; 32], plain: &[u8]) -> Result<String, String> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce = [0u8; 12];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plain)
        .map_err(|e| format!("aes encrypt: {e}"))?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    Ok(base64::engine::general_purpose::STANDARD.encode(out))
}

pub fn decrypt(key: &[u8; 32], enc: &str) -> Result<Vec<u8>, String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(enc)
        .map_err(|e| format!("base64 decode: {e}"))?;
    if raw.len() < 28 {
        return Err("ciphertext too short".into());
    }
    let (nonce, ct) = raw.split_at(12);
    let cipher = Aes256Gcm::new(key.into());
    cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|e| format!("aes decrypt (密钥错误或密文被篡改): {e}"))
}
```

- [ ] **Step 4: 写迁移 SQL（幂等）**

`e-cat/services/iot-device/migrations/0002_vendor_auth.sql`:

```sql
CREATE TABLE IF NOT EXISTS vendor_credentials (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    vendor VARCHAR(64) NOT NULL,
    config_encrypted TEXT NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uq_cred_tenant_vendor (tenant_id, vendor),
    CONSTRAINT fk_cred_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS device_links (
    device_id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    vendor VARCHAR(64) NOT NULL,
    vendor_id VARCHAR(128) NOT NULL,
    vendor_name VARCHAR(255) NOT NULL DEFAULT '',
    category VARCHAR(64) NOT NULL DEFAULT '',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uq_link_vendor_vendorid (vendor, vendor_id),
    CONSTRAINT fk_link_device FOREIGN KEY (device_id) REFERENCES devices(id),
    CONSTRAINT fk_link_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;
```

注：`vendor_credentials.config_encrypted` 存 AES-GCM 密文 JSON（字段见 Task 4 `VendorCreds`）。`device_links` 存厂商设备 ID ↔ 平台设备 UUID 映射；`devices.vendor` 列记录厂商名。

- [ ] **Step 5: 更新 iot-device 迁移加载（两个文件都跑）**

`e-cat/services/iot-device/src/main.rs` 的 `migrate` 函数替换为：

```rust
async fn migrate(db: &SqlxClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for file in ["migrations/0001_init.sql", "migrations/0002_vendor_auth.sql"] {
        let sql = std::fs::read_to_string(file)?;
        // execute 逐条执行: sqlx Any 驱动不启用 multi-statements,整文件一次 execute 会 1064
        for stmt in sql.split(';').filter(|s| !s.trim().is_empty()) {
            db.execute(stmt).await?;
        }
    }
    Ok(())
}
```

- [ ] **Step 6: 更新 lib.rs**

`e-cat/services/iot-access/src/lib.rs`（替换）：

```rust
pub mod adapter;
pub mod adapters;
pub mod crypto;
pub mod models;
```

- [ ] **Step 7: 运行测试确认通过 + 验证迁移**

Run:
```bash
cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test crypto
cd /home/wwwroot/iot-platform && docker compose up -d mysql && sleep 5
cd /home/wwwroot/iot-platform/e-cat/services/iot-device && cargo run &
sleep 8 && curl -s http://localhost:8081/health
docker exec -i iot-platform-mysql-1 mysql -uiot -piot iot -e "SHOW TABLES;"
```
Expected: 3 个测试 PASS；`/health` 返回 `{"db":true,...}`；`SHOW TABLES` 含 `devices`、`tenants`、`vendor_credentials`、`device_links`。

- [ ] **Step 8: Commit**

```bash
git add e-cat/services/iot-access/ e-cat/services/iot-device/
git commit -m "feat(access): AES-256-GCM credential encryption + vendor_auth migration"
```

---

### Task 4: 凭据存储与 OAuth 授权码流程 API

**Files:**
- Create: `e-cat/services/iot-access/src/store.rs`
- Create: `e-cat/services/iot-access/src/oauth.rs`
- Modify: `e-cat/services/iot-access/src/lib.rs`
- Create: `e-cat/services/iot-access/tests/oauth_state.rs`

- [ ] **Step 1: 写失败测试（state 编码与凭据加解密 JSON 往返）**

`e-cat/services/iot-access/tests/oauth_state.rs`:

```rust
use iot_access::crypto::{decrypt, derive_key, encrypt};
use iot_access::oauth::{decode_state, encode_state};
use iot_access::store::creds_json;

#[test]
fn state_roundtrip() {
    let s = encode_state("t1", "tuya");
    assert_eq!(decode_state(&s).unwrap(), ("t1".to_string(), "tuya".to_string()));
}

#[test]
fn creds_json_roundtrip() {
    let key = derive_key("dev-key-0123456789abcdef");
    let cfg = serde_json::json!({
        "client_id": "cid", "client_secret": "cs",
        "uid": "u1", "access_token": "at", "refresh_token": "rt", "expires_at": 1690000000
    });
    let enc = encrypt(&key, &creds_json(&cfg)).unwrap();
    let dec = decrypt(&key, &enc).unwrap();
    assert_eq!(dec, creds_json(&cfg));
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test oauth_state`
Expected: 编译失败（`iot_access::oauth`、`iot_access::store` 不存在）。

- [ ] **Step 3: 实现 store.rs（凭据读写 MySQL）**

`e-cat/services/iot-access/src/store.rs`:

```rust
use crate::crypto::{decrypt, derive_key, encrypt};
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
```

- [ ] **Step 4: 实现 oauth.rs（授权 URL + 回调端点）**

`e-cat/services/iot-access/src/oauth.rs`:

```rust
use crate::adapter::VendorCreds;
use crate::store::Store;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

/// state = base64url(tenant_id:vendor)，回调时还原租户归属。
pub fn encode_state(tenant_id: &str, vendor: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(format!("{tenant_id}:{vendor}"))
}

pub fn decode_state(state: &str) -> Result<(String, String), String> {
    use base64::Engine;
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(state)
        .map_err(|e| format!("bad state: {e}"))?;
    let s = String::from_utf8(raw).map_err(|e| format!("bad state: {e}"))?;
    let (t, v) = s.split_once(':').ok_or_else(|| "bad state".to_string())?;
    Ok((t.to_string(), v.to_string()))
}

#[derive(Clone)]
pub struct OauthState {
    pub store: Arc<Store>,
    /// 涂鸦开放平台 client_id（授权 URL 用）
    pub tuya_client_id: String,
    /// 授权完成后浏览器跳回的地址（含 /api/access/oauth/callback）
    pub callback_base: String,
}

#[derive(Deserialize)]
pub struct AuthorizeReq {
    pub vendor: String,
}

#[derive(Serialize)]
pub struct AuthorizeResp {
    pub url: String,
}

/// POST /api/access/oauth/authorize-url（受保护：需 x-tenant-id）
pub async fn authorize_url(
    State(oauth): State<OauthState>,
    axum::Extension(tenant_id): axum::Extension<String>,
    Json(req): Json<AuthorizeReq>,
) -> Result<Json<AuthorizeResp>, (StatusCode, String)> {
    if req.vendor != "tuya" {
        return Err((StatusCode::BAD_REQUEST, format!("vendor {} not supported", req.vendor)));
    }
    let state = encode_state(&tenant_id, &req.vendor);
    let url = format!(
        "https://openapi.tuyacn.com/oauth2/auth?client_id={}&response_type=code&redirect_uri={}&state={}",
        oauth.tuya_client_id, oauth.callback_base, state
    );
    Ok(Json(AuthorizeResp { url }))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

/// GET /api/access/oauth/callback（公开：浏览器从涂鸦跳回，无 JWT）
/// 用授权码换 token 并加密落库；返回 HTML 提示可关闭窗口。
pub async fn callback(
    State(oauth): State<OauthState>,
    Query(q): Query<CallbackQuery>,
) -> Result<axum::response::Html<String>, (StatusCode, String)> {
    let (tenant_id, vendor) = decode_state(&q.state)?;
    if vendor != "tuya" {
        return Err((StatusCode::BAD_REQUEST, "unsupported vendor in state".into()));
    }
    let creds = exchange_authorization_code(&q.code, &oauth.tuya_client_id).await?;
    oauth
        .store
        .save_creds(&tenant_id, "tuya", &serde_json::to_value(&creds).unwrap())
        .await?;
    Ok(axum::response::Html(
        "<html><body><h2>授权成功，可关闭此窗口</h2></body></html>".to_string(),
    ))
}

/// 调涂鸦 token 端点换授权码（真实环境走 openapi.tuyacn.com；测试指向 mock）。
pub async fn exchange_authorization_code(
    code: &str,
    client_id: &str,
) -> Result<VendorCreds, String> {
    let base = std::env::var("TUYA_OPENAPI_BASE").unwrap_or_else(|_| "https://openapi.tuyacn.com".into());
    let client_secret = std::env::var("TUYA_CLIENT_SECRET").map_err(|_| "TUYA_CLIENT_SECRET not set".to_string())?;
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string();
    // 取 token 时签名不携带 access_token（约定为空串）
    let sign = crate::adapters::tuya::sign(&client_id, &t, "", &client_secret);
    let url = format!(
        "{base}/v1.0/token?grant_type=authorization_code&code={code}"
    );
    let resp = reqwest::Client::new()
        .get(&url)
        .header("client_id", &client_id)
        .header("t", &t)
        .header("sign_method", "HMAC-SHA256")
        .header("sign", sign)
        .send()
        .await
        .map_err(|e| format!("tuya token request: {e}"))?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("tuya token parse: {e}"))?;
    if body["success"] != true {
        return Err(format!("tuya token error: {body}"));
    }
    let r = &body["result"];
    let expires_in = r["expire_time"].as_i64().unwrap_or(2592000);
    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + expires_in;
    Ok(VendorCreds {
        client_id: client_id.to_string(),
        client_secret,
        uid: r["uid"].as_str().unwrap_or("").to_string(),
        access_token: r["access_token"].as_str().unwrap_or("").to_string(),
        refresh_token: r["refresh_token"].as_str().unwrap_or("").to_string(),
        expires_at,
    })
}

/// 受保护路由（authorize-url 需 x-tenant-id）。
pub fn router(oauth: OauthState) -> axum::Router {
    axum::Router::new()
        .route("/oauth/authorize-url", axum::routing::post(authorize_url))
        .with_state(oauth)
}

/// 公开路由（浏览器从涂鸦跳回，无 JWT/tenant）。
pub fn router_public(oauth: OauthState) -> axum::Router {
    axum::Router::new()
        .route("/oauth/callback", axum::routing::get(callback))
        .with_state(oauth)
}
```

注：`use serde_json::{Value, json}` 中 `json` 在 oauth.rs 未使用，编译报未使用导入时删掉 `json`（`Value` 在 `exchange_authorization_code` 中用到）。

- [ ] **Step 5: 更新 lib.rs**

`e-cat/services/iot-access/src/lib.rs`（替换）：

```rust
pub mod adapter;
pub mod adapters;
pub mod crypto;
pub mod models;
pub mod oauth;
pub mod store;
```

- [ ] **Step 6: 运行测试确认通过**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test oauth_state`
Expected: 2 个测试全 PASS（`exchange_authorization_code` 未在测试中调用，不触发网络）。

- [ ] **Step 7: Commit**

```bash
git add e-cat/services/iot-access/
git commit -m "feat(access): OAuth authorization-code flow API + encrypted credential store"
```

---

### Task 5: 涂鸦适配器（OAuth 刷新 + OpenAPI + 设备导入端点）

**Files:**
- Create: `e-cat/services/iot-access/src/adapters/tuya.rs`（替换占位文件）
- Create: `e-cat/services/iot-access/src/api.rs`
- Modify: `e-cat/services/iot-access/src/lib.rs`
- Create: `e-cat/services/iot-access/tests/tuya_sign.rs`

- [x] **Step 1: 写失败测试（签名向量 + 刷新逻辑）**

`e-cat/services/iot-access/tests/tuya_sign.rs`:

```rust
use iot_access::adapters::tuya::{sign, TuyaAdapter};
use iot_access::adapter::VendorCreds;

#[test]
fn sign_matches_hmac_sha256_hex() {
    // HMAC-SHA256("client_id" + "1690000000000" + "", "secret") 的 hex
    let s = sign("client_id", "1690000000000", "", "secret");
    assert_eq!(
        s,
        "e9c1b96e2fd0f0e79a1ec52d92ff44b37f59b6fbe7d3e1f6d6fbbbe28a3d8b0e"
    );
}
```

注：上面的 hex 是占位示例值——Step 2 先运行测试看失败，然后用 `python3 -c "import hmac,hashlib;print(hmac.new(b'secret', b'client_id'+b'1690000000000', hashlib.sha256).hexdigest())"` 计算真实值替换后再跑。签名格式是确定的：`HMAC-SHA256(client_id + t + access_token, secret)` 输出小写 hex，与涂鸦官方一致。

- [x] **Step 2: 运行确认失败**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test tuya_sign`
Expected: 编译失败（`iot_access::adapters::tuya` 不存在）。

- [x] **Step 3: 实现 tuya.rs（Trait 首个实现）**

`e-cat/services/iot-access/src/adapters/tuya.rs`:

```rust
use crate::adapter::{AdapterError, VendorAdapter, VendorCreds};
use crate::models::{DeviceRecord, PropertyValue};
use async_trait::async_trait;
use serde_json::{Value, json};

/// 涂鸦签名：HMAC-SHA256(client_id + t + access_token, secret)，hex 小写。
pub fn sign(client_id: &str, t: &str, access_token: &str, secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(client_id.as_bytes());
    mac.update(t.as_bytes());
    mac.update(access_token.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn now_ms() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()
}

pub struct TuyaAdapter {
    http: reqwest::Client,
}

impl TuyaAdapter {
    pub fn new() -> Self {
        Self { http: reqwest::Client::new() }
    }

    fn base(&self) -> String {
        std::env::var("TUYA_OPENAPI_BASE").unwrap_or_else(|_| "https://openapi.tuyacn.com".into())
    }

    /// 带签名的 GET；access_token 过期时用 refresh_token 换新后重试一次。
    async fn get(
        &self,
        creds: &VendorCreds,
        path: &str,
    ) -> Result<Value, AdapterError> {
        self.get_once(creds, path).await.or_else(|e| match e {
            AdapterError::Vendor(ref msg)
                if msg.contains("token")
                    || msg.contains("ACCESS_TOKEN_SESSION_INVALID")
                    || msg.contains("21000002") =>
            {
                let refreshed = self.refresh_token(creds).await?;
                self.get_once(&refreshed, path).await
            }
            other => Err(other),
        })
    }

    async fn get_once(
        &self,
        creds: &VendorCreds,
        path: &str,
    ) -> Result<Value, AdapterError> {
        let t = now_ms();
        let sign = sign(&creds.client_id, &t, &creds.access_token, &creds.client_secret);
        let url = format!("{}{path}", self.base());
        let resp = self
            .http
            .get(&url)
            .header("client_id", &creds.client_id)
            .header("t", &t)
            .header("sign_method", "HMAC-SHA256")
            .header("access_token", &creds.access_token)
            .header("sign", sign)
            .send()
            .await
            .map_err(|e| AdapterError::Vendor(format!("request: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AdapterError::Vendor(format!("parse: {e}")))?;
        if body["success"] != true {
            return Err(AdapterError::Vendor(format!("tuya error: {body}")));
        }
        Ok(body["result"].clone())
    }

    /// grant_type=refresh_token 换新；成功写回 DB（调用方忽略返回值则只更新内存凭据）。
    pub async fn refresh_token(&self, creds: &VendorCreds) -> Result<VendorCreds, AdapterError> {
        let t = now_ms();
        let sign = sign(&creds.client_id, &t, "", &creds.client_secret);
        let url = format!(
            "{}/v1.0/token?grant_type=refresh_token&refresh_token={}",
            self.base(),
            creds.refresh_token
        );
        let resp = self
            .http
            .get(&url)
            .header("client_id", &creds.client_id)
            .header("t", &t)
            .header("sign_method", "HMAC-SHA256")
            .header("sign", sign)
            .send()
            .await
            .map_err(|e| AdapterError::Refresh(format!("request: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AdapterError::Refresh(format!("parse: {e}")))?;
        if body["success"] != true {
            return Err(AdapterError::Refresh(format!("tuya refresh error: {body}")));
        }
        let r = &body["result"];
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + r["expire_time"].as_i64().unwrap_or(2592000);
        Ok(VendorCreds {
            client_id: creds.client_id.clone(),
            client_secret: creds.client_secret.clone(),
            uid: r["uid"].as_str().unwrap_or(&creds.uid).to_string(),
            access_token: r["access_token"].as_str().unwrap_or("").to_string(),
            refresh_token: r["refresh_token"].as_str().unwrap_or("").to_string(),
            expires_at,
        })
    }

    fn to_record(&self, dev: &Value) -> DeviceRecord {
        let status = dev["status"].as_array().cloned().unwrap_or_default();
        DeviceRecord {
            id: String::new(), // 由 store.upsert_device 回填
            vendor_id: dev["id"].as_str().unwrap_or("").to_string(),
            name: dev["name"].as_str().unwrap_or("").to_string(),
            category: dev["category"].as_str().unwrap_or("").to_string(),
            online: dev["online"].as_bool().unwrap_or(false),
            properties: status
                .iter()
                .filter_map(|s| {
                    Some(PropertyValue {
                        code: s["code"].as_str()?.to_string(),
                        value: s["value"].clone(),
                    })
                })
                .collect(),
        }
    }
}

impl Default for TuyaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VendorAdapter for TuyaAdapter {
    /// GET /v1.0/users/{uid}/devices
    async fn list_devices(&self, creds: &VendorCreds) -> Result<Vec<DeviceRecord>, AdapterError> {
        let result = self
            .get(creds, &format!("/v1.0/users/{}/devices", creds.uid))
            .await?;
        let devices = result.as_array().cloned().unwrap_or_default();
        Ok(devices.iter().map(|d| self.to_record(d)).collect())
    }

    /// GET /v1.0/devices/{deviceId}/status
    async fn get_properties(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
    ) -> Result<Vec<PropertyValue>, AdapterError> {
        let result = self
            .get(creds, &format!("/v1.0/devices/{vendor_id}/status"))
            .await?;
        Ok(result
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|s| {
                Some(PropertyValue {
                    code: s["code"].as_str()?.to_string(),
                    value: s["value"].clone(),
                })
            })
            .collect())
    }

    /// POST /v1.0/devices/{deviceId}/commands
    async fn send_command(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
        code: &str,
        value: serde_json::Value,
    ) -> Result<(), AdapterError> {
        let refreshed = self.maybe_refresh(creds).await?;
        let t = now_ms();
        let sign = sign(
            &refreshed.client_id,
            &t,
            &refreshed.access_token,
            &refreshed.client_secret,
        );
        let url = format!(
            "{}/v1.0/devices/{vendor_id}/commands",
            self.base()
        );
        let resp = self
            .http
            .post(&url)
            .header("client_id", &refreshed.client_id)
            .header("t", &t)
            .header("sign_method", "HMAC-SHA256")
            .header("access_token", &refreshed.access_token)
            .header("sign", sign)
            .json(&json!({ "commands": [ { "code": code, "value": value } ] }))
            .send()
            .await
            .map_err(|e| AdapterError::Vendor(format!("command request: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AdapterError::Vendor(format!("command parse: {e}")))?;
        if body["success"] != true {
            return Err(AdapterError::Vendor(format!("command error: {body}")));
        }
        Ok(())
    }

    /// 涂鸦事件经控制台配置的 Webhook URL 推送，由 webhook.rs 接收。
    async fn subscribe_events(&self, _creds: &VendorCreds) -> Result<(), AdapterError> {
        Ok(())
    }
}

impl TuyaAdapter {
    /// access_token 过期（expires_at 距今 < 60s）则刷新并返回新凭据。
    async fn maybe_refresh(&self, creds: &VendorCreds) -> Result<VendorCreds, AdapterError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        if creds.expires_at == 0 || creds.expires_at - now < 60 {
            self.refresh_token(creds).await
        } else {
            Ok(creds.clone())
        }
    }
}
```

- [x] **Step 4: 实现 api.rs（设备导入 + 指令下发端点）**

`e-cat/services/iot-access/src/api.rs`:

```rust
use crate::adapter::adapter_for;
use crate::store::Store;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use ecat_data_redis::RedisCache;
use ecat_mq_kafka::KafkaMq;
use ecat_mq_mqtt::MqttMq;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiState {
    pub store: Arc<Store>,
    pub kafka: Arc<KafkaMq>,
    pub redis: Arc<RedisCache>,
    pub mqtt: Arc<MqttMq>,
}

/// 构造受保护 API 路由（挂载见 main.rs，路径前缀 /api/access）。
pub fn router(api: ApiState) -> axum::Router {
    axum::Router::new()
        .route("/vendors/{vendor}/import", axum::routing::post(import_devices))
        .route("/devices/{device_id}/command", axum::routing::post(send_command))
        .with_state(api)
}

/// POST /api/access/vendors/{vendor}/import（受保护）
/// 拉取厂商设备列表 → 入库 devices + device_links。
pub async fn import_devices(
    State(api): State<ApiState>,
    axum::Extension(tenant_id): axum::Extension<String>,
    Path(vendor): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let adapter = adapter_for(&vendor).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let creds = api
        .store
        .load_creds(&tenant_id, &vendor)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let creds: crate::adapter::VendorCreds =
        serde_json::from_value(creds).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let devices = adapter
        .list_devices(&creds)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    let mut imported = Vec::new();
    for d in &devices {
        let platform_id = api
            .store
            .upsert_device(&tenant_id, &vendor, &d.vendor_id, &d.name, &d.category, d.online)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        imported.push(json!({ "platform_id": platform_id, "vendor_id": d.vendor_id, "name": d.name }));
    }
    Ok(Json(json!({ "imported": imported, "count": imported.len() })))
}

#[derive(Deserialize)]
pub struct CommandReq {
    pub code: String,
    pub value: Value,
}

/// POST /api/access/devices/{id}/command（受保护）
/// 查 device_links → 适配器 send_command → 厂商 OpenAPI / 直连 MQTT 下发。
pub async fn send_command(
    State(api): State<ApiState>,
    axum::Extension(tenant_id): axum::Extension<String>,
    Path(device_id): Path<String>,
    Json(req): Json<CommandReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let link = api
        .store
        .find_link(&device_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let (vendor, vendor_id) = link
        .ok_or_else(|| (StatusCode::NOT_FOUND, "device not linked".to_string()))?;
    if vendor == "direct" {
        // 直连设备：MQTT 下发
        crate::mqtt::publish_command(&device_id, &req.code, &req.value)
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
        return Ok(Json(json!({ "ok": true, "channel": "mqtt" })));
    }
    let adapter = adapter_for(&vendor).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let creds = api
        .store
        .load_creds(&tenant_id, &vendor)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let creds: crate::adapter::VendorCreds =
        serde_json::from_value(creds).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    adapter
        .send_command(&creds, &vendor_id, &req.code, req.value.clone())
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(json!({ "ok": true, "channel": vendor })))
}
```

- [x] **Step 5: 更新 lib.rs + store 加 find_link**

`e-cat/services/iot-access/src/lib.rs`（替换）：

```rust
pub mod adapter;
pub mod adapters;
pub mod api;
pub mod crypto;
pub mod mqtt;
pub mod models;
pub mod oauth;
pub mod store;
pub mod webhook;
```

注：`mqtt.rs` 由 Task 7 实现、`webhook.rs` 由 Task 6 实现；本步先建两个空占位文件（`// 见 Task 6` / `// 见 Task 7`）保证编译。

`e-cat/services/iot-access/src/store.rs` 加方法（插在 `device_name` 后）：

```rust
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
```

- [x] **Step 6: 计算真实签名向量并运行测试**

Run:
```bash
python3 -c "import hmac,hashlib;print(hmac.new(b'secret', b'client_id'+b'1690000000000', hashlib.sha256).hexdigest())"
```
Expected: 输出 64 位 hex —— 用它替换 `tests/tuya_sign.rs` 中的断言值，然后：
Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test tuya_sign`
Expected: 1 个测试 PASS。

- [x] **Step 7: Commit**

```bash
git add e-cat/services/iot-access/
git commit -m "feat(access): Tuya adapter (OAuth refresh + OpenAPI list/status/command) + import endpoint"
```

---

### Task 6: 涂鸦 Webhook 接收 + 事件入 Kafka + 设备影子

**Files:**
- Create: `e-cat/services/iot-access/src/events.rs`
- Create: `e-cat/services/iot-access/src/webhook.rs`（替换占位文件）
- Modify: `e-cat/services/iot-access/src/lib.rs`
- Create: `e-cat/services/iot-access/tests/webhook.rs`

- [x] **Step 1: 写失败测试（事件归一化纯函数，无需 Docker）**

`e-cat/services/iot-access/tests/webhook.rs`:

```rust
use iot_access::webhook::{normalize_event, WebhookPayload};

#[test]
fn data_as_json_string_is_normalized() {
    let p = WebhookPayload {
        r#type: "deviceData".into(),
        biz_code: "report".into(),
        data: serde_json::json!(
            "{\"deviceId\":\"tuya-dev-1\",\"code\":\"temp\",\"value\":23.5,\"ts\":1690000000000}"
        ),
    };
    let ev = normalize_event("plat-dev-1", "t1", &p).unwrap();
    assert_eq!(ev.device_id, "plat-dev-1");
    assert_eq!(ev.tenant_id, "t1");
    assert_eq!(ev.kind, "property");
    assert_eq!(ev.code, "temp");
    assert_eq!(ev.value, serde_json::json!(23.5));
    assert_eq!(ev.ts, 1690000000000);
}

#[test]
fn data_as_object_is_normalized() {
    let p = WebhookPayload {
        r#type: "deviceData".into(),
        biz_code: "online".into(),
        data: serde_json::json!({"deviceId": "tuya-dev-1"}),
    };
    let ev = normalize_event("plat-dev-1", "t1", &p).unwrap();
    assert_eq!(ev.kind, "online");
}

#[test]
fn unknown_bizcode_is_error() {
    let p = WebhookPayload {
        r#type: "deviceData".into(),
        biz_code: "delete".into(),
        data: serde_json::json!({"deviceId": "tuya-dev-1"}),
    };
    assert!(normalize_event("d", "t", &p).is_err());
}
```

- [x] **Step 2: 运行确认失败**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test webhook`
Expected: 编译失败（`iot_access::webhook` 无 `normalize_event`/`WebhookPayload`）。

- [x] **Step 3: 实现 events.rs（Kafka 发布 + 影子更新）**

`e-cat/services/iot-access/src/events.rs`:

```rust
use crate::models::EventMessage;
use ecat_data::Cache;
use ecat_data_redis::RedisCache;
use ecat_mq::MqError;
use ecat_mq_kafka::KafkaMq;
use serde_json::{Value, json};

/// 事件总线 topic：P2 的 iot-data 消费者订阅此 topic。
pub const TOPIC_EVENTS: &str = "iot.events";

/// 影子键前缀：`shadow:{device_id}`。
pub fn shadow_key(device_id: &str) -> String {
    format!("shadow:{device_id}")
}

pub async fn publish_event(mq: &KafkaMq, ev: &EventMessage) -> Result<(), MqError> {
    let payload = serde_json::to_vec(ev).map_err(|e| MqError::Other(format!("encode: {e}")))?;
    mq.publish(TOPIC_EVENTS, &payload).await
}

/// 影子结构：{"online":bool,"properties":{code:value},"ts":ms}。
/// property 事件隐含设备在线；online/offline 只改在线标记。
pub async fn shadow_apply(redis: &RedisCache, ev: &EventMessage) -> Result<(), String> {
    let key = shadow_key(&ev.device_id);
    let mut shadow: Value = redis
        .get(&key)
        .await
        .map_err(|e| format!("shadow get: {e}"))?
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_else(|| json!({ "online": false, "properties": {} }));
    match ev.kind.as_str() {
        "property" => {
            shadow["online"] = json!(true);
            shadow["properties"][ev.code.clone()] = ev.value.clone();
        }
        "online" => shadow["online"] = json!(true),
        "offline" => shadow["online"] = json!(false),
        _ => {}
    }
    shadow["ts"] = json!(ev.ts);
    let raw = serde_json::to_vec(&shadow).map_err(|e| format!("shadow encode: {e}"))?;
    // ttl = 0 → 无过期时间（Cache::set 内部走 SET 而非 PSETEX）
    redis
        .set(&key, &raw, std::time::Duration::ZERO)
        .await
        .map_err(|e| format!("shadow set: {e}"))
}
```

- [x] **Step 4: 实现 webhook.rs（接收 + 验签 + 归一化 + 发布）**

`e-cat/services/iot-access/src/webhook.rs`:

```rust
use crate::events::{publish_event, shadow_apply};
use crate::models::EventMessage;
use crate::store::Store;
use axum::{
    Json,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use ecat_data_redis::RedisCache;
use ecat_mq_kafka::KafkaMq;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

/// 涂鸦 Webhook 原始结构：type/bizCode/data（data 可能为 JSON 字符串或对象）。
#[derive(Deserialize)]
pub struct WebhookPayload {
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(rename = "bizCode")]
    pub biz_code: String,
    pub data: Value,
}

/// 归一化：data 解包（字符串→内层 JSON），bizCode → EventMessage.kind。
/// 返回 Err 表示不支持的 bizCode（如 delete），调用方直接丢弃。
pub fn normalize_event(
    platform_id: &str,
    tenant_id: &str,
    p: &WebhookPayload,
) -> Result<EventMessage, String> {
    let data = match &p.data {
        Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
        other => other.clone(),
    };
    let code = data["code"].as_str().unwrap_or("").to_string();
    let value = data["value"].clone();
    let ts = data["ts"].as_i64().unwrap_or_else(now_ms);
    let (kind, ev_code, ev_value) = match p.biz_code.as_str() {
        "report" => ("property", code, value),
        "online" => ("online", "online".to_string(), json!(true)),
        "offline" => ("offline", "offline".to_string(), json!(false)),
        other => return Err(format!("unsupported bizCode: {other}")),
    };
    Ok(EventMessage {
        device_id: platform_id.to_string(),
        tenant_id: tenant_id.to_string(),
        kind: kind.to_string(),
        code: ev_code,
        value: ev_value,
        ts,
    })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[derive(Clone)]
pub struct WebhookState {
    pub store: Arc<Store>,
    pub kafka: Arc<KafkaMq>,
    pub redis: Arc<RedisCache>,
}

/// POST /api/access/webhook/tuya（公开：涂鸦服务器回调，无 JWT）。
pub async fn receive(
    State(ws): State<WebhookState>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let raw = match to_bytes(body, 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(json!({"error": "body too large"})),
            )
                .into_response()
        }
    };
    let p: WebhookPayload = match serde_json::from_slice(&raw) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("bad payload: {e}")})),
            )
                .into_response()
        }
    };
    let device_id = match extract_device_id(&p) {
        Some(d) => d,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "no deviceId in data"})),
            )
                .into_response()
        }
    };
    // 涂鸦设备 ID → 平台设备 + 租户
    let platform_id = match ws
        .store
        .find_device_by_vendor_id("tuya", &device_id)
        .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::warn!(device_id, "webhook event for unknown device, dropped");
            return (StatusCode::OK, Json(json!({"accepted": false}))).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e})),
            )
                .into_response()
        }
    };
    let tenant_id = match ws.store.tenant_of_device(&platform_id).await {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e})),
            )
                .into_response()
        }
    };
    // 签名校验：header 存在则必须通过（HMAC-SHA256(raw body, client_secret)）
    if let Some(sig) = headers
        .get("x-tuya-signature")
        .and_then(|v| v.to_str().ok())
    {
        let secret = match ws.store.load_creds(&tenant_id, "tuya").await {
            Ok(c) => c["client_secret"].as_str().unwrap_or("").to_string(),
            Err(_) => String::new(),
        };
        if !secret.is_empty() && !verify_signature(&secret, &raw, sig) {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "bad tuya signature"})),
            )
                .into_response();
        }
    }
    let ev = match normalize_event(&platform_id, &tenant_id, &p) {
        Ok(ev) => ev,
        Err(_) => return (StatusCode::OK, Json(json!({"accepted": false}))).into_response(),
    };
    if let Err(e) = publish_event(&ws.kafka, &ev).await {
        tracing::error!(error = %e, "kafka publish failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("kafka: {e}")})),
        )
            .into_response();
    }
    if let Err(e) = shadow_apply(&ws.redis, &ev).await {
        tracing::warn!(error = %e, "shadow update failed");
    }
    (StatusCode::OK, Json(json!({"accepted": true}))).into_response()
}

fn extract_device_id(p: &WebhookPayload) -> Option<String> {
    let data = match &p.data {
        Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
        other => other.clone(),
    };
    data["deviceId"]
        .as_str()
        .or_else(|| data["device_id"].as_str())
        .map(str::to_string)
}

fn verify_signature(secret: &str, raw: &[u8], sig: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(raw);
    hex::encode(mac.finalize().into_bytes()) == sig
}

pub fn router(ws: WebhookState) -> axum::Router {
    axum::Router::new()
        .route("/webhook/tuya", axum::routing::post(receive))
        .with_state(ws)
}
```

- [x] **Step 5: 更新 lib.rs**

`e-cat/services/iot-access/src/lib.rs`（替换）：

```rust
pub mod adapter;
pub mod adapters;
pub mod api;
pub mod crypto;
pub mod events;
pub mod models;
pub mod mqtt;
pub mod oauth;
pub mod store;
pub mod webhook;
```

- [x] **Step 6: 运行测试确认通过**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test webhook`
Expected: 3 个测试全 PASS。

- [x] **Step 7: Commit**

```bash
git add e-cat/services/iot-access/
git commit -m "feat(access): Tuya webhook receive (signature verify + normalize) -> Kafka iot.events + Redis shadow"
```

---

### Task 7: 直连 MQTT 接入（EMQX 订阅 + 影子 + Kafka）

**Files:**
- Create: `e-cat/services/iot-access/src/mqtt.rs`（替换占位文件）
- Create: `e-cat/services/iot-access/tests/mqtt_payload.rs`

- [x] **Step 1: 写失败测试（直连上报 payload 解析，无 Docker）**

`e-cat/services/iot-access/tests/mqtt_payload.rs`:

```rust
use iot_access::mqtt::parse_payload;

#[test]
fn payload_with_code_value_ts() {
    let ev = parse_payload("dev-1", "t1", br#"{"code":"temp","value":23.5,"ts":1690000000000}"#)
        .unwrap();
    assert_eq!(ev.device_id, "dev-1");
    assert_eq!(ev.tenant_id, "t1");
    assert_eq!(ev.kind, "property");
    assert_eq!(ev.code, "temp");
    assert_eq!(ev.value, serde_json::json!(23.5));
    assert_eq!(ev.ts, 1690000000000);
}

#[test]
fn payload_without_ts_uses_now() {
    let ev = parse_payload("dev-1", "t1", br#"{"code":"switch","value":true}"#).unwrap();
    assert!(ev.ts > 1_700_000_000_000);
}

#[test]
fn bad_json_is_error() {
    assert!(parse_payload("dev-1", "t1", b"not json").is_err());
}
```

- [x] **Step 2: 运行确认失败**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test mqtt_payload`
Expected: 编译失败（`iot_access::mqtt` 无 `parse_payload`）。

- [x] **Step 3: 实现 mqtt.rs（按设备订阅 + 周期刷新 + 指令下发）**

`e-cat/services/iot-access/src/mqtt.rs`:

```rust
use crate::events::{publish_event, shadow_apply};
use crate::models::EventMessage;
use crate::store::Store;
use ecat_data_redis::RedisCache;
use ecat_mq_kafka::KafkaMq;
use ecat_mq_mqtt::MqttMq;
use futures_util::StreamExt;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 直连设备上报 topic 约定：iot/devices/{device_id}/properties。
pub fn report_topic(device_id: &str) -> String {
    format!("iot/devices/{device_id}/properties")
}

/// 指令下发 topic 约定：iot/devices/{device_id}/commands。
pub fn command_topic(device_id: &str) -> String {
    format!("iot/devices/{device_id}/commands")
}

/// 上报 payload：{"code","value","ts?"}；ts 缺省取当前毫秒。
pub fn parse_payload(
    platform_id: &str,
    tenant_id: &str,
    raw: &[u8],
) -> Result<EventMessage, String> {
    let v: serde_json::Value =
        serde_json::from_slice(raw).map_err(|e| format!("bad mqtt payload: {e}"))?;
    Ok(EventMessage {
        device_id: platform_id.to_string(),
        tenant_id: tenant_id.to_string(),
        kind: "property".into(),
        code: v["code"].as_str().unwrap_or("").to_string(),
        value: v["value"].clone(),
        ts: v["ts"]
            .as_i64()
            .unwrap_or_else(|| now_ms()),
    })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// 指令下发（api.rs 直连分支调用）。
pub async fn publish_command(
    mqtt: &MqttMq,
    device_id: &str,
    code: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    let payload = json!({ "code": code, "value": value });
    mqtt.publish(
        &command_topic(device_id),
        serde_json::to_vec(&payload).unwrap().as_slice(),
    )
    .await
    .map_err(|e| format!("mqtt publish: {e}"))
}

/// 后台任务：每 30s 扫描 vendor='direct' 设备，为新设备建立订阅。
/// 每个设备一条独立订阅（ecat-mq-mqtt 的 subscribe 自带独立连接），
/// 消息回调中设备 ID 由订阅上下文确定，杜绝 payload 伪造跨租户。
/// ponytail: 30s 轮询；设备量大后改为注册接口主动订阅。
pub async fn run(
    mqtt: Arc<MqttMq>,
    store: Arc<Store>,
    redis: Arc<RedisCache>,
    kafka: Arc<KafkaMq>,
) {
    let subs: Arc<Mutex<HashMap<String, ()>>> = Arc::new(Mutex::new(HashMap::new()));
    loop {
        let devices = match store.list_direct_devices().await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = %e, "list direct devices failed");
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
        };
        for (device_id, tenant_id) in devices {
            let already = {
                let s = subs.lock().await;
                s.contains_key(&device_id)
            };
            if already {
                continue;
            }
            let topic = report_topic(&device_id);
            let stream = match mqtt.subscribe(&topic).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(topic, error = %e, "mqtt subscribe failed");
                    continue;
                }
            };
            let mut stream =
                futures_util::stream::poll_fn(move |cx| stream.poll_recv(cx)).boxed();
            subs.lock().await.insert(device_id.clone(), ());
            let store = store.clone();
            let redis = redis.clone();
            let kafka = kafka.clone();
            let did = device_id.clone();
            tokio::spawn(async move {
                while let Some(Ok(raw)) = stream.next().await {
                    match parse_payload(&did, &tenant_id, &raw) {
                        Ok(ev) => {
                            if let Err(e) = publish_event(&kafka, &ev).await {
                                tracing::error!(error = %e, "kafka publish failed");
                            }
                            if let Err(e) = shadow_apply(&redis, &ev).await {
                                tracing::warn!(error = %e, "shadow update failed");
                            }
                        }
                        Err(e) => tracing::warn!(device = %did, error = %e, "drop mqtt payload"),
                    }
                }
            });
        }
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}
```

- [x] **Step 4: 运行测试确认通过**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test mqtt_payload`
Expected: 3 个测试全 PASS。

- [x] **Step 5: Commit**

```bash
git add e-cat/services/iot-access/
git commit -m "feat(access): direct MQTT ingestion (per-device EMQX subscribe -> shadow + Kafka)"
```

---

### Task 8: main.rs 装配（依赖注入 + 迁移 + 后台任务）

**Files:**
- Modify: `e-cat/services/iot-access/src/main.rs`（替换）
- Create: `e-cat/services/iot-access/migrations/0001_init.sql`（与 iot-device 同内容副本）
- Create: `e-cat/services/iot-access/migrations/0002_vendor_auth.sql`（与 iot-device 同内容副本）

- [ ] **Step 1: 复制迁移文件**

Run:
```bash
mkdir -p /home/wwwroot/iot-platform/e-cat/services/iot-access/migrations
cp /home/wwwroot/iot-platform/e-cat/services/iot-device/migrations/0001_init.sql \
   /home/wwwroot/iot-platform/e-cat/services/iot-device/migrations/0002_vendor_auth.sql \
   /home/wwwroot/iot-platform/e-cat/services/iot-access/migrations/
```
Expected: iot-access/migrations 下两个文件与 iot-device 完全一致（幂等，两服务各自建表）。

- [ ] **Step 2: 写 main.rs（全依赖装配）**

`e-cat/services/iot-access/src/main.rs`（替换）：

```rust
use axum::{
    Router,
    middleware::{self, Next},
    routing::get,
    extract::Request,
    http::Response,
};
use ecat_data_redis::RedisCache;
use ecat_data_sqlx::SqlxClient;
use ecat_mq_kafka::KafkaMq;
use ecat_mq_mqtt::MqttMq;
use iot_access::{
    api::{self, ApiState},
    oauth::{self, OauthState},
    store::Store,
    webhook::{self, WebhookState},
};
use std::sync::Arc;

async fn health() -> &'static str {
    "OK"
}

async fn migrate(db: &SqlxClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for file in ["migrations/0001_init.sql", "migrations/0002_vendor_auth.sql"] {
        let sql = std::fs::read_to_string(file)?;
        // execute 逐条执行: sqlx Any 驱动不启用 multi-statements
        for stmt in sql.split(';').filter(|s| !s.trim().is_empty()) {
            db.execute(stmt).await?;
        }
    }
    Ok(())
}

/// 把网关注入的 x-tenant-id 写入 request extensions，供受保护 handler 用。
async fn tenant_from_header(mut req: Request, next: Next) -> Response {
    if let Some(t) = req
        .headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
    {
        req.extensions_mut().insert(t.to_string());
    }
    next.run(req).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://iot:iot@localhost:3306/iot".into());
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".into());
    let mqtt_url =
        std::env::var("MQTT_URL").unwrap_or_else(|_| "tcp://localhost:1883".into());
    let enc_key = std::env::var("IOT_CRED_ENCRYPT_KEY")
        .unwrap_or_else(|_| "dev-only-encrypt-key-0123456789".into());
    let tuya_client_id = std::env::var("TUYA_CLIENT_ID")
        .unwrap_or_else(|_| "dev-tuya-client-id".into());
    // 测试/演示指向 mock：export TUYA_OPENAPI_BASE=http://127.0.0.1:18084
    // TUYA_CLIENT_SECRET 由 oauth::exchange_authorization_code 读取

    let db = SqlxClient::connect(&db_url).await?;
    migrate(&db).await?;
    let redis = Arc::new(RedisCache::connect(&redis_url).await?);
    let kafka = Arc::new(KafkaMq::connect(&kafka_brokers).await?);
    let mqtt = Arc::new(MqttMq::connect(&mqtt_url).await?);
    let store = Arc::new(Store::new(Arc::new(db), &enc_key));

    let callback_base = std::env::var("ACCESS_CALLBACK_BASE")
        .unwrap_or_else(|_| "http://localhost:8080/api/access/oauth/callback".into());

    let oauth_state = OauthState {
        store: store.clone(),
        tuya_client_id,
        callback_base,
    };
    let api_state = ApiState {
        store: store.clone(),
        kafka: kafka.clone(),
        redis: redis.clone(),
        mqtt: mqtt.clone(),
    };
    let webhook_state = WebhookState {
        store: store.clone(),
        kafka: kafka.clone(),
        redis: redis.clone(),
    };

    // 后台任务：直连 MQTT 订阅
    let (mqtt_run_mqtt, mqtt_run_store, mqtt_run_redis, mqtt_run_kafka) =
        (mqtt.clone(), store.clone(), redis.clone(), kafka.clone());
    tokio::spawn(async move {
        iot_access::mqtt::run(mqtt_run_mqtt, mqtt_run_store, mqtt_run_redis, mqtt_run_kafka)
            .await;
    });

    // 公开路由：涂鸦 webhook、OAuth 回调（浏览器跳回，无 JWT）
    let public = Router::new()
        .merge(webhook::router(webhook_state))
        .merge(oauth::router_public(oauth_state.clone()));
    // 受保护路由：需 x-tenant-id（网关 JWT 后注入）
    let protected = Router::new()
        .merge(oauth::router(oauth_state))
        .merge(api::router(api_state))
        .layer(middleware::from_fn(tenant_from_header));

    let router = Router::new()
        .route("/health", get(health))
        .nest("/api/access", public)
        .nest("/api/access", protected);

    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8082").router(router);
    let mut app = ecat::App::builder()
        .name("iot-access")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
```

- [ ] **Step 3: 编译验证 + 启动检查**

Run:
```bash
cd /home/wwwroot/iot-platform/e-cat && cargo check -p iot-access
cd /home/wwwroot/iot-platform/e-cat/services/iot-access && cargo run &
sleep 10 && curl -s http://localhost:8082/health
```
Expected: 编译通过；`/health` 返回 `OK`。若 Kafka/Redis/EMQX 未启动，先 `cd /home/wwwroot/iot-platform && docker compose up -d mysql redis emqx kafka`。

- [ ] **Step 4: Commit**

```bash
git add e-cat/services/iot-access/
git commit -m "feat(access): main assembly (mysql/redis/kafka/mqtt wiring + tenant header middleware)"
```

---

### Task 9: 涂鸦 OpenAPI mock 服务器 + 集成测试（全链路）

**Files:**
- Create: `e-cat/services/iot-access/tests/mock_tuya.rs`
- Create: `e-cat/services/iot-access/tests/tuya_flow.rs`
- Create: `e-cat/services/iot-access/tests/event_flow.rs`（需 docker，标注 `#[ignore]`）

- [ ] **Step 1: 写 mock 服务器（tests/mock_tuya.rs，模块被 tuya_flow.rs 引用）**

`e-cat/services/iot-access/tests/mock_tuya.rs`:

```rust
//! 涂鸦 OpenAPI mock：绑定 127.0.0.1:18084，校验 HMAC 签名后返回固定数据。
use axum::{Router, extract::State, http::HeaderMap, routing::get, Json};
use serde_json::{Value, json};
use std::sync::Arc;

pub const BASE: &str = "http://127.0.0.1:18084";
pub const CLIENT_ID: &str = "mock-client-id";
pub const CLIENT_SECRET: &str = "mock-client-secret";

#[derive(Clone)]
struct MockState {
    tokens: Arc<std::sync::Mutex<Vec<(String, String)>>>, // (code, access_token)
}

pub fn sign(secret: &str, client_id: &str, t: &str, token: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(client_id.as_bytes());
    mac.update(t.as_bytes());
    mac.update(token.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn check_sign(headers: &HeaderMap, token: &str) -> bool {
    let (Some(cid), Some(t), Some(sig)) = (
        headers.get("client_id").and_then(|v| v.to_str().ok()),
        headers.get("t").and_then(|v| v.to_str().ok()),
        headers.get("sign").and_then(|v| v.to_str().ok()),
    ) else {
        return false;
    };
    cid == CLIENT_ID && sign(CLIENT_SECRET, CLIENT_ID, t, token) == sig
}

async fn token(State(s): State<MockState>, headers: HeaderMap, query: axum::extract::Query<Value>) -> Json<Value> {
    // 签名校验（token 交换阶段 access_token 为空串）
    if !check_sign(&headers, "") {
        return Json(json!({"success": false, "code": "BAD_SIGN"}));
    }
    let grant = query.0.get("grant_type").and_then(Value::as_str).unwrap_or("");
    match grant {
        "authorization_code" => {
            let code = query.0.get("code").and_then(Value::as_str).unwrap_or("");
            let at = format!("mock-at-{code}");
            s.tokens.lock().unwrap().push((code.to_string(), at.clone()));
            Json(json!({
                "success": true,
                "result": {
                    "access_token": at,
                    "expire_time": 2592000,
                    "refresh_token": format!("mock-rt-{code}"),
                    "uid": "mock-uid-1"
                }
            }))
        }
        "refresh_token" => {
            let rt = query.0.get("refresh_token").and_then(Value::as_str).unwrap_or("");
            Json(json!({
                "success": true,
                "result": {
                    "access_token": format!("mock-at-refreshed-{rt}"),
                    "expire_time": 2592000,
                    "refresh_token": format!("mock-rt-new-{rt}"),
                    "uid": "mock-uid-1"
                }
            }))
        }
        _ => Json(json!({"success": false, "code": "BAD_GRANT"})),
    }
}

async fn devices(State(_): State<MockState>, headers: HeaderMap) -> Json<Value> {
    let at = headers.get("access_token").and_then(|v| v.to_str().ok()).unwrap_or("");
    if at.is_empty() || !check_sign(headers, at) {
        return Json(json!({"success": false, "code": "BAD_SIGN"}));
    }
    Json(json!({
        "success": true,
        "result": [
            {
                "id": "tuya-dev-1",
                "name": "mock-temp-sensor",
                "category": "temp_sensor",
                "online": true,
                "status": [{"code": "temp", "value": 23.5}]
            },
            {
                "id": "tuya-dev-2",
                "name": "mock-switch",
                "category": "switch",
                "online": false,
                "status": [{"code": "switch_1", "value": false}]
            }
        ]
    }))
}

async fn status(State(_): State<MockState>, headers: HeaderMap) -> Json<Value> {
    let at = headers.get("access_token").and_then(|v| v.to_str().ok()).unwrap_or("");
    if at.is_empty() || !check_sign(headers, at) {
        return Json(json!({"success": false, "code": "BAD_SIGN"}));
    }
    Json(json!({"success": true, "result": [{"code": "temp", "value": 25.0}]}))
}

async fn commands(State(_): State<MockState>, headers: HeaderMap, body: axum::body::Bytes) -> Json<Value> {
    let at = headers.get("access_token").and_then(|v| v.to_str().ok()).unwrap_or("");
    if at.is_empty() || !check_sign(headers, at) {
        return Json(json!({"success": false, "code": "BAD_SIGN"}));
    }
    let _ = body; // 记录即可，测试断言响应 success
    Json(json!({"success": true, "result": true}))
}

pub async fn spawn() -> tokio::task::JoinHandle<()> {
    let state = MockState { tokens: Arc::new(std::sync::Mutex::new(Vec::new())) };
    let router = Router::new()
        .route("/v1.0/token", get(token))
        .route("/v1.0/users/{uid}/devices", get(devices))
        .route("/v1.0/devices/{device_id}/status", get(status))
        .route("/v1.0/devices/{device_id}/commands", axum::routing::post(commands))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:18084").await.unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    })
}
```

- [ ] **Step 2: 写 tuya_flow.rs（适配器全流程：授权码 → 拉设备 → 属性 → 指令）**

`e-cat/services/iot-access/tests/tuya_flow.rs`:

```rust
mod mock_tuya;

use iot_access::adapters::tuya::TuyaAdapter;
use iot_access::adapter::VendorCreds;

fn creds(access_token: &str, refresh_token: &str) -> VendorCreds {
    VendorCreds {
        client_id: mock_tuya::CLIENT_ID.into(),
        client_secret: mock_tuya::CLIENT_SECRET.into(),
        uid: "mock-uid-1".into(),
        access_token: access_token.into(),
        refresh_token: refresh_token.into(),
        expires_at: 0, // 强制 maybe_refresh 走刷新路径
    }
}

#[tokio::test]
async fn full_oauth_and_device_flow() {
    mock_tuya::spawn().await;
    // 环境变量指向 mock（2024 edition 中 set_var 为 unsafe fn）
    unsafe {
        std::env::set_var("TUYA_OPENAPI_BASE", mock_tuya::BASE);
        std::env::set_var("TUYA_CLIENT_SECRET", mock_tuya::CLIENT_SECRET);
    }

    // 1. 授权码换 token（与 oauth::exchange_authorization_code 等价流程）
    let adapter = TuyaAdapter::new();
    let c = creds("mock-at-tuya-dev-1", "mock-rt-tuya-dev-1");
    let refreshed = adapter.refresh_token(&c).await.unwrap();
    assert!(refreshed.access_token.starts_with("mock-at-refreshed-"));

    // 2. 拉设备列表（签名正确 → mock 返回 2 台）
    let devs = adapter.list_devices(&refreshed).await.unwrap();
    assert_eq!(devs.len(), 2);
    assert_eq!(devs[0].vendor_id, "tuya-dev-1");
    assert_eq!(devs[0].properties[0].value, serde_json::json!(23.5));

    // 3. 单设备属性
    let props = adapter.get_properties(&refreshed, "tuya-dev-1").await.unwrap();
    assert_eq!(props[0].code, "temp");

    // 4. 指令下发
    adapter
        .send_command(&refreshed, "tuya-dev-1", "temp", serde_json::json!(26.0))
        .await
        .unwrap();
}

#[tokio::test]
async fn expired_token_auto_refresh_on_list() {
    mock_tuya::spawn().await;
    unsafe { std::env::set_var("TUYA_OPENAPI_BASE", mock_tuya::BASE) };
    let adapter = TuyaAdapter::new();
    // expires_at=0 → get 前先刷新，再带新 token 请求
    let c = creds("mock-at-expired", "mock-rt-expired");
    let devs = adapter.list_devices(&c).await.unwrap();
    assert_eq!(devs.len(), 2);
}
```

- [ ] **Step 3: 运行 tuya_flow（无需 docker）**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo test -p iot-access --test tuya_flow`
Expected: 2 个测试 PASS（mock 绑定 127.0.0.1:18084，全流程本地完成）。

- [ ] **Step 4: 写 event_flow.rs（Webhook → Kafka + Redis 影子，需 docker）**

`e-cat/services/iot-access/tests/event_flow.rs`:

```rust
//! 集成测试：需要 `docker compose up -d mysql redis emqx kafka`。
//! 运行：cargo test -p iot-access --test event_flow -- --ignored --nocapture
use axum::body::Body;
use ecat_data::Cache;
use ecat_data_redis::RedisCache;
use ecat_data_sqlx::SqlxClient;
use ecat_mq::{MessageQueue, MessageStream};
use ecat_mq_kafka::KafkaMq;
use iot_access::crypto::derive_key;
use iot_access::events::shadow_key;
use iot_access::store::Store;
use iot_access::webhook::{WebhookState, router};
use std::sync::Arc;
use tower::ServiceExt;

async fn setup() -> (Store, KafkaMq, RedisCache) {
    let db = Arc::new(
        SqlxClient::connect("mysql://iot:iot@localhost:3306/iot")
            .await
            .unwrap(),
    );
    // 幂等建表（与 iot-access 启动迁移一致）
    for file in ["migrations/0001_init.sql", "migrations/0002_vendor_auth.sql"] {
        let sql = std::fs::read_to_string(file).unwrap();
        for stmt in sql.split(';').filter(|s| !s.trim().is_empty()) {
            db.execute(stmt).await.unwrap();
        }
    }
    let store = Store::new(db, "test-encrypt-key-0123456789");
    // 种子：租户 + 涂鸦设备 + 凭据
    let db = store.db.clone();
    let _ = db
        .execute_with(
            "INSERT IGNORE INTO tenants (id, name) VALUES ('t1', 'mock-tenant')",
            &[],
        )
        .await;
    let _ = db
        .execute_with(
            "INSERT IGNORE INTO devices (id, tenant_id, name, vendor, status) \
             VALUES ('p1', 't1', 'mock-tuya-1', 'tuya', 'online')",
            &[],
        )
        .await;
    let _ = db
        .execute_with(
            "INSERT IGNORE INTO device_links (device_id, tenant_id, vendor, vendor_id, vendor_name, category) \
             VALUES ('p1', 't1', 'tuya', 'tuya-dev-1', 'mock-tuya-1', 'temp_sensor')",
            &[],
        )
        .await;
    let enc = iot_access::crypto::encrypt(&derive_key("test-encrypt-key-0123456789"), b"{\"client_secret\":\"mock-client-secret\"}")
        .unwrap();
    let _ = db
        .execute_with(
            "INSERT IGNORE INTO vendor_credentials (id, tenant_id, vendor, config_encrypted, status) \
             VALUES ('c1', 't1', 'tuya', ?, 'active')",
            &[serde_json::json!(enc)],
        )
        .await;
    let kafka = KafkaMq::connect("localhost:9092").await.unwrap();
    let redis = RedisCache::connect("redis://localhost:6379").await.unwrap();
    (store, kafka, redis)
}

#[tokio::test]
#[ignore = "requires docker compose up -d mysql redis kafka"]
async fn webhook_event_reaches_kafka_and_shadow() {
    let (store, kafka, redis) = setup().await;
    let ws = WebhookState {
        store: Arc::new(store),
        kafka: Arc::new(kafka),
        redis: Arc::new(redis),
    };
    let app = router(ws);
    // 先订阅 Kafka（group 从 latest 开始，先于发布才能收到事件），等 rebalance
    let mut stream = kafka.subscribe("iot.events").await.unwrap();
    let mut stream = futures_util::stream::poll_fn(move |cx| stream.poll_recv(cx)).boxed();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/webhook/tuya")
                .header("content-type", "application/json")
                // 签名：HMAC-SHA256(body, mock-client-secret)
                .header(
                    "x-tuya-signature",
                    tuya_sign(br#"{"type":"deviceData","bizCode":"report","data":{"deviceId":"tuya-dev-1","code":"temp","value":23.5,"ts":1690000000000}}"#),
                )
                .body(Body::from(
                    r#"{"type":"deviceData","bizCode":"report","data":{"deviceId":"tuya-dev-1","code":"temp","value":23.5,"ts":1690000000000}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    // Kafka 断言：topic iot.events 收到一条 property 事件（订阅已在 POST 之前建立）
    let raw = tokio::time::timeout(std::time::Duration::from_secs(10), stream.next())
        .await
        .expect("kafka event timeout")
        .expect("stream ended")
        .unwrap();
    let ev: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(ev["device_id"], "p1");
    assert_eq!(ev["kind"], "property");
    assert_eq!(ev["value"], 23.5);

    // 影子断言：shadow:p1 含属性 temp=23.5 且 online=true
    let shadow: serde_json::Value = serde_json::from_slice(
        &redis.get(&shadow_key("p1")).await.unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(shadow["online"], true);
    assert_eq!(shadow["properties"]["temp"], 23.5);
}

fn tuya_sign(raw: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(b"mock-client-secret").unwrap();
    mac.update(raw);
    hex::encode(mac.finalize().into_bytes())
}
```

注：测试运行目录须为 `e-cat/services/iot-access`（`migrations/` 相对路径）。`Cargo.toml` 的 `[dev-dependencies]` 需补 `futures-util`（或直接依赖主依赖，已含）。

- [ ] **Step 5: 运行集成测试（docker 已启动时）**

Run:
```bash
cd /home/wwwroot/iot-platform && docker compose up -d mysql redis emqx kafka
cd /home/wwwroot/iot-platform/e-cat/services/iot-access
cargo test -p iot-access --test event_flow -- --ignored --nocapture
```
Expected: 1 个测试 PASS（webhook → Kafka `iot.events` 事件 → Redis `shadow:p1` 影子全链路）。

^- [x] **Step 6: Commit**

```bash
git add e-cat/services/iot-access/tests/
git commit -m "test(access): Tuya OpenAPI mock + full-flow integration tests (oauth/list/command/event)"
```

---

### Task 10: 网关转发 `/api/access/*` + 冒烟扩展

**Files:**
- Modify: `e-cat/services/iot-gateway/Cargo.toml`（+reqwest）
- Create: `e-cat/services/iot-gateway/src/proxy.rs`
- Modify: `e-cat/services/iot-gateway/src/lib.rs`
- Modify: `e-cat/services/iot-gateway/src/main.rs`
- Modify: `e-cat/services/iot-gateway/src/api_version.rs`（豁免 webhook/callback 路径）
- Modify: `scripts/smoke.sh`

- [ ] **Step 1: 加 reqwest 依赖**

`e-cat/services/iot-gateway/Cargo.toml` 的 `[dependencies]` 加：

```toml
reqwest = { version = "0.12", features = ["json"] }
```

- [ ] **Step 2: 写 proxy.rs（转发 + 租户透传）**

`e-cat/services/iot-gateway/src/proxy.rs`:

```rust
use axum::{
    body::{Body, to_bytes},
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use ecat_auth::AuthClaims;
use std::sync::Arc;

/// iot-access 内部地址（生产走服务发现，P1 直连）。
const ACCESS_BASE: &str = "http://localhost:8082";

#[derive(Clone)]
pub struct ProxyState {
    pub client: reqwest::Client,
}

/// 受保护转发：从 extensions 取 AuthClaims.sub 作为租户，注入 x-tenant-id。
pub async fn access_proxy(
    State(ps): State<ProxyState>,
    Extension(claims): Extension<AuthClaims>,
    Path(rest): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    forward(&ps, &rest, &headers, &claims.sub, body).await
}

/// 公开转发（OAuth callback / 涂鸦 webhook）：不注入租户。
pub async fn access_proxy_open(
    State(ps): State<ProxyState>,
    Path(rest): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    forward(&ps, &rest, &headers, "", body).await
}

async fn forward(
    ps: &ProxyState,
    rest: &str,
    headers: &HeaderMap,
    tenant: &str,
    body: Body,
) -> Response {
    let raw = match to_bytes(body, 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                axum::Json(serde_json::json!({"error": "body too large"})),
            )
                .into_response()
        }
    };
    let url = format!("{ACCESS_BASE}/api/access/{rest}");
    let mut req = ps.client.post(&url);
    // 透传 content-type（有 body 时）
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        req = req.header(header::CONTENT_TYPE, ct);
    }
    if !tenant.is_empty() {
        req = req.header("x-tenant-id", tenant);
    }
    let resp = match req.body(raw.to_vec()).send().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({"error": format!("upstream: {e}")})),
            )
                .into_response()
        }
    };
    let status = resp.status();
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({"error": format!("upstream read: {e}")})),
            )
                .into_response()
        }
    };
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .unwrap()
}
```

注：P1 的 iot-access 端点全为 POST，转发统一用 POST；若后续加 GET 端点，按方法分支即可（`reqwest::Client` 的 `request(method, url)`）。

- [ ] **Step 3: 更新 lib.rs 与 api_version.rs**

`e-cat/services/iot-gateway/src/lib.rs`（替换）：

```rust
pub mod api_version;
pub mod auth_compat;
pub mod proxy;
pub mod scan;
```

`e-cat/services/iot-gateway/src/api_version.rs` 的豁免判断改为（原第 254 行）：

```rust
            if path == "/health"
                || path == "/metrics"
                || path.starts_with("/api/access/webhook")
                || path.starts_with("/api/access/oauth/callback")
            {
                return inner.call(req).await;
            }
```

注：涂鸦 Webhook 与浏览器 OAuth 回调无法携带 `x-api-version` header，必须豁免；其余 `/api/access/*` 仍要求版本 header。

- [ ] **Step 4: 更新 main.rs（挂载代理路由）**

`e-cat/services/iot-gateway/src/main.rs`（替换）：

```rust
use axum::{Router, routing::{get, post}};
use ecat_auth::JwtAuthLayer;
use ecat_health::HealthRegistry;
use iot_gateway::{
    api_version::ApiVersionLayer,
    auth_compat::JwtAuthCompat,
    proxy::{ProxyState, access_proxy, access_proxy_open},
    scan::ScanLayer,
};
use std::sync::Arc;

async fn submit() -> &'static str {
    "ok"
}

async fn devices() -> &'static str {
    "admin-devices"
}

async fn me() -> &'static str {
    "client-me"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-key-0123456789abcdefghijklmn".into());

    let proxy_state = ProxyState {
        client: reqwest::Client::new(),
    };

    // /api/access/* 公开路径：OAuth 回调、涂鸦 Webhook（无 JWT，浏览器/厂商服务器直连）
    let access_public = Router::new()
        .route("/oauth/callback", get(access_proxy_open))
        .route("/webhook/tuya", post(access_proxy_open))
        .with_state(proxy_state.clone());
    // /api/access/* 受保护路径：JWT 校验后透传租户（AuthClaims.sub → x-tenant-id）
    let access_admin = Router::new()
        .route("/oauth/authorize-url", post(access_proxy))
        .route("/vendors/{vendor}/import", post(access_proxy))
        .route("/devices/{device_id}/command", post(access_proxy))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?)
        .with_state(proxy_state);

    let admin_api = Router::new()
        .route("/devices", get(devices))
        .layer(JwtAuthCompat::new(&secret, &["sub", "role"])?);
    let client_api = Router::new()
        .route("/me", get(me))
        .layer(JwtAuthCompat::new(&secret, &["sub"])?);

    let router = Router::new()
        .merge(HealthRegistry::new().into_router())
        .route("/api/ping", get(|| async { "pong" }))
        .route("/api/submit", post(submit))
        .nest("/api/access", access_public)
        .nest("/api/access", access_admin)
        .nest("/api", admin_api)
        .nest("/admin", client_api)
        .layer(ApiVersionLayer)
        .layer(ScanLayer::new());

    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
    let mut app = ecat::App::builder()
        .name("iot-gateway")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
```

注意：`Extension<AuthClaims>` 提取器要求 JwtAuthCompat 把 claims 放入 extensions——ecat-auth 的 JwtAuthService 已做（`req.extensions().insert(AuthClaims)`，见 ecat-auth/src/claims.rs 测试用例）。若挂载顺序导致 claims 未注入，把 `.layer(JwtAuthCompat...)` 改为在 `with_state` 之后、nest 之前，或按编译报错调整。

- [ ] **Step 5: 编译验证**

Run: `cd /home/wwwroot/iot-platform/e-cat && cargo check -p iot-gateway`
Expected: 编译通过（JWT_SECRET 由 AuthClaims 提取器隐含要求 ≥32 字节，dev 值满足）。

- [ ] **Step 6: 扩展冒烟脚本（追加到 `scripts/smoke.sh` 末尾、`echo "----"` 之前）**

```bash
# 8. access 服务健康（直连）
ACCESS=${ACCESS:-http://localhost:8082}
body=$(curl -s "$ACCESS/health")
check "access /health 200" "OK" "$body"

# 9. 网关转发 /api/access/oauth/authorize-url（带 JWT + 版本 header → 200）
code=$(curl -s -o /tmp/access_authorize.json -w "%{http_code}" -H "x-api-version: v1" \
  -H "authorization: Bearer $token" -H "content-type: application/json" \
  -X POST -d '{"vendor":"tuya"}' "$GATEWAY/api/access/oauth/authorize-url")
check "gateway -> access authorize-url 200" 200 "$code"
grep -q "openapi.tuyacn.com/oauth2/auth" /tmp/access_authorize.json && pass=$((pass+1)) && echo "PASS: authorize url contains tuya auth" || { fail=$((fail+1)); echo "FAIL: authorize url"; }

# 10. 涂鸦 Webhook 事件（经网关，无 JWT/版本 header）→ Kafka iot.events + Redis 影子
# 前置：event_flow 集成测试已跑过（种子设备 p1 与凭据），此处直接复用
sig=$(python3 - "$TUYA_WEBHOOK_SECRET" <<'PY'
import sys, hmac, hashlib
secret = sys.argv[1].encode()
body = b'{"type":"deviceData","bizCode":"report","data":{"deviceId":"tuya-dev-1","code":"temp","value":23.5,"ts":1690000000000}}'
print(hmac.new(secret, body, hashlib.sha256).hexdigest())
PY
)
code=$(curl -s -o /dev/null -w "%{http_code}" -H "content-type: application/json" \
  -H "x-tuya-signature: $sig" \
  -X POST -d '{"type":"deviceData","bizCode":"report","data":{"deviceId":"tuya-dev-1","code":"temp","value":23.5,"ts":1690000000000}}' \
  "$GATEWAY/api/access/webhook/tuya")
check "gateway -> access webhook accepted" 200 "$code"

# 11. Kafka 断言：iot.events 收到事件
ev=$(docker exec iot-platform-kafka-1 kafka-console-consumer.sh --bootstrap-server localhost:9092 \
  --topic iot.events --from-beginning --max-messages 1 --timeout-ms 8000 2>/dev/null | head -1)
echo "$ev" | grep -q '"kind":"property"' && pass=$((pass+1)) && echo "PASS: kafka iot.events has property event" || { fail=$((fail+1)); echo "FAIL: kafka event (got: $ev)"; }

# 12. Redis 影子断言：shadow:p1 属性 temp=23.5
shadow=$(docker exec iot-platform-redis-1 redis-cli GET shadow:p1)
echo "$shadow" | grep -q '"temp":23.5' && pass=$((pass+1)) && echo "PASS: redis shadow:p1 has temp=23.5" || { fail=$((fail+1)); echo "FAIL: shadow (got: $shadow)"; }
```

脚本头部加变量：

```bash
TUYA_WEBHOOK_SECRET=${TUYA_WEBHOOK_SECRET:-mock-client-secret}
```

- [ ] **Step 7: 全链路运行冒烟**

Run:
```bash
cd /home/wwwroot/iot-platform && docker compose up -d mysql redis emqx kafka
cd /home/wwwroot/iot-platform/e-cat/services/iot-access && cargo run &
cd /home/wwwroot/iot-platform/e-cat/services/iot-gateway && cargo run &
cd /home/wwwroot/iot-platform/e-cat/services/iot-device && cargo run &
sleep 15
# 先跑一次集成测试种子数据（幂等，见 Task 9 Step 5）
cd /home/wwwroot/iot-platform/e-cat/services/iot-access && cargo test -p iot-access --test event_flow -- --ignored
cd /home/wwwroot/iot-platform && ./scripts/smoke.sh
kill %1 %2 %3 2>/dev/null; true
```
Expected: 全部 PASS，`smoke: 12 passed, 0 failed`（P0 的 7 项 + P1 新增 5 项）。

- [ ] **Step 8: Commit**

```bash
git add e-cat/services/iot-gateway/ scripts/smoke.sh
git commit -m "feat(gateway): /api/access proxy (JWT -> x-tenant-id) + P1 smoke (webhook/kafka/shadow)"
```

---

## Self-Review 结果

- **T1.1-T1.8 覆盖**：T1.1 骨架（Task 1）；T1.2 适配器 Trait（Task 2，`adapter.rs` 四方法齐备）；T1.3 凭据表 + AES + OAuth 流程 API（Task 3+4，`0002_vendor_auth.sql`、aes-gcm 加密、密钥环境变量 `IOT_CRED_ENCRYPT_KEY` 注入）；T1.4 涂鸦适配器（Task 5，授权码 → 刷新 → 设备拉取 → 统一物模型映射 + import 端点）；T1.5 Webhook → Kafka（Task 6，topic `iot.events`）；T1.6 直连 MQTT（Task 7，EMQX 订阅 + Redis 影子 + Kafka）；T1.7 指令下发（Task 5/7/10，`/devices/{id}/command` → 涂鸦 OpenAPI 或 MQTT）；T1.8 测试（Task 9 mock 服务器 + 集成测试 + Task 10 冒烟扩展）。
- **占位符扫描**：无 TBD/TODO/"类似 Task N"；Task 5 签名测试的 hex 断言值标注了需用 python3 实测替换（确定性算法，非占位）；Task 1/2/5 的占位 `.rs` 文件均为"见 Task N"注释并由后续 Task 覆盖。`ApiState` 中的 `reqwest`/`ecat_mq` 等依赖均已列入 Cargo.toml。
- **一致性核对**：`DeviceRecord {id, vendor_id, name, category, online, properties}` 在 tuya.rs `to_record`、mock_tuya.rs、tests 中一致；`EventMessage {device_id, tenant_id, kind, code, value, ts}` 在 models.rs、webhook.rs `normalize_event`、mqtt.rs `parse_payload`、events.rs `publish_event`、Kafka JSON、影子 JSON（`{"online","properties","ts"}`）中一致；`VendorCreds {client_id, client_secret, uid, access_token, refresh_token, expires_at}` 在 adapter.rs、oauth.rs、tuya.rs、mock 中一致；topic 常量 `iot.events` 唯一真源 `events.rs::TOPIC_EVENTS`，MQTT topic 约定 `iot/devices/{id}/properties|commands` 在 mqtt.rs 与 mock/测试中一致；租户透传 `x-tenant-id` 在 gateway proxy.rs 注入、iot-access `tenant_from_header` 消费，两处一致。

