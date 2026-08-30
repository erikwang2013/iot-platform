//! AWS IoT mock：绑定 127.0.0.1:18087，独立实现 SigV4 签名校验（与适配器分开写，
//! 交叉验证），校验通过后返回固定数据。
use axum::{Json, Router, extract::{Path, State}, http::HeaderMap, routing::get};
use serde_json::{Value, json};

pub const BASE: &str = "http://127.0.0.1:18087";
pub const AK: &str = "mock-aws-ak";
pub const SK: &str = "mock-aws-secret";
pub const REGION: &str = "mock-region-1";

fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b))
}

fn hmac(key: &[u8], msg: &str) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(msg.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

fn signing_key(sk: &str, date_short: &str, region: &str) -> Vec<u8> {
    let k_date = hmac(format!("AWS4{sk}").as_bytes(), date_short);
    let k_region = hmac(&k_date, region);
    let k_service = hmac(&k_region, "iot");
    hmac(&k_service, "aws4_request")
}

/// 独立 SigV4 校验：从请求取 host/x-amz-date/body 重算签名比对。
fn verify(headers: &HeaderMap, method: &str, path: &str, body: &[u8]) -> bool {
    let auth = match headers.get("authorization").and_then(|v| v.to_str().ok()) {
        Some(a) => a,
        None => return false,
    };
    if !auth.starts_with("AWS4-HMAC-SHA256") {
        return false;
    }
    let cred = auth.split("Credential=").nth(1).and_then(|s| s.split(',').next()).unwrap_or("");
    let signed = auth.split("SignedHeaders=").nth(1).and_then(|s| s.split(',').next()).unwrap_or("");
    let sig = auth.split("Signature=").nth(1).unwrap_or("").trim();
    let Some((ak, scope)) = cred.split_once('/') else {
        return false;
    };
    let date_short = scope.split('/').next().unwrap_or("");
    if ak != AK || sig.is_empty() || signed != "host;x-amz-date" {
        return false;
    }
    let host = headers.get("host").and_then(|v| v.to_str().ok()).unwrap_or("");
    let amz_date = headers.get("x-amz-date").and_then(|v| v.to_str().ok()).unwrap_or("");
    if host.is_empty() || amz_date.is_empty() {
        return false;
    }
    let canonical = format!(
        "{method}\n{path}\n\nhost:{host}\nx-amz-date:{amz_date}\n\nhost;x-amz-date\n{}",
        if method == "GET" {
            "UNSIGNED-PAYLOAD".to_string()
        } else {
            sha256_hex(body)
        }
    );
    let sts = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical.as_bytes())
    );
    hex::encode(hmac(&signing_key(SK, date_short, REGION), &sts)) == sig
}

async fn things(State(_): State<MockState>, headers: HeaderMap) -> Json<Value> {
    if !verify(&headers, "GET", "/things", b"") {
        return Json(json!({"message": "SignatureDoesNotMatch"}));
    }
    Json(json!({
        "things": [
            {"thingName": "aws-dev-1", "attributes": {"name": "mock-temp-sensor"}},
            {"thingName": "aws-dev-2", "attributes": {"name": "mock-switch"}}
        ]
    }))
}

async fn shadow(State(_): State<MockState>, Path(name): Path<String>, headers: HeaderMap) -> Json<Value> {
    let path = format!("/things/{name}/shadow");
    if !verify(&headers, "GET", &path, b"") {
        return Json(json!({"message": "SignatureDoesNotMatch"}));
    }
    Json(json!({
        "state": {
            "desired": {"temp": 25.0},
            "reported": {"temp": 24.5, "humidity": 55}
        }
    }))
}

async fn shadow_update(State(_): State<MockState>, Path(name): Path<String>, headers: HeaderMap, body: axum::body::Bytes) -> Json<Value> {
    let path = format!("/things/{name}/shadow");
    if !verify(&headers, "POST", &path, &body) {
        return Json(json!({"message": "SignatureDoesNotMatch"}));
    }
    Json(json!({"state": {"desired": {"ok": true}}}))
}

#[derive(Clone)]
struct MockState;

static MOCK: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn spawn() {
    MOCK.get_or_init(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:18087")
            .unwrap_or_else(|e| panic!("mock aws: bind 127.0.0.1:18087 failed (端口被其他进程占用？): {e}"));
        listener.set_nonblocking(true).unwrap();
        let router = Router::new()
            .route("/things", get(things))
            .route("/things/{name}/shadow", get(shadow).post(shadow_update))
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
