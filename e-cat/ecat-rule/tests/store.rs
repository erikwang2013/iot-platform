use ecat_rule::models::NewRule;
use ecat_rule::store::{OPERATORS, validate_channel, validate_rule};
use serde_json::json;

fn new_rule() -> NewRule {
    NewRule {
        name: "高温告警".into(),
        // 平台设备 id 为 snowflake i64（十进制字符串）
        device_id: "12345678901234567".into(),
        code: "temp".into(),
        operator: "gt".into(),
        threshold: 30.0,
        webhook_url: None,
        action_device_id: None,
        action_code: None,
        action_value: None,
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

fn email_cfg() -> serde_json::Value {
    json!({
        "smtp_host": "smtp.example.com",
        "smtp_port": 587,
        "smtp_user": "u",
        "smtp_pass": "p",
        "mail_from": "a@example.com",
        "mail_to": "b@example.com",
    })
}

#[test]
fn channel_whitelist_enforced() {
    let webhook = json!({ "webhook_url": "https://oapi.dingtalk.com/robot/send?access_token=x" });
    assert!(validate_channel("email", &email_cfg()).is_ok());
    assert!(validate_channel("dingtalk", &webhook).is_ok());
    assert!(validate_channel("wecom", &webhook).is_ok());
    assert!(validate_channel("sms", &webhook).is_err());
    assert!(validate_channel("email OR 1=1", &webhook).is_err(), "注入载荷不得通过");
}

#[test]
fn email_channel_requires_smtp_fields() {
    let mut c = email_cfg();
    c.as_object_mut().unwrap().remove("smtp_host");
    assert!(validate_channel("email", &c).is_err());
    let mut c = email_cfg();
    c.as_object_mut().unwrap().insert("smtp_port".into(), json!(70000));
    assert!(validate_channel("email", &c).is_err());
    let mut c = email_cfg();
    c.as_object_mut().unwrap().insert("mail_to".into(), json!("not-an-email"));
    assert!(validate_channel("email", &c).is_err());
}

#[test]
fn webhook_channel_requires_http_url() {
    let ok = json!({ "webhook_url": "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=k" });
    assert!(validate_channel("dingtalk", &ok).is_ok());
    assert!(validate_channel("wecom", &ok).is_ok());
    let bad = json!({ "webhook_url": "javascript:alert(1)" });
    assert!(validate_channel("wecom", &bad).is_err());
    let no_url = json!({});
    assert!(validate_channel("wecom", &no_url).is_err());
}

#[test]
fn channel_config_must_be_object() {
    assert!(validate_channel("wecom", &json!(42)).is_err());
}
