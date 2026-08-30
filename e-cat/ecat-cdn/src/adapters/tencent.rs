use crate::adapter::{CdnAdapter, CdnError};
use crate::util::{host_of, utc_parts};
use async_trait::async_trait;
use serde_json::{Value, json};

/// 腾讯云 CDN 适配器：API 3.0 TC3-HMAC-SHA256 签名 + URL 鉴权签名。
pub struct TencentAdapter;

impl TencentAdapter {
    pub fn new() -> Self {
        Self
    }

    fn base() -> String {
        std::env::var("TENCENT_CDN_BASE")
            .unwrap_or_else(|_| "https://cdn.tencentcloudapi.com".into())
    }

    /// TC3 签名后 POST JSON body。
    async fn call(config: &Value, action: &str, body: &Value) -> Result<Value, CdnError> {
        let sid = config["secret_id"]
            .as_str()
            .ok_or_else(|| CdnError::Internal("tencent: secret_id missing".into()))?;
        let sk = config["secret_key"]
            .as_str()
            .ok_or_else(|| CdnError::Internal("tencent: secret_key missing".into()))?;
        let (secs, p) = utc_parts();
        let date = format!("{:04}-{:02}-{:02}", p.0, p.1, p.2);
        let host = host_of(&Self::base());
        let raw = serde_json::to_vec(body).unwrap_or_default();
        let payload_hash = sha256_hex(&raw);
        let canonical = format!(
            "POST\n/\n\ncontent-type:application/json\nhost:{host}\n\ncontent-type;host\n{payload_hash}"
        );
        let scope = format!("{date}/cdn/tc3_request");
        let sts = format!(
            "TC3-HMAC-SHA256\n{secs}\n{scope}\n{}",
            sha256_hex(canonical.as_bytes())
        );
        let k_date = hmac_sha256(format!("TC3{sk}").as_bytes(), &date);
        let k_service = hmac_sha256(&k_date, "cdn");
        let k_signing = hmac_sha256(&k_service, "tc3_request");
        let sig = hex::encode(hmac_sha256(&k_signing, &sts));
        let auth = format!(
            "TC3-HMAC-SHA256 Credential={sid}/{scope}, SignedHeaders=content-type;host, Signature={sig}"
        );
        let resp = reqwest::Client::new()
            .post(Self::base())
            .header("Content-Type", "application/json")
            .header("Host", &host)
            .header("X-TC-Action", action)
            .header("X-TC-Version", "2018-06-06")
            .header("X-TC-Timestamp", secs.to_string())
            .header("X-TC-Region", "ap-guangzhou")
            .header("Authorization", auth)
            .body(raw)
            .send()
            .await
            .map_err(|e| CdnError::Vendor(format!("request: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| CdnError::Vendor(format!("parse: {e}")))?;
        if body["Response"]["Error"].is_object() {
            return Err(CdnError::Vendor(format!("tencent error: {body}")));
        }
        Ok(body)
    }
}

fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b))
}

fn hmac_sha256(key: &[u8], msg: &str) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(msg.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// URL 鉴权 auth_key：md5(密钥 + 路径 + 时间戳 + 随机数 + uid)，hex 小写。
fn auth_key(secret: &str, path: &str, ts: u64, rand: &str, uid: &str) -> String {
    use md5::{Digest, Md5};
    let mut d = Md5::new();
    d.update(secret.as_bytes());
    d.update(path.as_bytes());
    d.update(ts.to_string().as_bytes());
    d.update(rand.as_bytes());
    d.update(uid.as_bytes());
    hex::encode(d.finalize())
}

impl Default for TencentAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CdnAdapter for TencentAdapter {
    async fn ping(&self, config: &Value) -> Result<(), CdnError> {
        Self::call(config, "DescribeCdnDomains", &json!({ "Limit": 1 })).await?;
        Ok(())
    }

    async fn purge(&self, config: &Value, urls: &[String]) -> Result<(), CdnError> {
        Self::call(config, "PurgeUrlsCache", &json!({ "Urls": urls })).await?;
        Ok(())
    }

    async fn prefetch(&self, config: &Value, urls: &[String]) -> Result<(), CdnError> {
        Self::call(config, "PushUrlsCache", &json!({ "Urls": urls })).await?;
        Ok(())
    }

    fn sign_url(&self, config: &Value, url: &str, expires_secs: u64) -> Result<String, CdnError> {
        let secret = config["auth_key"]
            .as_str()
            .ok_or_else(|| CdnError::Internal("tencent: auth_key missing".into()))?;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + expires_secs;
        let (_, path) = url.split_once("://").unwrap_or(("", url));
        let path = format!("/{}", path.split('/').skip(1).collect::<Vec<_>>().join("/"));
        let rand = "0";
        let uid = "0";
        let key = auth_key(secret, &path, ts, rand, uid);
        let sep = if url.contains('?') { "&" } else { "?" };
        Ok(format!("{url}{sep}auth_key={ts}-{rand}-{uid}-{key}"))
    }
}
