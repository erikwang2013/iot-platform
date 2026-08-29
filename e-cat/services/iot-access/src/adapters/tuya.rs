// 见 Task 5：完整涂鸦 API 实现；此处仅占位保证编译
use crate::adapter::{AdapterError, VendorAdapter, VendorCreds};
use crate::models::{DeviceRecord, PropertyValue};
use async_trait::async_trait;

pub struct TuyaAdapter;

impl TuyaAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl VendorAdapter for TuyaAdapter {
    async fn list_devices(&self, _c: &VendorCreds) -> Result<Vec<DeviceRecord>, AdapterError> {
        Err(AdapterError::Internal("tuya adapter not implemented".into()))
    }
    async fn get_properties(
        &self,
        _c: &VendorCreds,
        _vendor_id: &str,
    ) -> Result<Vec<PropertyValue>, AdapterError> {
        Err(AdapterError::Internal("tuya adapter not implemented".into()))
    }
    async fn send_command(
        &self,
        _c: &VendorCreds,
        _vendor_id: &str,
        _code: &str,
        _value: serde_json::Value,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::Internal("tuya adapter not implemented".into()))
    }
    async fn subscribe_events(&self, _c: &VendorCreds) -> Result<(), AdapterError> {
        Err(AdapterError::Internal("tuya adapter not implemented".into()))
    }
}
