use crate::adapter::{CdnAdapter, CdnError};
use crate::util::{percent_encode, utc_parts};
use async_trait::async_trait;
use serde_json::Value;

/// 阿里云 CDN 适配器：OpenAPI RPC 签名（HMAC-SHA1）+ A/B 鉴权签名 URL。
pub struct AliyunAdapter;

impl AliyunAdapter {
    pub fn new() -> Self {
        Self
    }

    fn base() -> String {
        std::env::var("ALIYUN_CDN_BASE")
            .unwrap_or_else(|_| "https://cdn.aliyuncs.com".into())
    }

    /// RPC 签名后 POST：params 为业务参数（Action 等），自动附公共参数与签名。
    async fn call(config: &Value, params: &[(&str, &str)]) -> Result<Value, CdnError> {
        let ak = config["access_key_id"]
            .as_str()
            .ok_or_else(|| CdnError::Internal("aliyun: access_key_id missing".into()))?;
        let sk = config["access_key_secret"]
            .as_str()
            .ok_or_else(|| CdnError::Internal("aliyun: access_key_secret missing".into()))?;
        let (_, p) = utc_parts();
        let timestamp = format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", p.0, p.1, p.2, p.3, p.4, p.5);
        let mut query: Vec<(String, String)> = params
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .chain(vec![
                ("AccessKeyId".into(), ak.to_string()),
                ("Format".into(), "JSON".into()),
                ("SignatureMethod".into(), "HMAC-SHA1".into()),
                ("SignatureNonce".into(), uuid::Uuid::new_v4().to_string()),
                ("SignatureVersion".into(), "1.0".into()),
                ("Timestamp".into(), timestamp),
                ("Version".into(), "2018-05-10".into()),
            ])
            .collect();
        query.sort_by_key(|(k, _)| k.clone());
        let canonical = query
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let sts = format!("POST&{}&{}", percent_encode("/"), percent_encode(&canonical));
        let sig = aliyun_hmac_sha1_base64(&format!("{sk}&"), &sts);
        query.push(("Signature".into(), sig));
        let url = format!("{}?{}", Self::base(), to_query(&query));
        let resp = reqwest::Client::new()
            .post(&url)
            .send()
            .await
            .map_err(|e| CdnError::Vendor(format!("request: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| CdnError::Vendor(format!("parse: {e}")))?;
        if body["Code"].as_str().is_some_and(|c| c != "Success") {
            return Err(CdnError::Vendor(format!("aliyun error: {body}")));
        }
        Ok(body)
    }
}

fn to_query(params: &[(String, String)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn aliyun_hmac_sha1_base64(key: &str, msg: &str) -> String {
    use base64::Engine;
    use hmac::{Hmac, Mac};
    use sha1::Sha1;
    type HmacSha1 = Hmac<Sha1>;
    let mut mac = HmacSha1::new_from_slice(key.as_bytes()).expect("hmac key");
    mac.update(msg.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

/// A/B 鉴权 auth_key：md5(密钥 + 路径 + 时间戳 + 随机数 + uid)，hex 小写。
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

impl Default for AliyunAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CdnAdapter for AliyunAdapter {
    async fn ping(&self, config: &Value) -> Result<(), CdnError> {
        Self::call(config, &[("Action", "DescribeCdnService")]).await?;
        Ok(())
    }

    async fn purge(&self, config: &Value, urls: &[String]) -> Result<(), CdnError> {
        Self::call(
            config,
            &[
                ("Action", "RefreshObjectCaches"),
                ("ObjectType", "File"),
                ("ObjectPath", &urls.join(",")),
            ],
        )
        .await?;
        Ok(())
    }

    async fn prefetch(&self, config: &Value, urls: &[String]) -> Result<(), CdnError> {
        Self::call(
            config,
            &[
                ("Action", "PushObjectCache"),
                ("ObjectType", "File"),
                ("ObjectPath", &urls.join(",")),
            ],
        )
        .await?;
        Ok(())
    }

    fn sign_url(&self, config: &Value, url: &str, expires_secs: u64) -> Result<String, CdnError> {
        let secret = config["auth_key"]
            .as_str()
            .ok_or_else(|| CdnError::Internal("aliyun: auth_key missing".into()))?;
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
