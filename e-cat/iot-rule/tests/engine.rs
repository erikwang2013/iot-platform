use iot_rule::engine::{TOPIC_EVENTS, evaluate, kafka_config, to_alert_record};
use iot_rule::models::{EventMessage, Rule};
use serde_json::json;

fn rule(id: &str, tenant: &str, device: &str, code: &str, op: &str, th: f64, enabled: bool) -> Rule {
    Rule {
        id: id.into(),
        tenant_id: tenant.into(),
        name: format!("rule-{id}"),
        device_id: device.into(),
        code: code.into(),
        operator: op.into(),
        threshold: th,
        webhook_url: None,
        enabled,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn event(tenant: &str, device: &str, code: &str, value: serde_json::Value, ts: i64) -> EventMessage {
    EventMessage {
        device_id: device.into(),
        tenant_id: tenant.into(),
        kind: "property".into(),
        code: code.into(),
        value,
        ts,
    }
}

#[test]
fn topic_matches_p1_events_bus() {
    assert_eq!(TOPIC_EVENTS, "iot.events");
}

#[test]
fn gt_matches_and_builds_alert() {
    let rules = vec![rule("r1", "t1", "d1", "temp", "gt", 30.0, true)];
    let msgs = evaluate(&event("t1", "d1", "temp", json!(35.0), 1690000000000), &rules);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].rule_id, "r1");
    assert_eq!(msgs[0].rule_name, "rule-r1");
    assert_eq!(msgs[0].value, json!(35.0));
    assert_eq!(msgs[0].ts, 1690000000000);
}

#[test]
fn all_operators_compare_correctly() {
    let mk = |op: &str, th: f64| evaluate(&event("t", "d", "c", json!(10.0), 1), &[rule("r", "t", "d", "c", op, th, true)]);
    assert_eq!(mk("gt", 9.0).len(), 1);
    assert_eq!(mk("gt", 10.0).len(), 0);
    assert_eq!(mk("gte", 10.0).len(), 1);
    assert_eq!(mk("lt", 11.0).len(), 1);
    assert_eq!(mk("lt", 10.0).len(), 0);
    assert_eq!(mk("lte", 10.0).len(), 1);
    assert_eq!(mk("eq", 10.0).len(), 1);
    assert_eq!(mk("eq", 10.0001).len(), 0);
    assert_eq!(mk("neq", 10.0001).len(), 1);
    assert_eq!(mk("neq", 10.0).len(), 0);
}

#[test]
fn non_numeric_value_never_matches() {
    let rules = vec![rule("r1", "t1", "d1", "temp", "gt", 30.0, true)];
    assert!(evaluate(&event("t1", "d1", "temp", json!("hot"), 1), &rules).is_empty());
    assert!(evaluate(&event("t1", "d1", "temp", json!(true), 1), &rules).is_empty());
}

#[test]
fn tenant_device_code_filtered() {
    let rules = vec![rule("r1", "t1", "d1", "temp", "gt", 30.0, true)];
    assert!(evaluate(&event("t2", "d1", "temp", json!(99.0), 1), &rules).is_empty());
    assert!(evaluate(&event("t1", "d2", "temp", json!(99.0), 1), &rules).is_empty());
    assert!(evaluate(&event("t1", "d1", "hum", json!(99.0), 1), &rules).is_empty());
}

#[test]
fn disabled_rule_and_non_property_event_ignored() {
    let rules = vec![rule("r1", "t1", "d1", "temp", "gt", 30.0, false)];
    assert!(evaluate(&event("t1", "d1", "temp", json!(99.0), 1), &rules).is_empty());
    let mut ev = event("t1", "d1", "online", json!(true), 1);
    ev.kind = "online".into();
    assert!(evaluate(&ev, &[rule("r1", "t1", "d1", "online", "eq", 1.0, true)]).is_empty());
}

#[test]
fn to_alert_record_carries_fields_and_active_status() {
    let rules = vec![rule("r1", "t1", "d1", "temp", "gt", 30.0, true)];
    let msgs = evaluate(&event("t1", "d1", "temp", json!(35.0), 1690000000000), &rules);
    let rec = to_alert_record(&msgs[0]);
    assert_eq!(rec.rule_id, "r1");
    assert_eq!(rec.status, "active");
    assert_eq!(rec.value, json!(35.0));
    assert!(!rec.id.is_empty());
}

#[test]
fn kafka_consumer_group_is_distinct_from_data_service() {
    // iot-data 用 KafkaMq::connect（group_id=None → 随机独立组）；
    // iot-rule 必须显式组名，同组多实例共享消费负载
    let cfg = kafka_config("localhost:9092");
    assert_eq!(cfg.group_id.as_deref(), Some("iot-rule-rules"));
}
