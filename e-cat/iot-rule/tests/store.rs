use iot_rule::models::NewRule;
use iot_rule::store::{OPERATORS, validate_rule};

fn new_rule() -> NewRule {
    NewRule {
        name: "高温告警".into(),
        device_id: "d1".into(),
        code: "temp".into(),
        operator: "gt".into(),
        threshold: 30.0,
        webhook_url: None,
        enabled: Some(true),
    }
}

#[test]
fn valid_rule_passes() {
    assert!(validate_rule(&new_rule()).is_ok());
}

#[test]
fn operator_whitelist_enforced() {
    for op in OPERATORS {
        let mut r = new_rule();
        r.operator = op.to_string();
        assert!(validate_rule(&r).is_ok(), "operator {op} 应合法");
    }
    let mut r = new_rule();
    r.operator = "lteq".into();
    assert!(validate_rule(&r).is_err());
    let mut r = new_rule();
    r.operator = "gt OR 1=1".into();
    assert!(validate_rule(&r).is_err(), "注入载荷不得通过");
}

#[test]
fn rejects_empty_fields_and_bad_webhook() {
    let mut r = new_rule();
    r.device_id.clear();
    assert!(validate_rule(&r).is_err());
    let mut r = new_rule();
    r.webhook_url = Some("javascript:alert(1)".into());
    assert!(validate_rule(&r).is_err());
}

#[test]
fn rejects_non_finite_threshold() {
    let mut r = new_rule();
    r.threshold = f64::NAN;
    assert!(validate_rule(&r).is_err());
}
