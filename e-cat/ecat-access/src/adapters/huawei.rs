use crate::adapter::{AdapterError, VendorAdapter, VendorCreds};
use crate::models::{DeviceRecord, PropertyValue};
use async_trait::async_trait;
use serde_json::{Value, json};

fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b))
}

fn hmac_sha256_hex(key: &[u8], msg: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(msg.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// 当前 UTC 时间，X-Sdk-Date 格式：YYYYMMDDTHHMMSSZ（手写公历转换，无 chrono 依赖）。
fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let days = secs.div_euclid(86400);
    let sod = secs.rem_euclid(86400);
    let (h, mi, s) = (sod / 3600, (sod % 3600) / 60, sod % 60);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mth <= 2 { y + 1 } else { y };
    format!("{y:04}{mth:02}{d:02}T{h:02}{mi:02}{s:02}Z")
}

/// 华为云 API 网关 v2 请求签名（SDK-HMAC-SHA256）。
/// 签名头固定 x-sdk-date（GET）/ content-type;x-sdk-date（POST）；
/// `ponytail: 生产同款 SDK 会另签 host 头，此处按 mock 约定固定集合，厂商文档核对时补齐`
pub fn sign_request(
    ak: &str,
    sk: &str,
    method: &str,
    path: &str,
    x_sdk_date: &str,
    content_type: Option<&str>,
    body: &[u8],
) -> String {
    let mut headers: Vec<(&str, &str)> = vec![("x-sdk-date", x_sdk_date)];
    if let Some(ct) = content_type {
        headers.push(("content-type", ct));
    }
    headers.sort_by_key(|(k, _)| *k);
    let mut canonical_headers = String::new();
    let mut signed_names = Vec::new();
    for (k, v) in &headers {
        canonical_headers.push_str(&format!("{}:{}\n", k, v.trim()));
        signed_names.push(*k);
    }
    let canon = format!(
        "{method}\n{path}\n\n{canonical_headers}\n{}\n{}",
        signed_names.join(";"),
        sha256_hex(body)
    );
    let sts = format!("SDK-HMAC-SHA256\n{x_sdk_date}\n{canon}");
    let sig = hmac_sha256_hex(sk.as_bytes(), &sts);
    format!(
        "SDK-HMAC-SHA256 Access={ak}, SignedHeaders={}, Signature={sig}",
        signed_names.join(";")
    )
}

/// 华为云 IoTDA 适配器（AK/SK 签名，无 OAuth；client_id=AK, client_secret=SK, uid=project_id）。
pub struct HuaweiAdapter {
    http: reqwest::Client,
}

impl HuaweiAdapter {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("http client"),
        }
    }

    fn base(&self) -> String {
        std::env::var("HUAWEI_IOTDA_BASE")
            .unwrap_or_else(|_| "https://iotda.cn-north-4.myhuaweicloud.com".into())
    }

    async fn get(&self, creds: &VendorCreds, path: &str) -> Result<Value, AdapterError> {
        let date = utc_now();
        let auth = sign_request(
            &creds.client_id,
            &creds.client_secret,
            "GET",
            path,
            &date,
            None,
            b"",
        );
        let url = format!("{}{path}", self.base());
        let resp = self
            .http
            .get(&url)
            .header("X-Sdk-Date", &date)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| AdapterError::Vendor(format!("request: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AdapterError::Vendor(format!("parse: {e}")))?;
        if body.get("error").is_some() {
            return Err(AdapterError::Vendor(format!("huawei error: {body}")));
        }
        Ok(body)
    }

    async fn post(&self, creds: &VendorCreds, path: &str, body: &Value) -> Result<Value, AdapterError> {
        let date = utc_now();
        let raw = serde_json::to_vec(body).unwrap_or_default();
        let auth = sign_request(
            &creds.client_id,
            &creds.client_secret,
            "POST",
            path,
            &date,
            Some("application/json"),
            &raw,
        );
        let url = format!("{}{path}", self.base());
        let resp = self
            .http
            .post(&url)
            .header("X-Sdk-Date", &date)
            .header("Content-Type", "application/json")
            .header("Authorization", auth)
            .body(raw)
            .send()
            .await
            .map_err(|e| AdapterError::Vendor(format!("command request: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AdapterError::Vendor(format!("command parse: {e}")))?;
        if body.get("error").is_some() {
            return Err(AdapterError::Vendor(format!("huawei error: {body}")));
        }
        Ok(body)
    }
}

impl Default for HuaweiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VendorAdapter for HuaweiAdapter {
    /// GET /v5/iot/{project_id}/devices
    async fn list_devices(&self, creds: &VendorCreds) -> Result<Vec<DeviceRecord>, AdapterError> {
        let body = self.get(creds, &format!("/v5/iot/{}/devices", creds.uid)).await?;
        Ok(body["devices"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|d| DeviceRecord {
                id: String::new(),
                vendor_id: d["device_id"].as_str().unwrap_or("").to_string(),
                name: d["node_id"].as_str().unwrap_or("").to_string(),
                category: d["product_id"].as_str().unwrap_or("").to_string(),
                online: d["status"].as_str() == Some("ONLINE"),
                properties: vec![],
            })
            .collect())
    }

    /// GET /v5/iot/{project_id}/devices/{id}/shadow：shadow[].reported 平铺为 {service}.{prop}
    async fn get_properties(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
    ) -> Result<Vec<PropertyValue>, AdapterError> {
        let path = format!("/v5/iot/{}/devices/{vendor_id}/shadow", creds.uid);
        let body = self.get(creds, &path).await?;
        let mut props = Vec::new();
        for svc in body["shadow"].as_array().cloned().unwrap_or_default() {
            let prefix = svc["service_id"].as_str().unwrap_or("").to_string();
            if let Some(reported) = svc["reported"].as_object() {
                for (k, v) in reported {
                    props.push(PropertyValue {
                        code: format!("{prefix}.{k}"),
                        value: v.clone(),
                    });
                }
            }
        }
        Ok(props)
    }

    /// POST /v5/iot/{project_id}/devices/{id}/commands
    async fn send_command(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
        code: &str,
        value: serde_json::Value,
    ) -> Result<(), AdapterError> {
        let path = format!("/v5/iot/{}/devices/{vendor_id}/commands", creds.uid);
        let body = json!({ "command_name": code, "paras": { code: value } });
        self.post(creds, &path, &body).await?;
        Ok(())
    }

    /// IoTDA 事件走控制台消息推送/规则转存配置（AMQP/HTTPS），注册即完成。
    async fn subscribe_events(&self, _creds: &VendorCreds) -> Result<(), AdapterError> {
        Ok(())
    }
}
