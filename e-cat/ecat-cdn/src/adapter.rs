use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum CdnError {
    #[error("vendor api error: {0}")]
    Vendor(String),
    #[error("unknown cdn vendor: {0}")]
    UnknownVendor(String),
    #[error("internal: {0}")]
    Internal(String),
}

/// CDN 供应商适配器统一接口。config 为解密后的厂商配置 JSON
/// （cloudflare: api_token/zone_id/secret；aliyun: access_key_id/access_key_secret/auth_key；
///  tencent: secret_id/secret_key/auth_key）。
#[async_trait]
pub trait CdnAdapter: Send + Sync {
    /// 连通性测试（管理端手动触发）。
    async fn ping(&self, config: &Value) -> Result<(), CdnError>;
    /// 刷新（清缓存）。
    async fn purge(&self, config: &Value, urls: &[String]) -> Result<(), CdnError>;
    /// 预热（预拉取到边缘）。
    async fn prefetch(&self, config: &Value, urls: &[String]) -> Result<(), CdnError>;
    /// 生成带过期时间的签名下载 URL。
    fn sign_url(&self, config: &Value, url: &str, expires_secs: u64) -> Result<String, CdnError>;
}

/// 注册表：vendor 名（cdn_providers.vendor 列的值）→ 适配器。
pub fn adapter_for(vendor: &str) -> Result<Box<dyn CdnAdapter>, CdnError> {
    match vendor {
        "cloudflare" => Ok(Box::new(crate::adapters::cloudflare::CloudflareAdapter::new())),
        "aliyun" => Ok(Box::new(crate::adapters::aliyun::AliyunAdapter::new())),
        "tencent" => Ok(Box::new(crate::adapters::tencent::TencentAdapter::new())),
        v => Err(CdnError::UnknownVendor(v.to_string())),
    }
}
