use ecat_rule::models::AlertMessage;
use ecat_rule::push::PushHub;
use serde_json::json;

fn msg(tenant: &str) -> AlertMessage {
    AlertMessage {
        rule_id: "r1".into(),
        rule_name: "告警".into(),
        tenant_id: tenant.into(),
        device_id: "d1".into(),
        code: "temp".into(),
        operator: "gt".into(),
        threshold: 30.0,
        value: json!(35.0),
        ts: 1,
    }
}

#[test]
fn subscriber_receives_published_alert() {
    let hub = PushHub::new();
    let mut rx = hub.subscribe("t1");
    hub.publish("t1", &msg("t1"));
    let got = rx.try_recv().unwrap();
    assert_eq!(got.tenant_id, "t1");
    assert_eq!(got.value, json!(35.0));
}

#[test]
fn tenants_are_isolated() {
    let hub = PushHub::new();
    let mut rx = hub.subscribe("t1");
    hub.publish("t2", &msg("t2"));
    assert!(rx.try_recv().is_err(), "跨租户消息不得送达");
}

#[test]
fn publish_without_subscribers_is_noop() {
    let hub = PushHub::new();
    hub.publish("t1", &msg("t1"));
}

#[test]
fn multiple_subscribers_all_receive() {
    let hub = PushHub::new();
    let mut a = hub.subscribe("t1");
    let mut b = hub.subscribe("t1");
    hub.publish("t1", &msg("t1"));
    assert_eq!(a.try_recv().unwrap().rule_id, "r1");
    assert_eq!(b.try_recv().unwrap().rule_id, "r1");
}
