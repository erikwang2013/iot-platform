mod mock_aws;

use ecat_access::adapter::{VendorAdapter, VendorCreds};
use ecat_access::adapters::aws::AwsAdapter;

fn creds() -> VendorCreds {
    VendorCreds {
        client_id: mock_aws::AK.into(),
        client_secret: mock_aws::SK.into(),
        uid: mock_aws::REGION.into(), // AWS：region
        access_token: String::new(),  // AK/SK 型，无 token
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
    mock_aws::spawn();
    let _g = EnvGuard::set(&[("AWS_IOT_BASE", mock_aws::BASE)]);
    let adapter = AwsAdapter::new();
    let devs = adapter.list_devices(&creds()).await.unwrap();
    assert_eq!(devs.len(), 2);
    assert_eq!(devs[0].vendor_id, "aws-dev-1");
    assert_eq!(devs[0].name, "mock-temp-sensor"); // attributes.name 优先
}

#[tokio::test]
async fn get_properties_prefers_reported() {
    mock_aws::spawn();
    let _g = EnvGuard::set(&[("AWS_IOT_BASE", mock_aws::BASE)]);
    let adapter = AwsAdapter::new();
    let props = adapter.get_properties(&creds(), "aws-dev-1").await.unwrap();
    assert_eq!(props.len(), 2);
    let get = |code: &str| props.iter().find(|p| p.code == code).unwrap();
    assert_eq!(get("temp").value, serde_json::json!(24.5)); // reported 优先于 desired
    assert_eq!(get("humidity").value, serde_json::json!(55));
}

#[tokio::test]
async fn send_command_writes_desired_state() {
    mock_aws::spawn();
    let _g = EnvGuard::set(&[("AWS_IOT_BASE", mock_aws::BASE)]);
    let adapter = AwsAdapter::new();
    adapter
        .send_command(&creds(), "aws-dev-1", "power", serde_json::json!(true))
        .await
        .unwrap();
}
