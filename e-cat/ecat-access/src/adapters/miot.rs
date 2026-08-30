use crate::adapter::{AdapterError, VendorAdapter, VendorCreds};
use crate::models::{DeviceRecord, PropertyValue};
use async_trait::async_trait;
use serde_json::{Value, json};

/// 小米签名：HMAC-SHA256(client_id + t + access_token, app_secret)，hex 大写。
pub fn sign(client_id: &str, t: &str, access_token: &str, secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(client_id.as_bytes());
    mac.update(t.as_bytes());
    mac.update(access_token.as_bytes());
    hex::encode(mac.finalize().into_bytes()).to_uppercase()
}

fn now_ms() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// 小米 MIoT 云对云适配器（https://api.open.home.miot.com）。
/// access_token 不随请求头发送，只参与 sign；请求头为 client_id / t / sign。
pub struct MiAdapter {
    http: reqwest::Client,
}

impl MiAdapter {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("http client"),
        }
    }

    fn base(&self) -> String {
        std::env::var("MIOT_OPENAPI_BASE")
            .unwrap_or_else(|_| "https://api.open.home.miot.com".into())
    }

    /// 带签名的 GET；token 无效（code 400006/400004 或 message 含 token）时刷新重试一次。
    async fn get(&self, creds: &VendorCreds, path: &str) -> Result<Value, AdapterError> {
        match self.get_once(creds, path).await {
            Ok(v) => Ok(v),
            Err(AdapterError::Vendor(ref msg)) if is_token_error(msg) => {
                let refreshed = self.refresh_token(creds).await?;
                self.get_once(&refreshed, path).await
            }
            Err(other) => Err(other),
        }
    }

    async fn get_once(&self, creds: &VendorCreds, path: &str) -> Result<Value, AdapterError> {
        let t = now_ms();
        let sign = sign(&creds.client_id, &t, &creds.access_token, &creds.client_secret);
        let url = format!("{}{path}", self.base());
        let resp = self
            .http
            .get(&url)
            .header("client_id", &creds.client_id)
            .header("t", &t)
            .header("sign", sign)
            .send()
            .await
            .map_err(|e| AdapterError::Vendor(format!("request: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AdapterError::Vendor(format!("parse: {e}")))?;
        if body["code"].as_i64() != Some(0) {
            return Err(AdapterError::Vendor(format!("miot error: {body}")));
        }
        Ok(body["data"].clone())
    }

    /// grant_type=refresh_token 换新；返回新凭据，由调用方决定是否持久化。
    pub async fn refresh_token(&self, creds: &VendorCreds) -> Result<VendorCreds, AdapterError> {
        let t = now_ms();
        let sign = sign(&creds.client_id, &t, "", &creds.client_secret);
        let url = format!(
            "{}/oauth/token?grant_type=refresh_token&refresh_token={}",
            self.base(),
            creds.refresh_token
        );
        let resp = self
            .http
            .get(&url)
            .header("client_id", &creds.client_id)
            .header("t", &t)
            .header("sign", sign)
            .send()
            .await
            .map_err(|e| AdapterError::Refresh(format!("request: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AdapterError::Refresh(format!("parse: {e}")))?;
        if body["code"].as_i64() != Some(0) {
            return Err(AdapterError::Refresh(format!("miot refresh error: {body}")));
        }
        let d = &body["data"];
        Ok(VendorCreds {
            client_id: creds.client_id.clone(),
            client_secret: creds.client_secret.clone(),
            uid: creds.uid.clone(),
            access_token: d["access_token"].as_str().unwrap_or("").to_string(),
            refresh_token: d["refresh_token"].as_str().unwrap_or("").to_string(),
            expires_at: now_secs() + d["expires_in"].as_i64().unwrap_or(2592000),
        })
    }

    /// access_token 过期（expires_at 距今 < 60s）则刷新并返回新凭据。
    async fn maybe_refresh(&self, creds: &VendorCreds) -> Result<VendorCreds, AdapterError> {
        if creds.expires_at == 0 || creds.expires_at - now_secs() < 60 {
            self.refresh_token(creds).await
        } else {
            Ok(creds.clone())
        }
    }
}

fn is_token_error(msg: &str) -> bool {
    msg.contains("400006") || msg.contains("400004") || msg.contains("token")
}

/// 用授权码换 token（OAuth 授权码接入；签名时 access_token 约定为空串）。
pub async fn exchange_authorization_code(
    code: &str,
    client_id: &str,
) -> Result<VendorCreds, String> {
    let base = std::env::var("MIOT_OPENAPI_BASE")
        .unwrap_or_else(|_| "https://api.open.home.miot.com".into());
    let client_secret = std::env::var("MIOT_CLIENT_SECRET")
        .map_err(|_| "MIOT_CLIENT_SECRET not set".to_string())?;
    let t = now_ms();
    let sign = sign(client_id, &t, "", &client_secret);
    let url = format!(
        "{base}/oauth/token?grant_type=authorization_code&code={code}&client_id={client_id}"
    );
    let resp = reqwest::Client::new()
        .get(&url)
        .header("client_id", client_id)
        .header("t", &t)
        .header("sign", sign)
        .send()
        .await
        .map_err(|e| format!("miot token request: {e}"))?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("miot token parse: {e}"))?;
    if body["code"].as_i64() != Some(0) {
        return Err(format!("miot token error: {body}"));
    }
    let d = &body["data"];
    Ok(VendorCreds {
        client_id: client_id.to_string(),
        client_secret,
        uid: String::new(),
        access_token: d["access_token"].as_str().unwrap_or("").to_string(),
        refresh_token: d["refresh_token"].as_str().unwrap_or("").to_string(),
        expires_at: now_secs() + d["expires_in"].as_i64().unwrap_or(2592000),
    })
}

impl Default for MiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VendorAdapter for MiAdapter {
    /// GET /v1/device/list
    async fn list_devices(&self, creds: &VendorCreds) -> Result<Vec<DeviceRecord>, AdapterError> {
        let data = self.get(creds, "/v1/device/list").await?;
        Ok(data["devices"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|d| DeviceRecord {
                id: String::new(), // 由 store.upsert_device 回填
                vendor_id: d["did"].as_str().unwrap_or("").to_string(),
                name: d["name"].as_str().unwrap_or("").to_string(),
                category: d["model"].as_str().unwrap_or("").to_string(),
                online: d["online"].as_bool().unwrap_or(false),
                properties: vec![],
            })
            .collect())
    }

    /// GET /v1/device/status?did={vendor_id}
    async fn get_properties(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
    ) -> Result<Vec<PropertyValue>, AdapterError> {
        let data = self.get(creds, &format!("/v1/device/status?did={vendor_id}")).await?;
        Ok(data["status"]
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

    /// POST /v1/device/command
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
        let url = format!("{}/v1/device/command", self.base());
        let resp = self
            .http
            .post(&url)
            .header("client_id", &refreshed.client_id)
            .header("t", &t)
            .header("sign", sign)
            .json(&json!({ "did": vendor_id, "code": code, "value": value }))
            .send()
            .await
            .map_err(|e| AdapterError::Vendor(format!("command request: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AdapterError::Vendor(format!("command parse: {e}")))?;
        if body["code"].as_i64() != Some(0) {
            return Err(AdapterError::Vendor(format!("command error: {body}")));
        }
        Ok(())
    }

    /// 小米事件经开放平台控制台配置的回调 URL 推送（同涂鸦语义），返回 Ok。
    async fn subscribe_events(&self, _creds: &VendorCreds) -> Result<(), AdapterError> {
        Ok(())
    }
}
