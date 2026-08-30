use serde::{Deserialize, Serialize};

/// 统一事件消息（Kafka `iot.events` 消费侧反序列化）。
/// 字段与 P1 iot-access/src/models.rs 的 EventMessage 完全一致，
/// 新增/改名字段必须两边同步。
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

/// 历史曲线单点：ts 为 epoch 毫秒，value 为原始值（数值或字符串）。
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct HistoryPoint {
    pub ts: i64,
    pub value: serde_json::Value,
}
