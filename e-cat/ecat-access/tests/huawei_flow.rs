mod mock_huawei;

use ecat_access::adapter::{VendorAdapter, VendorCreds};
use ecat_access::adapters::huawei::HuaweiAdapter;

fn creds() -> VendorCreds {
    VendorCreds {
        client_id: mock_huawei::AK.into(),
        client_secret: mock_huawei::SK.into(),
        uid: mock_huawei::PROJECT_ID.into(), // 华为：project_id
        access_token: String::new(),         // AK/SK 型，无 token
        refresh_token: String::new(),
        expires_at: 0, // AK/SK 不过期
    }
}

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
    mock_huawei::spawn();
    let _g = EnvGuard::set(&[("HUAWEI_IOTDA_BASE", mock_huawei::BASE)]);
    let adapter = HuaweiAdapter::new();
    let devs = adapter.list_devices(&creds()).await.unwrap();
    assert_eq!(devs.len(), 2);
    assert_eq!(devs[0].vendor_id, "huawei-dev-1");
    assert_eq!(devs[0].name, "mock-temp-sensor");
    assert_eq!(devs[0].category, "temp_sensor");
    assert!(devs[0].online);
    assert!(!devs[1].online);
}

#[tokio::test]
async fn get_properties_flattens_shadow() {
    mock_huawei::spawn();
    let _g = EnvGuard::set(&[("HUAWEI_IOTDA_BASE", mock_huawei::BASE)]);
    let adapter = HuaweiAdapter::new();
    let props = adapter.get_properties(&creds(), "huawei-dev-1").await.unwrap();
    assert_eq!(props.len(), 2);
    let get = |code: &str| props.iter().find(|p| p.code == code).unwrap();
    assert_eq!(get("sensor.temp").value, serde_json::json!(25.0));
    assert_eq!(get("sensor.humidity").value, serde_json::json!(60));
}

#[tokio::test]
async fn send_command_succeeds() {
    mock_huawei::spawn();
    let _g = EnvGuard::set(&[("HUAWEI_IOTDA_BASE", mock_huawei::BASE)]);
    let adapter = HuaweiAdapter::new();
    adapter
        .send_command(&creds(), "huawei-dev-1", "reboot", serde_json::json!({}))
        .await
        .unwrap();
}
