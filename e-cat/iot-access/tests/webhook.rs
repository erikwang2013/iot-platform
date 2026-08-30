use axum::http::StatusCode;
use iot_access::webhook::{WebhookPayload, normalize_event, verify_webhook_signature};

#[test]
fn signature_missing_header_is_401() {
    let err = verify_webhook_signature("s3cret", b"{}", None).unwrap_err();
    assert_eq!(err.0, StatusCode::UNAUTHORIZED);
}

#[test]
fn signature_empty_secret_is_403() {
    let err = verify_webhook_signature("", b"{}", Some("abc")).unwrap_err();
    assert_eq!(err.0, StatusCode::FORBIDDEN);
}

#[test]
fn signature_valid_hmac_passes_bad_sig_fails() {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(b"s3cret").unwrap();
    mac.update(b"{}");
    let sig = hex::encode(mac.finalize().into_bytes());
    assert!(verify_webhook_signature("s3cret", b"{}", Some(&sig)).is_ok());
    let bad = verify_webhook_signature("s3cret", b"{}", Some("deadbeef")).unwrap_err();
    assert_eq!(bad.0, StatusCode::FORBIDDEN);
}

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
