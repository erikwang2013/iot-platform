mod mock_azure;

use ecat_access::adapter::{VendorAdapter, VendorCreds};
use ecat_access::adapters::azure::AzureAdapter;

fn creds() -> VendorCreds {
    VendorCreds {
        client_id: mock_azure::HUB.into(), // Azure：IoT Hub host
        client_secret: mock_azure::KEY.into(), // Azure：base64 共享访问密钥
        uid: String::new(),
        access_token: String::new(), // SAS 每次请求现算
        refresh_token: String::new(),
        expires_at: 0,
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
    mock_azure::spawn();
    let _g = EnvGuard::set(&[("AZURE_IOTHUB_BASE", mock_azure::BASE)]);
    let adapter = AzureAdapter::new();
    let devs = adapter.list_devices(&creds()).await.unwrap();
    assert_eq!(devs.len(), 2);
    assert_eq!(devs[0].vendor_id, "azure-dev-1");
    assert!(devs[0].online); // connectionState=Connected
    assert!(!devs[1].online);
}

#[tokio::test]
async fn get_properties_prefers_reported() {
    mock_azure::spawn();
    let _g = EnvGuard::set(&[("AZURE_IOTHUB_BASE", mock_azure::BASE)]);
    let adapter = AzureAdapter::new();
    let props = adapter.get_properties(&creds(), "azure-dev-1").await.unwrap();
    assert_eq!(props.len(), 2);
    let get = |code: &str| props.iter().find(|p| p.code == code).unwrap();
    assert_eq!(get("temp").value, serde_json::json!(24.0));
    assert_eq!(get("humidity").value, serde_json::json!(58));
}

#[tokio::test]
async fn send_command_invokes_method() {
    mock_azure::spawn();
    let _g = EnvGuard::set(&[("AZURE_IOTHUB_BASE", mock_azure::BASE)]);
    let adapter = AzureAdapter::new();
    adapter
        .send_command(&creds(), "azure-dev-1", "reboot", serde_json::json!({"delay": 5}))
        .await
        .unwrap();
}
