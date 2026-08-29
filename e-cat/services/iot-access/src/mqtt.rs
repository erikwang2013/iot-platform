// 见 Task 7：完整直连 MQTT 实现（订阅/发布/重连）；此处仅保证 api.rs 编译
use serde_json::Value;

pub async fn publish_command(device_id: &str, code: &str, value: &Value) -> Result<(), String> {
    Err(format!("mqtt not implemented: {device_id} {code} {value}"))
}
