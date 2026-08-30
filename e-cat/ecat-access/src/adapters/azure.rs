use crate::adapter::{AdapterError, VendorAdapter, VendorCreds};
use crate::models::{DeviceRecord, PropertyValue};
use async_trait::async_trait;
use serde_json::{Value, json};

fn b64_encode(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn b64_decode(s: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .unwrap_or_default()
}

/// 生成 IoT Hub 服务级 SAS 令牌：sig = base64(HMAC-SHA256(decoded_key, "sr=&skn=&se="))。
/// `ponytail: 统一用 hub 级 sr + iothubowner 策略（列表/孪生/方法一次授权），
/// 需要最小权限时按 %2Fdevices%2F{deviceId} 逐设备签发`
pub fn sas_token(host: &str, key: &str, expiry: i64) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let policy = "iothubowner";
    let sts = format!("sr={host}&skn={policy}&se={expiry}");
    let mut mac = HmacSha256::new_from_slice(&b64_decode(key)).expect("hmac key");
    mac.update(sts.as_bytes());
    let sig = b64_encode(&mac.finalize().into_bytes());
    format!("SharedAccessSignature sr={host}&sig={sig}&se={expiry}&skn={policy}")
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Azure IoT Hub 适配器（SAS 令牌，无 OAuth；client_id=IoT Hub host，client_secret=base64 共享密钥）。
pub struct AzureAdapter {
    http: reqwest::Client,
}

impl AzureAdapter {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("http client"),
        }
    }

    fn base(&self, creds: &VendorCreds) -> String {
        std::env::var("AZURE_IOTHUB_BASE")
            .unwrap_or_else(|_| format!("https://{}", creds.client_id))
    }

    async fn get(&self, creds: &VendorCreds, path: &str) -> Result<Value, AdapterError> {
        let host = host_of(&self.base(creds));
        let auth = sas_token(&host, &creds.client_secret, now_secs() + 3600);
        let url = format!("{}{path}", self.base(creds));
        let resp = self
            .http
            .get(&url)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| AdapterError::Vendor(format!("request: {e}")))?;
        parse_response(resp, "azure").await
    }

    async fn post(
        &self,
        creds: &VendorCreds,
        path: &str,
        body: &Value,
    ) -> Result<Value, AdapterError> {
        let host = host_of(&self.base(creds));
        let auth = sas_token(&host, &creds.client_secret, now_secs() + 3600);
        let url = format!("{}{path}", self.base(creds));
        let resp = self
            .http
            .post(&url)
            .header("Authorization", auth)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| AdapterError::Vendor(format!("command request: {e}")))?;
        parse_response(resp, "azure").await
    }
}

async fn parse_response(
    resp: reqwest::Response,
    vendor: &str,
) -> Result<Value, AdapterError> {
    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(AdapterError::Vendor(format!("{vendor} error {text}")));
    }
    resp.json()
        .await
        .map_err(|e| AdapterError::Vendor(format!("parse: {e}")))
}

fn host_of(base: &str) -> String {
    base.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .to_string()
}

impl Default for AzureAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VendorAdapter for AzureAdapter {
    /// GET /devices?api-version=2020-10-01（注册表列表，SAS 需 hub 级权限）
    async fn list_devices(&self, creds: &VendorCreds) -> Result<Vec<DeviceRecord>, AdapterError> {
        let body = self
            .get(creds, "/devices?api-version=2020-10-01")
            .await?;
        Ok(body
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|d| DeviceRecord {
                id: String::new(),
                vendor_id: d["deviceId"].as_str().unwrap_or("").to_string(),
                name: d["deviceId"].as_str().unwrap_or("").to_string(),
                category: String::new(),
                online: d["connectionState"].as_str() == Some("Connected"),
                properties: vec![],
            })
            .collect())
    }

    /// GET /twins/{id}：reported 覆盖 desired（孪生以设备上报为准）
    async fn get_properties(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
    ) -> Result<Vec<PropertyValue>, AdapterError> {
        let path = format!("/twins/{vendor_id}?api-version=2020-10-01");
        let body = self.get(creds, &path).await?;
        let props = &body["properties"];
        let mut merged: std::collections::HashMap<String, Value> = props["desired"]
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        if let Some(reported) = props["reported"].as_object() {
            for (k, v) in reported {
                merged.insert(k.clone(), v.clone());
            }
        }
        Ok(merged
            .into_iter()
            .map(|(code, value)| PropertyValue { code, value })
            .collect())
    }

    /// POST /devices/{id}/methods：直接方法调用下发指令
    async fn send_command(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
        code: &str,
        value: serde_json::Value,
    ) -> Result<(), AdapterError> {
        let path = format!("/devices/{vendor_id}/methods?api-version=2020-10-01");
        let body = json!({ "methodName": code, "payload": value });
        self.post(creds, &path, &body).await?;
        Ok(())
    }

    /// Azure 事件经 IoT Hub 路由/Event Grid 转发（控制台配置），注册即完成。
    async fn subscribe_events(&self, _creds: &VendorCreds) -> Result<(), AdapterError> {
        Ok(())
    }
}
