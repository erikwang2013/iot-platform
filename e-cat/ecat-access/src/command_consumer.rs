//! 指令事件消费（D-3 联动）：订阅 iot.commands，解析 CommandEvent 后经 MQTT 下发。
//! 规则引擎（联动动作）→ 发布指令事件 → 本消费组 → publish_command。
use crate::store::Store;
use ecat_iot::CommandEvent;
use ecat_mq::MessageQueue;
use ecat_mq_kafka::KafkaMq;
use ecat_mq_mqtt::MqttMq;
use futures_util::{StreamExt, stream::poll_fn};
use std::sync::Arc;

/// 消费 iot.commands，逐条经 MQTT 下发到目标设备。
pub async fn run(kafka: Arc<KafkaMq>, mqtt: Arc<MqttMq>, store: Arc<Store>) {
    let mut stream = match kafka.subscribe(ecat_iot::TOPIC_COMMANDS).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "kafka subscribe commands failed, command consumer exits");
            return;
        }
    };
    let mut stream = poll_fn(move |cx| stream.poll_recv(cx)).boxed();
    while let Some(Ok(raw)) = stream.next().await {
        let cmd: CommandEvent = match serde_json::from_slice(&raw) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "drop unparseable command event");
                continue;
            }
        };
        // 租户隔离：目标设备必须属于指令携带的租户，防跨租户下发
        match store.tenant_of_device(&cmd.device_id).await {
            Ok(t) if t == cmd.tenant_id => {}
            Ok(_) => {
                tracing::warn!(device = %cmd.device_id, "command target not in tenant; dropped");
                continue;
            }
            Err(e) => {
                tracing::warn!(error = %e, device = %cmd.device_id, "command target lookup failed");
                continue;
            }
        }
        if let Err(e) = crate::mqtt::publish_command(&mqtt, &cmd.device_id, &cmd.code, &cmd.value)
            .await
        {
            tracing::warn!(device = %cmd.device_id, code = %cmd.code, error = %e, "linkage command send failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_event_roundtrips() {
        let cmd = CommandEvent {
            device_id: "d1".into(),
            tenant_id: "t1".into(),
            code: "power".into(),
            value: serde_json::json!(true),
            ts: 1700000000,
        };
        let raw = serde_json::to_vec(&cmd).unwrap();
        let parsed: CommandEvent = serde_json::from_slice(&raw).unwrap();
        assert_eq!(parsed, cmd);
    }
}
