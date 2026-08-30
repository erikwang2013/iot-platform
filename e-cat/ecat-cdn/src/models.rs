use serde::{Deserialize, Serialize};
use serde_json::Value;

/// CDN 供应商（config 为解密后的厂商配置 JSON）。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Provider {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub vendor: String,
    pub domain: String,
    pub config: Value,
    pub status: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct NewProvider {
    pub name: String,
    pub vendor: String,
    pub domain: Option<String>,
    pub config: Value,
}

#[derive(Deserialize, Default)]
pub struct UpdateProvider {
    pub name: Option<String>,
    pub domain: Option<String>,
    pub config: Option<Value>,
}

#[derive(Deserialize)]
pub struct UrlReq {
    pub url: String,
    pub expires_secs: u64,
}

#[derive(Deserialize)]
pub struct TaskReq {
    pub urls: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CdnTask {
    pub id: String,
    pub tenant_id: String,
    pub provider_id: String,
    pub kind: String,
    pub urls: Vec<String>,
    pub status: String,
    pub error: String,
    pub created_at: String,
}

/// 边界校验：URL 必须是 http/https 且长度受限。
pub fn validate_url(url: &str) -> Result<(), String> {
    if url.len() > 2048 || !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("url must be http(s) and within 2048 chars".into());
    }
    Ok(())
}

/// 边界校验：签名 URL 有效期 60s ~ 24h。
pub fn validate_expires(secs: u64) -> Result<(), String> {
    if !(60..=86400).contains(&secs) {
        return Err("expires_secs must be 60..=86400".into());
    }
    Ok(())
}
