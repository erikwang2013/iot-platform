use iot_rule::models::AlertMessage;
use iot_rule::runner::webhook_payload;
use serde_json::json;

#[test]
fn webhook_payload_is_alert_message_json() {
    let msg = AlertMessage {
        rule_id: "r1".into(),
        rule_name: "高温告警".into(),
        tenant_id: "t1".into(),
        device_id: "d1".into(),
        code: "temp".into(),
        operator: "gt".into(),
        threshold: 30.0,
        value: json!(35.0),
        ts: 1690000000000,
    };
    let v: serde_json::Value = serde_json::from_slice(&webhook_payload(&msg)).unwrap();
    assert_eq!(v["rule_id"], "r1");
    assert_eq!(v["value"], 35.0);
    assert_eq!(v["ts"], 1690000000000_i64);
}
