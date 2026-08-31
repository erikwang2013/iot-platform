// Copyright (c) 2026 erik <erik@erik.xyz> — https://erik.xyz
use serde::{Deserialize, Serialize};

/// 当前时间（epoch 毫秒）。
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// 事件总线 topic：iot-access 发布，iot-data / iot-rule 订阅。
pub const TOPIC_EVENTS: &str = "iot.events";

/// 指令事件 topic：iot-rule（联动动作 D-3）发布，iot-access 消费后经 MQTT 下发。
pub const TOPIC_COMMANDS: &str = "iot.commands";

/// 指令事件消息（D-3 联动）：由规则引擎发布，access 服务消费并下发目标设备。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CommandEvent {
    pub device_id: String,
    pub tenant_id: String,
    pub code: String,
    pub value: serde_json::Value,
    pub ts: i64,
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
