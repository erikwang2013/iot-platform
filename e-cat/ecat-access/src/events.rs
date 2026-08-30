use crate::models::EventMessage;
use ecat_data::Cache;
use ecat_data_redis::RedisCache;
use ecat_mq::{MessageQueue, MqError};
use ecat_mq_kafka::KafkaMq;
use serde_json::{Value, json};

pub use ecat_iot::TOPIC_EVENTS;

/// 影子键前缀：`shadow:{device_id}`。
pub fn shadow_key(device_id: &str) -> String {
    format!("shadow:{device_id}")
}

pub async fn publish_event(mq: &KafkaMq, ev: &EventMessage) -> Result<(), MqError> {
    let payload = serde_json::to_vec(ev).map_err(|e| MqError::Other(format!("encode: {e}")))?;
    mq.publish(TOPIC_EVENTS, &payload).await
}

/// 影子结构：{"online":bool,"properties":{code:value},"ts":ms}。
/// property 事件隐含设备在线；online/offline 只改在线标记。
pub async fn shadow_apply(redis: &RedisCache, ev: &EventMessage) -> Result<(), String> {
    let key = shadow_key(&ev.device_id);
    let mut shadow: Value = redis
        .get(&key)
        .await
        .map_err(|e| format!("shadow get: {e}"))?
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_else(|| json!({ "online": false, "properties": {} }));
    match ev.kind.as_str() {
        "property" => {
            shadow["online"] = json!(true);
            shadow["properties"][ev.code.clone()] = ev.value.clone();
        }
        "online" => shadow["online"] = json!(true),
        "offline" => shadow["online"] = json!(false),
        _ => {}
    }
    shadow["ts"] = json!(ev.ts);
    let raw = serde_json::to_vec(&shadow).map_err(|e| format!("shadow encode: {e}"))?;
    // ttl = 0 → 无过期时间（Cache::set 内部走 SET 而非 PSETEX）
    redis
        .set(&key, &raw, std::time::Duration::ZERO)
        .await
        .map_err(|e| format!("shadow set: {e}"))
}
