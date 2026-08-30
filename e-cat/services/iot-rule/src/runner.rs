use crate::engine::{TOPIC_EVENTS, evaluate};
use crate::models::{AlertMessage, EventMessage};
use crate::push::PushHub;
use crate::store::RuleStore;
use ecat_mq::MessageQueue;
use ecat_mq_kafka::KafkaMq;
use futures_util::{StreamExt, stream::poll_fn};
use std::sync::Arc;

/// 告警 → webhook 请求体（AlertMessage JSON，与 WS 推送同一形状）。
pub fn webhook_payload(msg: &AlertMessage) -> Vec<u8> {
    serde_json::to_vec(msg).unwrap_or_default()
}

/// 后台任务：消费 iot.events → 按事件租户加载规则 → 纯函数 evaluate →
/// 命中则 WS 推送 + 落告警记录 + 可选 webhook。
/// ponytail: 每事件查一次规则（MVP 量级足够；量大时改为定时重载 + 内存缓存）；
/// 告警无冷却去重，事件风暴会连续触发——需要时按 (rule_id, 时间窗) 去重。
pub async fn run(kafka: Arc<KafkaMq>, store: Arc<RuleStore>, hub: PushHub, http: reqwest::Client) {
    let mut stream = match kafka.subscribe(TOPIC_EVENTS).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "kafka subscribe failed, rule engine exits");
            return;
        }
    };
    let mut stream = poll_fn(move |cx| stream.poll_recv(cx)).boxed();
    while let Some(Ok(raw)) = stream.next().await {
        let ev: EventMessage = match serde_json::from_slice(&raw) {
            Ok(ev) => ev,
            Err(e) => {
                tracing::warn!(error = %e, "drop unparseable event");
                continue;
            }
        };
        let rules = match store.list_rules(&ev.tenant_id).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "list rules failed");
                continue;
            }
        };
        for msg in evaluate(&ev, &rules) {
            hub.publish(&msg.tenant_id, &msg);
            if let Err(e) = store.insert_alert(&msg).await {
                tracing::warn!(error = %e, "alert record insert failed");
            }
            let webhook = rules
                .iter()
                .find(|r| r.id == msg.rule_id)
                .and_then(|r| r.webhook_url.clone());
            if let Some(url) = webhook {
                notify_webhook(&http, &url, &msg).await;
            }
        }
    }
}

async fn notify_webhook(http: &reqwest::Client, url: &str, msg: &AlertMessage) {
    match http.post(url).body(webhook_payload(msg)).send().await {
        Ok(r) if r.status().is_success() => {}
        Ok(r) => tracing::warn!(status = %r.status().as_u16(), url = %url, "webhook non-2xx"),
        Err(e) => tracing::warn!(error = %e, url = %url, "webhook delivery failed"),
    }
}
