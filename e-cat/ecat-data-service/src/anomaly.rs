use crate::models::EventMessage;
use ecat_mq::MessageQueue;
use ecat_mq_kafka::KafkaMq;
use futures_util::{StreamExt, stream::poll_fn};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

pub use ecat_iot::TOPIC_EVENTS;

/// 异常判定默认阈值：z-score > 6σ（正常波动误报率极低；可 ANOMALY_Z_THRESHOLD 覆盖）。
const Z_THRESHOLD: f64 = 6.0;
/// 每属性最少学习样本数，冷启动期不判异常（ANOMALY_WARMUP 可覆盖）。
const WARMUP: u64 = 30;
/// 同 (设备, 属性) 两次异常告警最小间隔（秒），防事件风暴刷屏（ANOMALY_COOLDOWN_SECS）。
const COOLDOWN_SECS: i64 = 60;

/// Welford 在线统计（均值/方差）：只存常量状态，不存历史窗口，
/// 内存占用与数据量无关。
#[derive(Debug, Clone)]
pub struct Model {
    n: u64,
    mean: f64,
    m2: f64,
}

impl Model {
    fn new() -> Self {
        Self { n: 0, mean: 0.0, m2: 0.0 }
    }

    /// 当前样本相对历史分布的 z-score；样本不足或方差为 0（恒定信号）返回 None。
    /// 判定必须基于**更新前**的分布——样本计入基线会抬高均值/方差，掩盖尖峰。
    fn z_score(&self, v: f64) -> Option<f64> {
        if self.n < WARMUP {
            return None;
        }
        let var = self.m2 / (self.n as f64 - 1.0);
        if var <= f64::EPSILON {
            return None;
        }
        Some((v - self.mean).abs() / var.sqrt())
    }

    /// 把样本计入基线（Welford 递推）。
    fn update(&mut self, v: f64) {
        self.n += 1;
        let delta = v - self.mean;
        self.mean += delta / self.n as f64;
        self.m2 += delta * (v - self.mean);
    }

    fn std_dev(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        (self.m2 / (self.n as f64 - 1.0)).sqrt()
    }
}

