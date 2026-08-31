//! iot-edge 二进制：边缘网关运行时。
//! 循环：模拟采集（COLLECT_INTERVAL_MS 可配，缺省 1s）→ 落库 → MQTT 上报；
//! 每 30s 发心跳（含积压数）；连接中断时数据留在 SQLite，恢复后按 ts 升序
//! 以 ≤200 msg/s 补发。MQTT 复用 ecat-mq-mqtt（QoS1 + clean session=false）。
//!
//! 环境变量：
//!   EDGE_DEVICE_ID / EDGE_TENANT_ID  设备与租户 ID
//!   MQTT_URL                         默认 tcp://localhost:1883
//!   COLLECT_INTERVAL_MS              采集间隔（毫秒，默认 1000）
//!   EDGE_DB_PATH                     SQLite 缓冲路径（默认 edge_buffer.db）
use ecat_edge::buffer::{Buffer, BufferedPoint};
use ecat_edge::relay::{Sender, heartbeat_payload, ingest_and_send, replay};
use ecat_mq::MessageQueue;
use ecat_mq_mqtt::MqttMq;
use std::sync::Arc;

/// 心跳间隔（协议 §3）：30s。
const HEARTBEAT_SECS: u64 = 30;
/// 心跳 topic 前缀。
fn heartbeat_topic(device_id: &str) -> String {
    format!("iot/devices/{device_id}/heartbeat")
}
/// 属性上报 topic。
fn properties_topic(device_id: &str) -> String {
    format!("iot/devices/{device_id}/properties")
}

/// 基于 MqttMq 的属性发送器：发布到 properties topic。
struct MqttSender {
    mqtt: MqttMq,
    topic: String,
}

#[async_trait::async_trait]
impl Sender for MqttSender {
    async fn send(&self, p: &BufferedPoint) -> Result<(), String> {
        let payload = serde_json::json!({
            "code": p.code,
            "value": serde_json::from_str::<serde_json::Value>(&p.value_json)
                .unwrap_or(serde_json::Value::String(p.value_json.clone())),
            "ts": p.ts,
        });
        let raw = serde_json::to_vec(&payload).map_err(|e| format!("encode: {e}"))?;
        self.mqtt
            .publish(&self.topic, &raw)
            .await
            .map_err(|e| format!("mqtt publish: {e}"))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let device_id = std::env::var("EDGE_DEVICE_ID").unwrap_or_else(|_| "edge-dev-1".into());
    let mqtt_url = std::env::var("MQTT_URL").unwrap_or_else(|_| "tcp://localhost:1883".into());
    let db_path = std::env::var("EDGE_DB_PATH").unwrap_or_else(|_| "edge_buffer.db".into());
    let collect_ms: u64 = std::env::var("COLLECT_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);

    let buffer = Buffer::open(&db_path)?;
    let mqtt = MqttMq::connect(&mqtt_url).await?;
    let sender = MqttSender {
        mqtt,
        topic: properties_topic(&device_id),
    };

    // 采集循环：每 collect_ms 产生一个温度样本（演示用正弦波动）。
    // 生产接入真实设备协议时替换此循环的采样来源即可。
    let buffer = Arc::new(buffer);
    let sender = Arc::new(sender);
    let hb_topic = heartbeat_topic(&device_id);

    // 心跳任务（共享 MqttSender 的客户端，发布到心跳 topic）
    let (hb_buffer, hb_sender) = (Arc::clone(&buffer), Arc::clone(&sender));
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(HEARTBEAT_SECS));
        tick.tick().await; // 跳过首 tick
        loop {
            tick.tick().await;
            let buffered = hb_buffer.pending_count().unwrap_or(0);
            let now = ecat_iot_now_ms();
            let payload = heartbeat_payload(now, buffered);
            let raw = serde_json::to_vec(&payload).unwrap_or_default();
            let _ = hb_sender.mqtt.publish(&hb_topic, &raw).await;
            tracing::info!(buffered, "heartbeat");
        }
    });

    // 采集 + 上报循环（写后发送，失败积压）
    let mut value: f64 = 20.0;
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(collect_ms));
    tick.tick().await;
    loop {
        tick.tick().await;
        value = 20.0 + (value - 20.0) * 0.9 + (rand_sin() * 0.5);
        let point = BufferedPoint {
            device_id: device_id.clone(),
            code: "temperature".into(),
            value_json: value.to_string(),
            ts: ecat_iot_now_ms(),
        };
        let sent = ingest_and_send(&buffer, &*sender, &point).await;
        // 若发送失败（离线），尝试补发积压（恢复后自动清空）
        if !sent {
            let replayed = replay(&buffer, &*sender).await;
            if replayed > 0 {
                tracing::info!(replayed, "backlog replayed after recovery");
            }
        }
    }
}

/// 当前毫秒时间戳（epoch）。
fn ecat_iot_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 演示用伪随机（确定性正弦变化，避免引入 rand 依赖）。
fn rand_sin() -> f64 {
    let t = ecat_iot_now_ms() as f64 / 1000.0;
    (t.sin() + 1.0) / 2.0
}
