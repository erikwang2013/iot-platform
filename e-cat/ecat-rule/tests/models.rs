use ecat_rule::models::{AlertMessage, EventMessage, NewRule, Rule};
use serde_json::json;

#[test]
fn deserializes_p1_kafka_shape() {
    // 形状必须与 P1 iot-access/src/models.rs 的 EventMessage 序列化输出一致
    let raw = br#"{"device_id":"d1","tenant_id":"t1","kind":"property","code":"temp","value":23.5,"ts":1690000000000}"#;
    let ev: EventMessage = serde_json::from_slice(raw).unwrap();
    assert_eq!(ev.device_id, "d1");
    assert_eq!(ev.tenant_id, "t1");
    assert_eq!(ev.kind, "property");
    assert_eq!(ev.code, "temp");
    assert_eq!(ev.value, json!(23.5));
    assert_eq!(ev.ts, 1690000000000);
}

#[test]
fn rejects_missing_fields() {
    let raw = br#"{"device_id":"d1","kind":"property"}"#;
    assert!(serde_json::from_slice::<EventMessage>(raw).is_err());
}

#[test]
fn new_rule_deserializes_body() {
    let raw = r#"{"name":"高温告警","device_id":"d1","code":"temp","operator":"gt","threshold":30.5,"webhook_url":"https://x.example/hook","enabled":true}"#;
    let r: NewRule = serde_json::from_str(raw).unwrap();
    assert_eq!(r.operator, "gt");
    assert_eq!(r.threshold, 30.5);
    assert_eq!(r.webhook_url.as_deref(), Some("https://x.example/hook"));
    assert_eq!(r.enabled, Some(true));
}

#[test]
fn rule_serializes_roundtrip() {
    let r = Rule {
        id: "r1".into(),
        tenant_id: "t1".into(),
        name: "高温告警".into(),
        device_id: "d1".into(),
        code: "temp".into(),
        operator: "gt".into(),
        threshold: 30.0,
        webhook_url: None,
        action_device_id: None,
        action_code: None,
        action_value: None,
        enabled: true,
        created_at: String::new(),
        updated_at: String::new(),
    };
    let back: Rule = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(back, r);
}

#[test]
fn alert_message_serializes_ws_payload() {
    let msg = AlertMessage {
        rule_id: "r1".into(),
        rule_name: "高温告警".into(),
        tenant_id: "t1".into(),
        device_id: "d1".into(),
        code: "temp".into(),
        operator: "gt".into(),
        threshold: 30.0,
        value: json!(35.2),
        ts: 1690000000000,
    };
    let s = serde_json::to_string(&msg).unwrap();
    assert!(s.contains("\"rule_id\":\"r1\""));
    assert!(s.contains("\"value\":35.2"));
    assert!(s.contains("\"ts\":1690000000000"));
}
