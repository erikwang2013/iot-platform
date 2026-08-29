use iot_access::webhook::{normalize_event, WebhookPayload};

#[test]
fn data_as_json_string_is_normalized() {
    let p = WebhookPayload {
        r#type: "deviceData".into(),
        biz_code: "report".into(),
        data: serde_json::json!(
            "{\"deviceId\":\"tuya-dev-1\",\"code\":\"temp\",\"value\":23.5,\"ts\":1690000000000}"
        ),
    };
    let ev = normalize_event("plat-dev-1", "t1", &p).unwrap();
    assert_eq!(ev.device_id, "plat-dev-1");
    assert_eq!(ev.tenant_id, "t1");
    assert_eq!(ev.kind, "property");
    assert_eq!(ev.code, "temp");
    assert_eq!(ev.value, serde_json::json!(23.5));
    assert_eq!(ev.ts, 1690000000000);
}

#[test]
fn data_as_object_is_normalized() {
    let p = WebhookPayload {
        r#type: "deviceData".into(),
        biz_code: "online".into(),
        data: serde_json::json!({"deviceId": "tuya-dev-1"}),
    };
    let ev = normalize_event("plat-dev-1", "t1", &p).unwrap();
    assert_eq!(ev.kind, "online");
}

#[test]
fn unknown_bizcode_is_error() {
    let p = WebhookPayload {
        r#type: "deviceData".into(),
        biz_code: "delete".into(),
        data: serde_json::json!({"deviceId": "tuya-dev-1"}),
    };
    assert!(normalize_event("d", "t", &p).is_err());
}
