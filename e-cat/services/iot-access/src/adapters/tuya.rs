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
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("http client"),
        }
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
        match self.get_once(creds, path).await {
            Ok(v) => Ok(v),
            Err(AdapterError::Vendor(ref msg))
                if msg.contains("token")
                    || msg.contains("ACCESS_TOKEN_SESSION_INVALID")
                    || msg.contains("21000002") =>
            {
                let refreshed = self.refresh_token(creds).await?;
                self.get_once(&refreshed, path).await
            }
            Err(other) => Err(other),
        }
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

    /// grant_type=refresh_token 换新；返回新凭据，由调用方决定是否持久化。
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
