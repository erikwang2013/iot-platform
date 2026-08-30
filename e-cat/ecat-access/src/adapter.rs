use crate::models::{DeviceRecord, PropertyValue};
use async_trait::async_trait;

/// 当前 UTC 时间，X-Sdk-Date / SigV4 格式：YYYYMMDDTHHMMSSZ。
/// 手写公历转换（Hinnant civil_from_days），避免为时间格式引入 chrono。
pub(crate) fn utc_now() -> String {
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

/// 注册表：vendor 名（devices.vendor 列的值）→ 适配器。
pub fn adapter_for(vendor: &str) -> Result<Box<dyn VendorAdapter>, AdapterError> {
    match vendor {
        "tuya" => Ok(Box::new(crate::adapters::tuya::TuyaAdapter::new())),
        "miot" => Ok(Box::new(crate::adapters::miot::MiAdapter::new())),
        "huawei" => Ok(Box::new(crate::adapters::huawei::HuaweiAdapter::new())),
        "aws" => Ok(Box::new(crate::adapters::aws::AwsAdapter::new())),
        v => Err(AdapterError::UnknownVendor(v.to_string())),
    }
}
