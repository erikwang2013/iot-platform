use crate::models::{DeviceRecord, PropertyValue};
use async_trait::async_trait;

/// 解密后的厂商凭据（DB 中存 AES 密文，见 crypto.rs / store.rs）
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct VendorCreds {
    pub client_id: String,
    pub client_secret: String,
    pub uid: String,
    pub access_token: String,
    pub refresh_token: String,
    /// access_token 过期 epoch 秒；过期时由适配器用 refresh_token 刷新
    pub expires_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("vendor api error: {0}")]
    Vendor(String),
    #[error("token expired and refresh failed: {0}")]
    Refresh(String),
    #[error("unknown vendor: {0}")]
    UnknownVendor(String),
    #[error("internal: {0}")]
    Internal(String),
}

/// 厂商适配器统一接口。subscribe_events 的语义：注册厂商侧事件推送
/// （涂鸦为控制台配置 Webhook URL，事件由 webhook.rs 消费，故返回 Ok）；
/// 直连设备的"订阅"在 mqtt.rs 中按设备逐个建立，不经过本 Trait。
#[async_trait]
pub trait VendorAdapter: Send + Sync {
    async fn list_devices(&self, creds: &VendorCreds) -> Result<Vec<DeviceRecord>, AdapterError>;
    async fn get_properties(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
    ) -> Result<Vec<PropertyValue>, AdapterError>;
    async fn send_command(
        &self,
        creds: &VendorCreds,
        vendor_id: &str,
        code: &str,
        value: serde_json::Value,
    ) -> Result<(), AdapterError>;
    async fn subscribe_events(&self, creds: &VendorCreds) -> Result<(), AdapterError>;
}

/// 注册表：vendor 名（devices.vendor 列的值）→ 适配器。P4 补 miot/huawei/aws/azure。
pub fn adapter_for(vendor: &str) -> Result<Box<dyn VendorAdapter>, AdapterError> {
    match vendor {
        "tuya" => Ok(Box::new(crate::adapters::tuya::TuyaAdapter::new())),
        v => Err(AdapterError::UnknownVendor(v.to_string())),
    }
}
