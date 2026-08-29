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

#[tokio::test]
async fn full_oauth_and_device_flow() {
    mock_tuya::spawn().await;
    // 环境变量指向 mock（2024 edition 中 set_var 为 unsafe fn）
    unsafe {
        std::env::set_var("TUYA_OPENAPI_BASE", mock_tuya::BASE);
        std::env::set_var("TUYA_CLIENT_SECRET", mock_tuya::CLIENT_SECRET);
    }

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
    mock_tuya::spawn().await;
    unsafe { std::env::set_var("TUYA_OPENAPI_BASE", mock_tuya::BASE) };
    let adapter = TuyaAdapter::new();
    // expires_at=0 → get 前先刷新，再带新 token 请求
    let c = creds("mock-at-expired", "mock-rt-expired");
    let devs = adapter.list_devices(&c).await.unwrap();
    assert_eq!(devs.len(), 2);
}
