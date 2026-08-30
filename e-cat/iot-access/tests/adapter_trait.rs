use async_trait::async_trait;
use ecat_access::adapter::{AdapterError, VendorAdapter, VendorCreds};
use ecat_access::models::{DeviceRecord, EventMessage, PropertyValue};
use serde_json::json;

struct Dummy;

#[async_trait]
impl VendorAdapter for Dummy {
    async fn list_devices(&self, _c: &VendorCreds) -> Result<Vec<DeviceRecord>, AdapterError> {
        Ok(vec![DeviceRecord {
            id: "dev-1".into(),
            vendor_id: "tuya-dev-1".into(),
            name: "sensor".into(),
            category: "temp".into(),
            online: true,
            properties: vec![PropertyValue { code: "temp".into(), value: json!(23.5) }],
        }])
    }
    async fn get_properties(
        &self,
        _c: &VendorCreds,
        _vendor_id: &str,
    ) -> Result<Vec<PropertyValue>, AdapterError> {
        Ok(vec![])
    }
    async fn send_command(
        &self,
        _c: &VendorCreds,
        _vendor_id: &str,
        _code: &str,
        _value: serde_json::Value,
    ) -> Result<(), AdapterError> {
        Ok(())
    }
    async fn subscribe_events(&self, _c: &VendorCreds) -> Result<(), AdapterError> {
        Ok(())
    }
}

#[tokio::test]
async fn trait_object_roundtrip() {
    let adapter: Box<dyn VendorAdapter> = Box::new(Dummy);
    let creds = VendorCreds {
        client_id: "c".into(),
        client_secret: "s".into(),
        uid: "u".into(),
        access_token: "at".into(),
        refresh_token: "rt".into(),
        expires_at: 0,
    };
    let devs = adapter.list_devices(&creds).await.unwrap();
    assert_eq!(devs.len(), 1);
    assert_eq!(devs[0].properties[0].value, json!(23.5));
}

#[test]
fn event_message_json_shape() {
    let ev = EventMessage {
        device_id: "d1".into(),
        tenant_id: "t1".into(),
        kind: "property".into(),
        code: "temp".into(),
        value: json!(23.5),
        ts: 1690000000000,
    };
    let s = serde_json::to_string(&ev).unwrap();
    // 字段名固定：Kafka 消费者（P2 iot-data）依赖此形状
    assert!(s.contains("\"device_id\":\"d1\""));
    assert!(s.contains("\"kind\":\"property\""));
    assert!(s.contains("\"ts\":1690000000000"));
}
