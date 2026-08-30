use crate::models::EventMessage;
use crate::td::event_to_insert;
use ecat_data::TsdbClient;
use ecat_data_tdengine::TdengineClient;
use ecat_mq::MessageQueue;
use ecat_mq_kafka::KafkaMq;
use futures_util::{StreamExt, stream::poll_fn};
use std::sync::Arc;

/// 事件总线 topic：必须与 iot-access/src/events.rs 的 TOPIC_EVENTS 一致。
pub const TOPIC_EVENTS: &str = "iot.events";

/// 每批最多攒多少条事件后写一次（一次 REST 请求多行 INSERT）。
const BATCH_SIZE: usize = 100;

pub fn batch_sql(events: &[EventMessage]) -> String {
    events
        .iter()
        .map(event_to_insert)
        .collect::<Vec<_>>()
        .join("\n")
}

/// 后台任务：订阅 iot.events，攒批写入 TDengine。
/// 消费语义 at-least-once 近似（auto_commit=false，重启后从最新 offset 重读，
/// 停机期间消息跳过）；同 ts 同 tags 覆盖保证幂等。
/// 多副本消费需显式 KafkaConfig.group_id（见 ecat-mq-kafka），P2 单实例。
pub async fn run(td: Arc<TdengineClient>, kafka: Arc<KafkaMq>) {
    let mut stream = match kafka.subscribe(TOPIC_EVENTS).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "kafka subscribe failed, ingest exits");
            return;
        }
    };
    let mut stream = poll_fn(move |cx| stream.poll_recv(cx)).boxed();
    let mut buf: Vec<EventMessage> = Vec::with_capacity(BATCH_SIZE);
    // 低事件量（< BATCH_SIZE）时靠时间冲刷，防止数据无限滞留内存
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        tokio::select! {
            msg = stream.next() => match msg {
                Some(Ok(raw)) => match serde_json::from_slice::<EventMessage>(&raw) {
                    Ok(ev) => {
                        buf.push(ev);
                        if buf.len() >= BATCH_SIZE {
                            flush(&td, &mut buf).await;
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "drop unparseable event"),
                },
                Some(Err(e)) => tracing::warn!(error = %e, "kafka recv error"),
                None => break,
            },
            _ = ticker.tick() => {
                if !buf.is_empty() {
                    flush(&td, &mut buf).await;
                }
            }
        }
    }
    // 流结束（订阅中断）时冲刷剩余
    if !buf.is_empty() {
        flush(&td, &mut buf).await;
    }
}

async fn flush(td: &TdengineClient, buf: &mut Vec<EventMessage>) {
    let sql = batch_sql(buf);
    buf.clear();
    if let Err(e) = td.query(&sql).await {
        tracing::error!(error = %e, "tdengine batch write failed");
    }
}
