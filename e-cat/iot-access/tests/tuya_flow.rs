mod mock_tuya;

use iot_access::adapter::{VendorAdapter, VendorCreds};
use iot_access::adapters::tuya::TuyaAdapter;

fn creds(access_token: &str, refresh_token: &str) -> VendorCreds {
    VendorCreds {
        client_id: mock_tuya::CLIENT_ID.into(),
        client_secret: mock_tuya::CLIENT_SECRET.into(),
        uid: "mock-uid-1".into(),
        access_token: access_token.into(),
        refresh_token: refresh_token.into(),
        expires_at: 0, // 强制 maybe_refresh 走刷新路径
    }
}

// 并行测试共享进程级环境变量，若各自设置/恢复会造成竞态：
// 先结束的测试把旧值写回时，另一个测试的请求会读到旧值（打到真实 API）。
// 故持同一把锁设置，drop 时恢复并释放锁（测试实际串行化，均瞬时完成）。
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// RAII：设置环境变量，drop 时恢复旧值，避免污染其他测试。
struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    saved: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn set(kv: &[(&'static str, &str)]) -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved = kv
            .iter()
            .map(|&(k, _)| (k, std::env::var(k).ok()))
            .collect();
        for &(k, v) in kv {
            unsafe { std::env::set_var(k, v) }; // 2024 edition: set_var 为 unsafe fn
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
async fn full_oauth_and_device_flow() {
    mock_tuya::spawn();
    // 环境变量指向 mock，测试结束自动恢复
    let _guard = EnvGuard::set(&[
        ("TUYA_OPENAPI_BASE", mock_tuya::BASE),
        ("TUYA_CLIENT_SECRET", mock_tuya::CLIENT_SECRET),
    ]);

    // 1. 授权码换 token（与 oauth::exchange_authorization_code 等价流程）
    let adapter = TuyaAdapter::new();
    let c = creds("mock-at-tuya-dev-1", "mock-rt-tuya-dev-1");
    let refreshed = adapter.refresh_token(&c).await.unwrap();
    assert!(refreshed.access_token.starts_with("mock-at-refreshed-"));

    // 2. 拉设备列表（签名正确 → mock 返回 2 台）
    let devs = adapter.list_devices(&refreshed).await.unwrap();
    assert_eq!(devs.len(), 2);
    assert_eq!(devs[0].vendor_id, "tuya-dev-1");
    assert_eq!(devs[0].properties[0].value, serde_json::json!(23.5));

    // 3. 单设备属性
    let props = adapter.get_properties(&refreshed, "tuya-dev-1").await.unwrap();
    assert_eq!(props[0].code, "temp");

    // 4. 指令下发
    adapter
        .send_command(&refreshed, "tuya-dev-1", "temp", serde_json::json!(26.0))
        .await
        .unwrap();
}

#[tokio::test]
async fn expired_token_auto_refresh_on_list() {
    mock_tuya::spawn();
    let _guard = EnvGuard::set(&[("TUYA_OPENAPI_BASE", mock_tuya::BASE)]);
    let adapter = TuyaAdapter::new();
    // expires_at=0 → get 前先刷新，再带新 token 请求
    let c = creds("mock-at-expired", "mock-rt-expired");
    let devs = adapter.list_devices(&c).await.unwrap();
    assert_eq!(devs.len(), 2);
}
