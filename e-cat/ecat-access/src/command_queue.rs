//! 离线指令队列（D-2）：设备离线时指令入队，上线后自动补发。
//! 补发触发点：send_command 检测到设备离线时入队；设备上线（shadow online）
//! 时由 flush_for_device 补发。补发按入队时间升序，带过期时间。
use crate::events::shadow_key;
use crate::store::Store;
use ecat_data::Cache;
use ecat_data_redis::RedisCache;
use ecat_mq_mqtt::MqttMq;
use serde_json::Value;
use std::sync::Arc;

/// 指令过期时间（秒）：env COMMAND_EXPIRE_SECS 默认 3600（1 小时）。
pub fn command_expire_secs() -> i64 {
    std::env::var("COMMAND_EXPIRE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3600)
}

/// 设备是否在线（查 Redis shadow online 标记）。
pub async fn device_online(redis: &RedisCache, device_id: &str) -> bool {
    redis
        .get(&shadow_key(device_id))
        .await
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_slice::<Value>(&raw).ok())
        .and_then(|v| v.get("online").and_then(Value::as_bool))
        .unwrap_or(false)
}

/// 设备上线后补发排队指令（按入队顺序）。逐条 MQTT 下发，成功即删。
/// 某条失败仅记日志、保留在队列，下次上线再补发。
pub async fn flush_for_device(
    store: Arc<Store>,
    redis: Arc<RedisCache>,
    mqtt: Arc<MqttMq>,
    device_id: &str,
) {
    let pending = match store.pending_commands(device_id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(device = %device_id, error = %e, "flush commands: load pending failed");
            return;
        }
    };
    for (id, code, value) in pending {
        if let Err(e) =
            crate::mqtt::publish_command(&mqtt, device_id, &code, &value).await
        {
            tracing::warn!(device = %device_id, cmd = %id, error = %e, "flush command failed; kept in queue");
            continue;
        }
        if let Err(e) = store.delete_command(&id).await {
            tracing::warn!(device = %device_id, cmd = %id, error = %e, "flush command: delete failed");
        }
        tracing::info!(device = %device_id, cmd = %id, code = %code, "offline command replayed");
    }
    let _ = redis; // 保留引用签名（补发不依赖 shadow 写回）
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_expire_secs_defaults() {
        // SAFETY: 单线程测试
        unsafe { std::env::remove_var("COMMAND_EXPIRE_SECS") };
        assert_eq!(command_expire_secs(), 3600);
    }

    #[test]
    fn command_expire_secs_respects_env() {
        // SAFETY: 单线程测试
        unsafe { std::env::set_var("COMMAND_EXPIRE_SECS", "600") };
        assert_eq!(command_expire_secs(), 600);
        unsafe { std::env::remove_var("COMMAND_EXPIRE_SECS") };
    }
}
