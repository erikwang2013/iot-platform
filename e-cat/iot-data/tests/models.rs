use ecat_data_service::models::EventMessage;

#[test]
fn deserializes_p1_kafka_shape() {
    // 形状必须与 P1 iot-access/src/models.rs 的 EventMessage 序列化输出一致
    let raw = br#"{"device_id":"d1","tenant_id":"t1","kind":"property","code":"temp","value":23.5,"ts":1690000000000}"#;
    let ev: EventMessage = serde_json::from_slice(raw).unwrap();
    assert_eq!(ev.device_id, "d1");
    assert_eq!(ev.tenant_id, "t1");
    assert_eq!(ev.kind, "property");
    assert_eq!(ev.code, "temp");
    assert_eq!(ev.value, serde_json::json!(23.5));
    assert_eq!(ev.ts, 1690000000000);
}

#[test]
fn deserializes_online_event() {
    let raw = br#"{"device_id":"d2","tenant_id":"t1","kind":"online","code":"online","value":true,"ts":1690000000001}"#;
    let ev: EventMessage = serde_json::from_slice(raw).unwrap();
    assert_eq!(ev.kind, "online");
    assert_eq!(ev.value, serde_json::json!(true));
}

#[test]
fn rejects_missing_fields() {
    let raw = br#"{"device_id":"d1","kind":"property"}"#;
    assert!(serde_json::from_slice::<EventMessage>(raw).is_err());
}
