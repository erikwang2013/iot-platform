//! 华为云 IoTDA mock：绑定 127.0.0.1:18086，独立实现 SDK-HMAC-SHA256 签名校验
//! （与适配器实现分开写，交叉验证），校验通过后返回固定数据。
use axum::{Json, Router, extract::{Path, State}, http::HeaderMap, routing::{get, post}};
use serde_json::{Value, json};

pub const BASE: &str = "http://127.0.0.1:18086";
pub const AK: &str = "mock-huawei-ak";
pub const SK: &str = "mock-huawei-secret";
pub const PROJECT_ID: &str = "mock-project-1";

fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b))
}

fn hmac_sha256_hex(key: &[u8], msg: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(msg.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn canonical_request(method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> String {
    // 签名头固定为 x-sdk-date（GET）或 content-type;x-sdk-date（POST），与适配器一致
    let mut sorted: Vec<(&str, &str)> = headers.to_vec();
    sorted.sort_by_key(|(k, _)| *k);
    let mut canonical_headers = String::new();
    let mut signed_names = Vec::new();
    for (k, v) in &sorted {
        canonical_headers.push_str(&format!("{}:{}\n", k.to_lowercase(), v.trim()));
        signed_names.push(k.to_lowercase());
    }
    format!(
        "{method}\n{path}\n\n{canonical_headers}\n{}\n{}",
        signed_names.join(";"),
        sha256_hex(body)
    )
}

/// 校验 Authorization 头：解析 Access/SignedHeaders/Signature 后独立重算。
fn verify(headers: &HeaderMap, method: &str, path: &str, body: &[u8]) -> bool {
    let auth = match headers.get("authorization").and_then(|v| v.to_str().ok()) {
        Some(a) => a,
        None => return false,
    };
    if !auth.starts_with("SDK-HMAC-SHA256") {
        return false;
    }
    let access = auth.split("Access=").nth(1).and_then(|s| s.split(',').next()).unwrap_or("");
    let signed = auth.split("SignedHeaders=").nth(1).and_then(|s| s.split(',').next()).unwrap_or("");
    let sig = auth.split("Signature=").nth(1).unwrap_or("").trim();
    if access != AK || sig.is_empty() {
        return false;
    }
    let date = headers.get("x-sdk-date").and_then(|v| v.to_str().ok()).unwrap_or("");
    if date.is_empty() {
        return false;
    }
    let mut hdrs: Vec<(&str, &str)> = vec![("x-sdk-date", date)];
    if let Some(ct) = headers.get("content-type").and_then(|v| v.to_str().ok()) {
        hdrs.push(("content-type", ct));
    }
    hdrs.sort_by_key(|(k, _)| *k); // 规范要求按名称字典序
    let expected_signed = hdrs.iter().map(|(k, _)| k.to_lowercase()).collect::<Vec<_>>().join(";");
    if expected_signed != signed {
        return false;
    }
    let canon = canonical_request(method, path, &hdrs, body);
    let sts = format!("SDK-HMAC-SHA256\n{date}\n{canon}");
    hmac_sha256_hex(SK.as_bytes(), &sts) == sig
}

async fn devices(State(_): State<MockState>, Path(project): Path<String>, headers: HeaderMap) -> Json<Value> {
    let path = format!("/v5/iot/{project}/devices");
    if !verify(&headers, "GET", &path, b"") {
        return Json(json!({"error": {"code": "IOTDA.000004", "message": "invalid auth"}}));
    }
    Json(json!({
        "devices": [
            {"device_id": "huawei-dev-1", "node_id": "mock-temp-sensor", "product_id": "temp_sensor", "status": "ONLINE"},
            {"device_id": "huawei-dev-2", "node_id": "mock-switch", "product_id": "switch", "status": "OFFLINE"}
        ],
        "page": {"count": 2}
    }))
}

async fn shadow(State(_): State<MockState>, Path((project, id)): Path<(String, String)>, headers: HeaderMap) -> Json<Value> {
    let path = format!("/v5/iot/{project}/devices/{id}/shadow");
    if !verify(&headers, "GET", &path, b"") {
        return Json(json!({"error": {"code": "IOTDA.000004", "message": "invalid auth"}}));
    }
    Json(json!({
        "shadow": [
            {"service_id": "sensor", "reported": {"temp": 25.0, "humidity": 60}}
        ]
    }))
}

async fn commands(State(_): State<MockState>, Path((project, id)): Path<(String, String)>, headers: HeaderMap, body: axum::body::Bytes) -> Json<Value> {
    let path = format!("/v5/iot/{project}/devices/{id}/commands");
    if !verify(&headers, "POST", &path, &body) {
        return Json(json!({"error": {"code": "IOTDA.000004", "message": "invalid auth"}}));
    }
    Json(json!({"command_id": "mock-cmd-1"}))
}

#[derive(Clone)]
struct MockState;

static MOCK: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn spawn() {
    MOCK.get_or_init(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:18086")
            .unwrap_or_else(|e| panic!("mock huawei: bind 127.0.0.1:18086 failed (端口被其他进程占用？): {e}"));
        listener.set_nonblocking(true).unwrap();
        let router = Router::new()
            .route("/v5/iot/{project}/devices", get(devices))
            .route("/v5/iot/{project}/devices/{id}/shadow", get(shadow))
            .route("/v5/iot/{project}/devices/{id}/commands", post(commands))
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
