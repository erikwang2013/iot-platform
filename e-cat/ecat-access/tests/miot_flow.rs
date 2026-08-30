mod mock_miot;

use ecat_access::adapter::{VendorAdapter, VendorCreds};
use ecat_access::adapters::miot::{MiAdapter, sign as miot_sign};

fn creds(access_token: &str) -> VendorCreds {
    VendorCreds {
        client_id: mock_miot::CLIENT_ID.into(),
        client_secret: mock_miot::CLIENT_SECRET.into(),
        uid: "mock-uid-1".into(),
        access_token: access_token.into(),
        refresh_token: "mock-rt-1".into(),
        expires_at: 0, // 强制走刷新路径
    }
}

// 与 tuya_flow.rs 同款进程级环境变量锁（并行测试共享 env 会竞态）。
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    saved: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn set(kv: &[(&'static str, &str)]) -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved = kv.iter().map(|&(k, _)| (k, std::env::var(k).ok())).collect();
        for &(k, v) in kv {
            unsafe { std::env::set_var(k, v) };
        }
        EnvGuard { _lock: lock, saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, old) in &self.saved {
            match old {
                Some(v) => unsafe { std::env::set_var(k, v) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
    }
}

#[tokio::test]
async fn list_devices_maps_fields() {
    mock_miot::spawn();
    let _g = EnvGuard::set(&[("MIOT_OPENAPI_BASE", mock_miot::BASE)]);
    let adapter = MiAdapter::new();
    let devs = adapter.list_devices(&creds("at-1")).await.unwrap();
    assert_eq!(devs.len(), 2);
    assert_eq!(devs[0].vendor_id, "miot-dev-1");
    assert_eq!(devs[0].name, "mock-temp-sensor");
    assert_eq!(devs[0].category, "xiaomi.temp.v1");
    assert!(devs[0].online);
    assert!(!devs[1].online);
}

#[tokio::test]
async fn get_properties_parses_status() {
    mock_miot::spawn();
    let _g = EnvGuard::set(&[("MIOT_OPENAPI_BASE", mock_miot::BASE)]);
    let adapter = MiAdapter::new();
    let props = adapter.get_properties(&creds("at-1"), "miot-dev-1").await.unwrap();
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].code, "temp");
    assert_eq!(props[0].value, serde_json::json!(25.0));
}

#[tokio::test]
async fn send_command_succeeds() {
    mock_miot::spawn();
    let _g = EnvGuard::set(&[("MIOT_OPENAPI_BASE", mock_miot::BASE)]);
    let adapter = MiAdapter::new();
    adapter
        .send_command(&creds("at-1"), "miot-dev-1", "power", serde_json::json!(true))
        .await
        .unwrap();
}

#[tokio::test]
async fn expired_token_triggers_refresh_and_retry() {
    mock_miot::spawn();
    let _g = EnvGuard::set(&[("MIOT_OPENAPI_BASE", mock_miot::BASE)]);
    let adapter = MiAdapter::new();
    // 签名以 "expired" 为 token → mock 返回 400006 → 适配器刷新后重试成功
    let devs = adapter.list_devices(&creds("expired")).await.unwrap();
    assert_eq!(devs.len(), 2);
}

#[tokio::test]
async fn sign_is_uppercase_hmac() {
    let s = miot_sign("mock-miot-client", "1690000000000", "at-1", "mock-miot-secret");
    assert_eq!(s, mock_miot::sign("mock-miot-secret", "mock-miot-client", "1690000000000", "at-1"));
    assert_eq!(s, s.to_uppercase());
}
