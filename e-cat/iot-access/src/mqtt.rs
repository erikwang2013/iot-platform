use crate::events::{publish_event, shadow_apply};
use crate::models::EventMessage;
use crate::store::Store;
use ecat_data_redis::RedisCache;
use ecat_mq::MessageQueue;
use ecat_mq_kafka::KafkaMq;
use ecat_mq_mqtt::MqttMq;
use futures_util::StreamExt;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 直连设备上报 topic 约定：iot/devices/{device_id}/properties。
pub fn report_topic(device_id: &str) -> String {
    format!("iot/devices/{device_id}/properties")
}

/// 指令下发 topic 约定：iot/devices/{device_id}/commands。
pub fn command_topic(device_id: &str) -> String {
    format!("iot/devices/{device_id}/commands")
}

/// 上报 payload：{"code","value","ts?"}；ts 缺省取当前毫秒。
pub fn parse_payload(
    platform_id: &str,
    tenant_id: &str,
    raw: &[u8],
) -> Result<EventMessage, String> {
    let v: serde_json::Value =
        serde_json::from_slice(raw).map_err(|e| format!("bad mqtt payload: {e}"))?;
    Ok(EventMessage {
        device_id: platform_id.to_string(),
        tenant_id: tenant_id.to_string(),
        kind: "property".into(),
        code: v["code"].as_str().unwrap_or("").to_string(),
        value: v["value"].clone(),
        ts: v["ts"].as_i64().unwrap_or_else(|| now_ms()),
    })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// 指令下发（api.rs 直连分支调用）。
pub async fn publish_command(
    mqtt: &MqttMq,
    device_id: &str,
    code: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    let payload = json!({ "code": code, "value": value });
    mqtt.publish(
        &command_topic(device_id),
        serde_json::to_vec(&payload).unwrap().as_slice(),
    )
    .await
    .map_err(|e| format!("mqtt publish: {e}"))
}

/// 后台任务：每 30s 扫描 vendor='direct' 设备，为新设备建立订阅。
/// 每个设备一条独立订阅（ecat-mq-mqtt 的 subscribe 自带独立连接），
/// 消息回调中设备 ID 由订阅上下文确定，杜绝 payload 伪造跨租户。
/// ponytail: 30s 轮询；设备量大后改为注册接口主动订阅。
pub async fn run(
    mqtt: Arc<MqttMq>,
    store: Arc<Store>,
    redis: Arc<RedisCache>,
    kafka: Arc<KafkaMq>,
) {
    let subs: Arc<Mutex<HashMap<String, ()>>> = Arc::new(Mutex::new(HashMap::new()));
    loop {
        let devices = match store.list_direct_devices().await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = %e, "list direct devices failed");
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
        };
        for (device_id, tenant_id) in devices {
            let already = {
                let s = subs.lock().await;
                s.contains_key(&device_id)
            };
            if already {
                continue;
            }
            let topic = report_topic(&device_id);
            let mut stream = match mqtt.subscribe(&topic).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(topic, error = %e, "mqtt subscribe failed");
                    continue;
                }
            };
            let mut stream =
                futures_util::stream::poll_fn(move |cx| stream.poll_recv(cx)).boxed();
            subs.lock().await.insert(device_id.clone(), ());
            let redis = redis.clone();
            let kafka = kafka.clone();
            let did = device_id.clone();
            tokio::spawn(async move {
                while let Some(Ok(raw)) = stream.next().await {
                    match parse_payload(&did, &tenant_id, &raw) {
                        Ok(ev) => {
                            if let Err(e) = publish_event(&kafka, &ev).await {
                                tracing::error!(error = %e, "kafka publish failed");
                            }
                            if let Err(e) = shadow_apply(&redis, &ev).await {
                                tracing::warn!(error = %e, "shadow update failed");
                            }
                        }
                        Err(e) => tracing::warn!(device = %did, error = %e, "drop mqtt payload"),
                    }
                }
            });
        }
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}
