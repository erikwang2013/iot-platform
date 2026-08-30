use crate::adapter::{CdnAdapter, CdnError};
use async_trait::async_trait;
use serde_json::{Value, json};

/// Cloudflare 适配器：Bearer token 管理 API + 令牌签名 URL。
pub struct CloudflareAdapter {
    http: reqwest::Client,
}

impl CloudflareAdapter {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("http client"),
        }
    }

    fn base() -> String {
        std::env::var("CLOUDFLARE_API_BASE")
            .unwrap_or_else(|_| "https://api.cloudflare.com/client/v4".into())
    }

    fn token(config: &Value) -> Result<String, CdnError> {
        config["api_token"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| CdnError::Internal("cloudflare: api_token missing".into()))
    }

    fn zone_id(config: &Value) -> Result<String, CdnError> {
        config["zone_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| CdnError::Internal("cloudflare: zone_id missing".into()))
    }

    async fn check(resp: reqwest::Response, what: &str) -> Result<Value, CdnError> {
        let body: Value = resp
            .json()
            .await
            .map_err(|e| CdnError::Vendor(format!("{what} parse: {e}")))?;
        if body["success"] != true {
            return Err(CdnError::Vendor(format!("{what} error: {body}")));
        }
        Ok(body)
    }
}

impl Default for CloudflareAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CdnAdapter for CloudflareAdapter {
    async fn ping(&self, config: &Value) -> Result<(), CdnError> {
        let token = Self::token(config)?;
        let zone = Self::zone_id(config)?;
        let url = format!("{}/zones/{zone}", Self::base());
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| CdnError::Vendor(format!("ping request: {e}")))?;
        Self::check(resp, "ping").await?;
        Ok(())
    }

    async fn purge(&self, config: &Value, urls: &[String]) -> Result<(), CdnError> {
        let token = Self::token(config)?;
        let zone = Self::zone_id(config)?;
        let url = format!("{}/zones/{zone}/purge_cache", Self::base());
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .json(&json!({ "files": urls }))
            .send()
            .await
            .map_err(|e| CdnError::Vendor(format!("purge request: {e}")))?;
        Self::check(resp, "purge").await?;
        Ok(())
    }

    /// `ponytail: cloudflare 无公开预热 API，记为完成（任务状态不阻塞）；
    /// 接入 cache reserve / 自建预热通道时替换为真实调用`
    async fn prefetch(&self, _config: &Value, _urls: &[String]) -> Result<(), CdnError> {
        Ok(())
    }

    /// Cloudflare 令牌签名：verify=base64url(HMAC-SHA256(secret, path+expires))。
    fn sign_url(&self, config: &Value, url: &str, expires_secs: u64) -> Result<String, CdnError> {
        use base64::Engine;
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let secret = config["secret"]
            .as_str()
            .ok_or_else(|| CdnError::Internal("cloudflare: secret missing".into()))?;
        let expires = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + expires_secs;
        let path = url.split("://").nth(1).unwrap_or(url).split('/').skip(1).collect::<Vec<_>>().join("/");
        let path = format!("/{path}");
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
        mac.update(path.as_bytes());
        mac.update(expires.to_string().as_bytes());
        let verify = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
        let sep = if url.contains('?') { "&" } else { "?" };
        Ok(format!("{url}{sep}verify={verify}&expires={expires}"))
    }
}
