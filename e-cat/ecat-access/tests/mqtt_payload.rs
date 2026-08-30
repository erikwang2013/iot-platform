use ecat_access::mqtt::parse_payload;

#[test]
fn payload_with_code_value_ts() {
    let ev = parse_payload("dev-1", "t1", br#"{"code":"temp","value":23.5,"ts":1690000000000}"#)
        .unwrap();
    assert_eq!(ev.device_id, "dev-1");
    assert_eq!(ev.tenant_id, "t1");
    assert_eq!(ev.kind, "property");
    assert_eq!(ev.code, "temp");
    assert_eq!(ev.value, serde_json::json!(23.5));
    assert_eq!(ev.ts, 1690000000000);
}

#[test]
fn payload_without_ts_uses_now() {
    let ev = parse_payload("dev-1", "t1", br#"{"code":"switch","value":true}"#).unwrap();
    assert!(ev.ts > 1_700_000_000_000);
}

#[test]
fn bad_json_is_error() {
    assert!(parse_payload("dev-1", "t1", b"not json").is_err());
}
