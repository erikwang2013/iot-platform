//! Azure IoT Hub mock：绑定 127.0.0.1:18088，独立实现 SAS 令牌校验
//! （HMAC-SHA256(base64-decoded key, "sr=..&skn=..&se=..")），校验通过后返回固定数据。
use axum::{Json, Router, extract::{Path, State}, http::HeaderMap, routing::{get, post}};
use serde_json::{Value, json};

pub const BASE: &str = "http://127.0.0.1:18088";
pub const HUB: &str = "127.0.0.1:18088";
pub const KEY: &str = "mock-azure-key"; // base64 编码的共享访问密钥

fn b64_decode(s: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s).unwrap_or_default()
}

fn b64_encode(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn hmac_sha256(key: &[u8], msg: &str) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(msg.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// 独立 SAS 校验：sr 取请求 Host，重算 sig 比对。
fn verify(headers: &HeaderMap) -> bool {
    let auth = match headers.get("authorization").and_then(|v| v.to_str().ok()) {
        Some(a) => a,
        None => return false,
    };
    if !auth.starts_with("SharedAccessSignature") {
        return false;
    }
    let get = |k: &str| {
        auth.split_once(' ')
            .map(|(_, rest)| rest)
            .unwrap_or("")
            .split('&')
            .find(|p| p.starts_with(&format!("{k}=")))
            .map(|p| p.trim_start_matches(&format!("{k}=")).to_string())
            .unwrap_or_default()
    };
    let sr = get("sr");
    let sig = get("sig");
    let se = get("se");
    let skn = get("skn");
    let host = headers.get("host").and_then(|v| v.to_str().ok()).unwrap_or("");
    if sr != host || sig.is_empty() || se.is_empty() {
        return false;
    }
    let sts = format!("sr={sr}&skn={skn}&se={se}");
    let expected = b64_encode(&hmac_sha256(&b64_decode(KEY), &sts));
    expected == sig
}

async fn devices(State(_): State<MockState>, headers: HeaderMap) -> Json<Value> {
    if !verify(&headers) {
        return Json(json!({"Message": "Invalid authorization"}));
    }
    Json(json!([
        {"deviceId": "azure-dev-1", "status": "enabled", "connectionState": "Connected"},
        {"deviceId": "azure-dev-2", "status": "enabled", "connectionState": "Disconnected"}
    ]))
}

async fn twin(State(_): State<MockState>, Path(_id): Path<String>, headers: HeaderMap) -> Json<Value> {
    if !verify(&headers) {
        return Json(json!({"Message": "Invalid authorization"}));
    }
    Json(json!({
        "deviceId": "azure-dev-1",
        "properties": {
            "desired": {"temp": 25.0},
            "reported": {"temp": 24.0, "humidity": 58}
        }
    }))
}

async fn method(State(_): State<MockState>, Path(_id): Path<String>, headers: HeaderMap, _body: axum::body::Bytes) -> Json<Value> {
    if !verify(&headers) {
        return Json(json!({"Message": "Invalid authorization"}));
    }
    Json(json!({"status": 200, "payload": {}}))
}

#[derive(Clone)]
struct MockState;

static MOCK: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn spawn() {
    MOCK.get_or_init(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:18088")
            .unwrap_or_else(|e| panic!("mock azure: bind 127.0.0.1:18088 failed (端口被其他进程占用？): {e}"));
        listener.set_nonblocking(true).unwrap();
        let router = Router::new()
            .route("/devices", get(devices))
            .route("/twins/{id}", get(twin))
            .route("/devices/{id}/methods", post(method))
            .with_state(MockState);
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                axum::serve(listener, router).await.unwrap();
            });
        });
    });
}
