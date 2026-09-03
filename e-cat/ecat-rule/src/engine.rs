use crate::models::{AlertMessage, AlertRecord, EventMessage, Rule};
use ecat_mq_kafka::KafkaConfig;

pub use ecat_iot::TOPIC_EVENTS;

/// iot-rule 消费组：与 iot-data（未配置 group_id → 每次订阅随机独立组）不同，
/// 互不抢消息；同组多实例共享消费负载（水平扩展点）。
pub fn kafka_config(brokers: &str) -> KafkaConfig {
    KafkaConfig {
        brokers: brokers.to_string(),
        group_id: Some("iot-rule-rules".into()),
        auto_commit: false,
        security_protocol: None,
        sasl_mechanism: None,
        sasl_username: None,
        sasl_password: None,
    }
}

fn compare(value: f64, operator: &str, threshold: f64) -> bool {
    match operator {
        "gt" => value > threshold,
        "gte" => value >= threshold,
        "lt" => value < threshold,
        "lte" => value <= threshold,
        "eq" => (value - threshold).abs() < f64::EPSILON,
        "neq" => (value - threshold).abs() >= f64::EPSILON,
        _ => false,
    }
}

/// 事件 → 命中规则 → 告警消息。纯函数：Kafka 消费只是适配层，引擎可纯内存测试。
/// 只评估 kind=property 的数值事件；非数值值不匹配任何规则。
pub fn evaluate(ev: &EventMessage, rules: &[Rule]) -> Vec<AlertMessage> {
    if ev.kind != "property" {
        return Vec::new();
    }
    let Some(v) = ev.value.as_f64() else {
        return Vec::new();
    };
    rules
        .iter()
        .filter(|r| {
            r.enabled
                && r.tenant_id == ev.tenant_id
                && r.device_id == ev.device_id
                && r.code == ev.code
                && compare(v, &r.operator, r.threshold)
        })
        .map(|r| AlertMessage {
            rule_id: r.id.clone(),
            rule_name: r.name.clone(),
            tenant_id: r.tenant_id.clone(),
            device_id: r.device_id.clone(),
            code: r.code.clone(),
            operator: r.operator.clone(),
            threshold: r.threshold,
            value: ev.value.clone(),
            ts: ev.ts,
        })
        .collect()
}

/// 告警消息 → 落库记录（status 固定 active，created_at 由 DB 生成）。
pub fn to_alert_record(msg: &AlertMessage) -> AlertRecord {
    AlertRecord {
        id: ecat::ids::next_id().to_string(),
        rule_id: msg.rule_id.clone(),
        tenant_id: msg.tenant_id.clone(),
        device_id: msg.device_id.clone(),
        code: msg.code.clone(),
        operator: msg.operator.clone(),
        threshold: msg.threshold,
        value: msg.value.clone(),
        status: "active".into(),
        created_at: String::new(),
    }
}
