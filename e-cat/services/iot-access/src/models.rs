use serde::{Deserialize, Serialize};

/// 物模型属性值。code 为厂商属性 code（涂鸦）或设备自定义 code（直连）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PropertyValue {
    pub code: String,
    pub value: serde_json::Value,
}

/// 统一设备记录：厂商设备拉取后的中间形态，入库前转换。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceRecord {
    /// 平台侧设备 UUID（device_links.device_id）
    pub id: String,
    /// 厂商侧设备 ID（涂鸦 devId；直连设备等于平台 UUID）
    pub vendor_id: String,
    pub name: String,
    /// 厂商品类（涂鸦 category；直连设备为 "direct"）
    pub category: String,
    pub online: bool,
    pub properties: Vec<PropertyValue>,
}

/// 统一事件消息：Webhook、MQTT 直连、Kafka `iot.events`、Redis 影子共用。
/// kind 取值：`"property"` | `"online"` | `"offline"`。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EventMessage {
    pub device_id: String,
    pub tenant_id: String,
    pub kind: String,
    /// property 时为属性 code；online/offline 时为 "online"/"offline"
    pub code: String,
    pub value: serde_json::Value,
    /// epoch 毫秒
    pub ts: i64,
}
