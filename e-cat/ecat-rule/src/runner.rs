use crate::engine::{TOPIC_EVENTS, evaluate};
use crate::models::{AlertMessage, EventMessage};
use crate::push::AlertBridge;
use crate::store::RuleStore;
use ecat_iot::CommandEvent;
use ecat_mq::MessageQueue;
use ecat_mq_kafka::KafkaMq;
use futures_util::{StreamExt, stream::poll_fn};
use std::sync::Arc;

/// 告警 → webhook 请求体（AlertMessage JSON，与 WS 推送同一形状）。
pub fn webhook_payload(msg: &AlertMessage) -> Vec<u8> {
    serde_json::to_vec(msg).unwrap_or_default()
}

/// 告警去重窗口（毫秒）：同一 (rule_id, device_id, code) 在该窗口内不重复
/// 投递（事件风暴防护，D-1）。env ALERT_DEDUP_WINDOW_SECS 默认 300s。
fn dedup_window_ms() -> i64 {
    std::env::var("ALERT_DEDUP_WINDOW_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|s: i64| s * 1000)
        .unwrap_or(300 * 1000)
}

/// 告警去重器：记录每个 (rule_id, device_id, code) 最近投递时间。
/// 非线程安全——仅 runner 单任务使用（std::collections::HashMap + Mutex 亦可，
/// 此处简化：runner 内单一消费循环串行访问）。
pub struct AlertDedup {
    last: std::collections::HashMap<(String, String, String), i64>,
    window_ms: i64,
}

impl AlertDedup {
    pub fn new() -> Self {
        Self {
            last: std::collections::HashMap::new(),
            window_ms: dedup_window_ms(),
        }
    }

    /// 判断该告警是否应在窗口内抑制。返回 true 表示应投递。
    /// 窗口 <= 0 时全部投递（关闭去重）。
    pub fn should_deliver(&mut self, msg: &AlertMessage) -> bool {
        if self.window_ms <= 0 {
            return true;
        }
        let key = (msg.rule_id.clone(), msg.device_id.clone(), msg.code.clone());
        let now = msg.ts;
        let suppress = self
            .last
            .get(&key)
            .map(|last_ts| now - *last_ts < self.window_ms)
            .unwrap_or(false);
        if !suppress {
            self.last.insert(key, now);
        }
        !suppress
    }
}

impl Default for AlertDedup {
    fn default() -> Self {
        Self::new()
    }
}

/// 后台任务：消费 iot.events → 按事件租户加载规则 → 纯函数 evaluate →
/// 命中则 WS 推送 + 落告警记录 + 可选 webhook；kind=anomaly（AI 异常检测
/// 产出）直接入告警流（检测器侧已做 z-score 阈值 + 冷却，无需规则匹配）。
/// ponytail: 每事件查一次规则（MVP 量级足够；量大时改为定时重载 + 内存缓存）。
pub async fn run(kafka: Arc<KafkaMq>, store: Arc<RuleStore>, bridge: AlertBridge, http: reqwest::Client) {
    let mut stream = match kafka.subscribe(TOPIC_EVENTS).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "kafka subscribe failed, rule engine exits");
            return;
        }
    };
    let mut stream = poll_fn(move |cx| stream.poll_recv(cx)).boxed();
    // 告警去重（D-1）：同一告警在窗口内只投递一次，防事件风暴重复通知
    let mut dedup = AlertDedup::new();
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
            if dedup.should_deliver(&msg) {
                deliver_alert(&store, &bridge, &http, &msg, &rules, &kafka).await;
            }
        }
        if ev.kind == "anomaly" {
            let z_threshold = std::env::var("ANOMALY_Z_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(6.0);
            let msg = AlertMessage {
                rule_id: "anomaly-detector".into(),
                rule_name: "AI 异常检测".into(),
                tenant_id: ev.tenant_id.clone(),
                device_id: ev.device_id.clone(),
                code: ev.code.clone(),
                operator: "z-score".into(),
                threshold: z_threshold,
                value: ev.value.clone(),
                ts: ev.ts,
            };
            if dedup.should_deliver(&msg) {
                deliver_alert(&store, &bridge, &http, &msg, &rules, &kafka).await;
            }
        }
    }
}

