use crate::adapter::{AdapterError, VendorAdapter, VendorCreds, utc_now};
use crate::models::{DeviceRecord, PropertyValue};
use async_trait::async_trait;
use serde_json::{Value, json};

fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b))
}

fn hmac(key: &[u8], msg: &str) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(msg.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

fn signing_key(sk: &str, date_short: &str, region: &str) -> Vec<u8> {
    let k_date = hmac(format!("AWS4{sk}").as_bytes(), date_short);
    let k_region = hmac(&k_date, region);
    let k_service = hmac(&k_region, "iot");
    hmac(&k_service, "aws4_request")
}

/// AWS SigV4 签名（服务 iot，host + x-amz-date 双签名头，GET 用 UNSIGNED-PAYLOAD）。
fn sign_request(
    ak: &str,
    sk: &str,
    region: &str,
    method: &str,
    path: &str,
    host: &str,
    amz_date: &str,
    body: &[u8],
) -> String {
    let date_short = &amz_date[..8];
    let scope = format!("{date_short}/{region}/iot/aws4_request");
    let canonical = format!(
        "{method}\n{path}\n\nhost:{host}\nx-amz-date:{amz_date}\n\nhost;x-amz-date\n{}",
        if method == "GET" {
            "UNSIGNED-PAYLOAD".to_string()
        } else {
            sha256_hex(body)
        }
    );
    let sts = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical.as_bytes())
    );
    let sig = hex::encode(hmac(&signing_key(sk, date_short, region), &sts));
    format!(
        "AWS4-HMAC-SHA256 Credential={ak}/{scope}, SignedHeaders=host;x-amz-date, Signature={sig}"
    )
}

/// 从 base URL 提取 Host（含非默认端口），与 reqwest 自动发送的 Host 头一致。
fn host_of(base: &str) -> String {
    base.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .to_string()
}

/// AWS IoT 适配器（AK/SK SigV4 签名，无 OAuth；client_id=AK, client_secret=SK, uid=region）。
pub struct AwsAdapter {
    http: reqwest::Client,
}

impl AwsAdapter {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("http client"),
        }
    }

    fn base(&self, creds: &VendorCreds) -> String {
        std::env::var("AWS_IOT_BASE")
            .unwrap_or_else(|_| format!("https://iot.{}.amazonaws.com", creds.uid))
    }

    async fn request(
        &self,
        creds: &VendorCreds,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, AdapterError> {
        let base = self.base(creds);
        let host = host_of(&base);
        let amz_date = utc_now();
        let raw = body.map(|b| serde_json::to_vec(b).unwrap_or_default()).unwrap_or_default();
        let auth = sign_request(
            &creds.client_id,
            &creds.client_secret,
            &creds.uid,
            method,
            path,
            &host,
            &amz_date,
            &raw,
        );
        let url = format!("{base}{path}");
        let mut rb = match method {
            "GET" => self.http.get(&url),
            "POST" => self.http.post(&url),
            other => return Err(AdapterError::Internal(format!("unsupported method {other}"))),
        };
        rb = rb
            .header("Host", &host)
            .header("X-Amz-Date", &amz_date)
            .header("Authorization", auth);
        let resp = if body.is_some() {
            rb.body(raw).send().await
        } else {
            rb.send().await
        }
        .map_err(|e| AdapterError::Vendor(format!("request: {e}")))?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AdapterError::Vendor(format!("aws error {text}")));
        }
        let bytes = resp.bytes().await.map_err(|e| AdapterError::Vendor(format!("read: {e}")))?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes)
            .map_err(|e| AdapterError::Vendor(format!("parse: {e}")))
    }
}

impl Default for AwsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VendorAdapter for AwsAdapter {
    /// GET /things：AWS 列表无在线状态，online 置 false（在线经事件流维护）。
    async fn list_devices(&self, creds: &VendorCreds) -> Result<Vec<DeviceRecord>, AdapterError> {
        let body = self.request(creds, "GET", "/things", None).await?;
        Ok(body["things"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|d| {
                let name = d["thingName"].as_str().unwrap_or("").to_string();
                DeviceRecord {
                    id: String::new(),
                    vendor_id: name.clone(),
                    name: d["attributes"]["name"].as_str().unwrap_or(&name).to_string(),
                    category: String::new(),
                    online: false,
                    properties: vec![],
                }
            })
            .collect())
    }

    /// GET /things/{name}/shadow：reported 优先，缺失回退 desired。
    async fn get_properties(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
    ) -> Result<Vec<PropertyValue>, AdapterError> {
        let body = self
            .request(creds, "GET", &format!("/things/{vendor_id}/shadow"), None)
            .await?;
        let state = &body["state"];
        // 先 desired 后 reported：reported 覆盖同名键（影子以设备上报为准）
        let mut merged: std::collections::HashMap<String, Value> = state["desired"]
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        if let Some(reported) = state["reported"].as_object() {
            for (k, v) in reported {
                merged.insert(k.clone(), v.clone());
            }
        }
        Ok(merged
            .into_iter()
            .map(|(code, value)| PropertyValue { code, value })
            .collect())
    }

    /// POST /things/{name}/shadow：以 desired 状态下发指令（AWS 设备指令经影子/消息路由）。
    async fn send_command(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
        code: &str,
        value: serde_json::Value,
    ) -> Result<(), AdapterError> {
        let path = format!("/things/{vendor_id}/shadow");
        let body = json!({ "state": { "desired": { code: value } } });
        self.request(creds, "POST", &path, Some(&body)).await?;
        Ok(())
    }

    /// AWS 事件经 IoT Rules / SNS 转发配置（控制台），注册即完成。
    async fn subscribe_events(&self, _creds: &VendorCreds) -> Result<(), AdapterError> {
        Ok(())
    }
}
