// Copyright (c) 2026 erik <erik@erik.xyz> — https://erik.xyz
use serde::{Deserialize, Serialize};

/// 事件总线 topic：iot-access 发布，iot-data / iot-rule 订阅。
pub const TOPIC_EVENTS: &str = "iot.events";

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