/// 告警送达流水线：WS 推送 + 落告警记录 + 规则 webhook + 联动动作（D-3）
/// + 多渠道通知。（规则 webhook 仅规则触发的告警有；anomaly-detector 无。）
async fn deliver_alert(
    store: &Arc<RuleStore>,
    bridge: &AlertBridge,
    http: &reqwest::Client,
    msg: &AlertMessage,
    rules: &[crate::models::Rule],
    kafka: &KafkaMq,
) {
    bridge.publish(&msg.tenant_id, msg).await;
    if let Err(e) = store.insert_alert(msg).await {
        tracing::warn!(error = %e, "alert record insert failed");
    }
    // T2.8: 告警同步写入 OpenSearch 索引（doc id 与 alert_records 同源；
    // 失败仅 warn，不阻断送达流水线）
    if let Some(search) = ecat_search::connect_search() {
        let id = crate::engine::to_alert_record(msg).id;
        let doc = serde_json::json!({
            "id": id,
            "rule_id": msg.rule_id,
            "tenant_id": msg.tenant_id,
            "device_id": msg.device_id,
            "code": msg.code,
            "operator": msg.operator,
            "threshold": msg.threshold,
            "value": msg.value,
            "ts": msg.ts,
            "status": "active",
        });
        if let Err(e) = search.index("alerts", &id, &doc).await {
            tracing::warn!(error = %e, "alert index failed");
        }
    }
    let rule = rules.iter().find(|r| r.id == msg.rule_id);
    // D-3 联动：命中规则配置了动作 → 发布指令事件到 iot.commands，access 消费后下发
    if let Some(dev) = rule.and_then(|r| r.action_device_id.clone()) {
        if let (Some(code), Some(value)) = (
            rule.and_then(|r| r.action_code.clone()),
            rule.and_then(|r| r.action_value.clone()),
        ) {
            let cmd = CommandEvent {
                device_id: dev,
                tenant_id: msg.tenant_id.clone(),
                code,
                value,
                ts: msg.ts,
            };
            let payload = serde_json::to_vec(&cmd).unwrap_or_default();
            if let Err(e) = kafka.publish(ecat_iot::TOPIC_COMMANDS, &payload).await {
                tracing::warn!(error = %e, "linkage action publish failed");
            }
        }
    }
    let webhook = rule.and_then(|r| r.webhook_url.clone());
    if let Some(url) = webhook {
        notify_webhook(http, &url, msg).await;
    }
    // 多渠道通知：加载租户渠道配置，异步发送，失败只记日志
    match store.list_channels(&msg.tenant_id).await {
        Ok(channels) if !channels.is_empty() => {
            let http = http.clone();
            let msg = msg.clone();
            tokio::spawn(async move {
                crate::notify::dispatch(channels, msg, http).await;
            });
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "list notify channels failed"),
    }
}

async fn notify_webhook(http: &reqwest::Client, url: &str, msg: &AlertMessage) {
    match http.post(url).body(webhook_payload(msg)).send().await {
        Ok(r) if r.status().is_success() => {}
        Ok(r) => tracing::warn!(status = %r.status().as_u16(), url = %url, "webhook non-2xx"),
        Err(e) => tracing::warn!(error = %e, url = %url, "webhook delivery failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn msg(rule: &str, dev: &str, code: &str, ts: i64) -> AlertMessage {
        AlertMessage {
            rule_id: rule.into(),
            rule_name: "t".into(),
            tenant_id: "t1".into(),
            device_id: dev.into(),
            code: code.into(),
            operator: "gt".into(),
            threshold: 1.0,
            value: json!(2),
            ts,
        }
    }

    #[test]
    fn dedup_suppresses_within_window() {
        let mut d = AlertDedup::new();
        d.window_ms = 5000;
        // 首次投递
        assert!(d.should_deliver(&msg("r1", "d1", "temp", 1000)));
        // 窗口内重复 → 抑制
        assert!(!d.should_deliver(&msg("r1", "d1", "temp", 2000)));
        // 不同 code → 放行
        assert!(d.should_deliver(&msg("r1", "d1", "hum", 2000)));
        // 不同设备 → 放行
        assert!(d.should_deliver(&msg("r1", "d2", "temp", 2000)));
        // 窗口过后 → 放行
        assert!(d.should_deliver(&msg("r1", "d1", "temp", 7000)));
    }

    #[test]
    fn dedup_disabled_when_window_zero() {
        let mut d = AlertDedup::new();
        d.window_ms = 0;
        assert!(d.should_deliver(&msg("r1", "d1", "temp", 1000)));
        assert!(d.should_deliver(&msg("r1", "d1", "temp", 1000)));
    }
}