/// 统计异常检测器：(device_id, code) → 在线模型 + 上次告警时间戳（冷却）。
pub struct AnomalyDetector {
    models: HashMap<(String, String), (Model, i64)>,
    z_threshold: f64,
    cooldown_secs: i64,
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl AnomalyDetector {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            z_threshold: env_f64("ANOMALY_Z_THRESHOLD", Z_THRESHOLD),
            cooldown_secs: env_i64("ANOMALY_COOLDOWN_SECS", COOLDOWN_SECS),
        }
    }

    /// 处理一条 property 事件；检出异常返回告警事件（kind=anomaly），否则 None。
    pub fn feed(&mut self, ev: &EventMessage) -> Option<EventMessage> {
        let v = ev.value.as_f64()?;
        let (model, last_alert) = self
            .models
            .entry((ev.device_id.clone(), ev.code.clone()))
            .or_insert_with(|| (Model::new(), 0));
        // 冷却期内的样本视为同一异常片段的延续：既不告警也不计入基线
        // （否则持续异常会把离群点学进分布，后续尖峰无法再检出）
        if ev.ts < *last_alert + self.cooldown_secs * 1000 {
            return None;
        }
        // 判定基于更新前的历史分布；样本随后计入基线
        let z = model.z_score(v);
        let mean = model.mean;
        let std = model.std_dev();
        model.update(v);
        let Some(z) = z else { return None };
        if z <= self.z_threshold {
            return None;
        }
        *last_alert = ev.ts;
        Some(EventMessage {
            device_id: ev.device_id.clone(),
            tenant_id: ev.tenant_id.clone(),
            kind: "anomaly".into(),
            code: ev.code.clone(),
            value: json!({
                "value": v,
                "z_score": round2(z),
                "mean": round2(mean),
                "std_dev": round2(std),
            }),
            ts: ev.ts,
        })
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// 后台任务：独立消费组（iot-anomaly）消费 iot.events → 统计检测 →
/// 异常事件回写 iot.events（rule 引擎消费 kind=anomaly 入告警流，
/// ingest 落 TDengine 留痕；检测器只喂 kind=property，无自循环）。
pub async fn run(kafka: Arc<KafkaMq>) {
    let mut stream = match kafka.subscribe(TOPIC_EVENTS).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "kafka subscribe failed, anomaly detector exits");
            return;
        }
    };
    let mut stream = poll_fn(move |cx| stream.poll_recv(cx)).boxed();
    let mut detector = AnomalyDetector::new();
    while let Some(Ok(raw)) = stream.next().await {
        let ev: EventMessage = match serde_json::from_slice(&raw) {
            Ok(ev) => ev,
            Err(e) => {
                tracing::warn!(error = %e, "drop unparseable event");
                continue;
            }
        };
        if ev.kind != "property" {
            continue;
        }
        let Some(alert) = detector.feed(&ev) else { continue };
        match kafka.publish(TOPIC_EVENTS, &serde_json::to_vec(&alert).unwrap_or_default()).await {
            Ok(()) => tracing::info!(device = %alert.device_id, code = %alert.code, z_score = %alert.value["z_score"], "anomaly detected"),
            Err(e) => tracing::warn!(error = %e, "anomaly publish failed"),
        }
    }
}

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_i64(name: &str, default: i64) -> i64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prop(device: &str, code: &str, v: f64, ts: i64) -> EventMessage {
        EventMessage {
            device_id: device.into(),
            tenant_id: "t1".into(),
            kind: "property".into(),
            code: code.into(),
            value: serde_json::json!(v),
            ts,
        }
    }

    #[test]
    fn steady_signal_no_alert() {
        let mut d = AnomalyDetector::new();
        // 基线：围绕 100 的 ±1 正常波动，30 个样本
        let mut ts = 1_700_000_000_000i64;
        for i in 0..30 {
            let v = 100.0 + (i % 3) as f64;
            assert!(d.feed(&prop("dev1", "temp", v, ts)).is_none());
            ts += 1000;
        }
        // 正常波动（2σ 内）不误报
        assert!(d.feed(&prop("dev1", "temp", 102.0, ts)).is_none());
        assert!(d.feed(&prop("dev1", "temp", 98.0, ts + 1000)).is_none());
    }

    #[test]
    fn spike_detected_and_cooldown() {
        let mut d = AnomalyDetector::new();
        let mut ts = 1_700_000_000_000i64;
        for i in 0..30 {
            let v = 100.0 + (i % 3) as f64;
            d.feed(&prop("dev1", "temp", v, ts));
            ts += 1000;
        }
        // 尖峰 +40 → z-score 远超 6σ → 告警
        let alert = d.feed(&prop("dev1", "temp", 140.0, ts)).expect("spike must alert");
        assert_eq!(alert.kind, "anomaly");
        assert_eq!(alert.code, "temp");
        assert!(alert.value["z_score"].as_f64().unwrap() > 6.0);
        // 冷却期内同属性连续异常不重复告警
        assert!(d.feed(&prop("dev1", "temp", 145.0, ts + 1000)).is_none());
        // 冷却期后（>60s）再次尖峰可告警
        let alert2 = d.feed(&prop("dev1", "temp", 150.0, ts + 61_000)).expect("alert after cooldown");
        assert_eq!(alert2.code, "temp");
    }

    #[test]
    fn cold_start_no_alert() {
        let mut d = AnomalyDetector::new();
        // 前 29 个样本不判异常（含一个明显尖峰）
        let mut ts = 1_700_000_000_000i64;
        for i in 0..29 {
            let v = if i == 20 { 500.0 } else { 100.0 };
            assert!(d.feed(&prop("dev1", "temp", v, ts)).is_none());
            ts += 1000;
        }
    }

    #[test]
    fn non_numeric_skipped() {
        let mut d = AnomalyDetector::new();
        let mut ev = prop("dev1", "state", 1.0, 1_700_000_000_000);
        ev.value = serde_json::json!("on");
        assert!(d.feed(&ev).is_none());
    }
}
