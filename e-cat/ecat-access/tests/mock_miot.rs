//! 小米 MIoT OpenAPI mock：绑定 127.0.0.1:18085，校验 HMAC 签名（大写 hex）后返回固定数据。
//! 签名规范：sign = HMAC-SHA256(app_secret, client_id + t + access_token)，hex 大写。
//! access_token 不随请求头发送（嵌在 sign 里），mock 跟踪已签发的 token 逐一校验。
use axum::{Json, Router, extract::{Query, State}, http::HeaderMap, routing::{get, post}};
use serde_json::{Value, json};
use std::sync::{Mutex, OnceLock};

pub const BASE: &str = "http://127.0.0.1:18085";
pub const CLIENT_ID: &str = "mock-miot-client";
pub const CLIENT_SECRET: &str = "mock-miot-secret";

#[derive(Clone)]
struct MockState;

/// 已签发的 access_token（token 端点写入，设备端点校验）。
fn issued_tokens() -> &'static Mutex<Vec<String>> {
    static TOKENS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    TOKENS.get_or_init(|| Mutex::new(vec![]))
}

pub fn sign(secret: &str, client_id: &str, t: &str, token: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(client_id.as_bytes());
    mac.update(t.as_bytes());
    mac.update(token.as_bytes());
    hex::encode(mac.finalize().into_bytes()).to_uppercase()
}

/// 请求头取 client_id/t/sign；签名须与任一已签发 token 匹配（token 交换阶段用空 token）。
fn check_sign(headers: &HeaderMap) -> bool {
    let (Some(cid), Some(t), Some(sig)) = (
        headers.get("client_id").and_then(|v| v.to_str().ok()),
        headers.get("t").and_then(|v| v.to_str().ok()),
        headers.get("sign").and_then(|v| v.to_str().ok()),
    ) else {
        return false;
    };
    if cid != CLIENT_ID {
        return false;
    }
    if sign(CLIENT_SECRET, CLIENT_ID, t, "") == sig {
        return true;
    }
    issued_tokens()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|tok| sign(CLIENT_SECRET, CLIENT_ID, t, tok) == sig)
}

fn tok_error() -> Value {
    json!({"code": 400006, "message": "token invalid"})
}

fn issue(tokens: &[String]) {
    issued_tokens()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(tokens.iter().cloned());
}

async fn token(State(_): State<MockState>, headers: HeaderMap, query: Query<Value>) -> Json<Value> {
    if !check_sign(&headers) {
        return Json(json!({"code": 400004, "message": "bad sign"}));
    }
    let grant = query.0.get("grant_type").and_then(Value::as_str).unwrap_or("");
    match grant {
        "authorization_code" => {
            let code = query.0.get("code").and_then(Value::as_str).unwrap_or("");
            let at = format!("miot-at-{code}");
            issue(&[at.clone(), format!("miot-rt-{code}")]);
            Json(json!({
                "code": 0,
                "message": "ok",
                "data": {
                    "access_token": at,
                    "refresh_token": format!("miot-rt-{code}"),
                    "expires_in": 2592000,
                    "scope": "open"
                }
            }))
        }
        "refresh_token" => {
            let rt = query.0.get("refresh_token").and_then(Value::as_str).unwrap_or("");
            let at = format!("miot-at-refreshed-{rt}");
            issue(&[at.clone()]);
            Json(json!({
                "code": 0,
                "message": "ok",
                "data": {
                    "access_token": at,
                    "refresh_token": format!("miot-rt-new-{rt}"),
                    "expires_in": 2592000,
                    "scope": "open"
                }
            }))
        }
        _ => Json(json!({"code": 400001, "message": "bad grant"})),
    }
}

async fn device_list(State(_): State<MockState>, headers: HeaderMap) -> Json<Value> {
    if !check_sign(&headers) {
        return Json(tok_error());
    }
    Json(json!({
        "code": 0,
        "message": "ok",
        "data": {
            "devices": [
                {"did": "miot-dev-1", "name": "mock-temp-sensor", "model": "xiaomi.temp.v1", "online": true},
                {"did": "miot-dev-2", "name": "mock-switch", "model": "xiaomi.switch.v1", "online": false}
            ]
        }
    }))
}

async fn device_status(State(_): State<MockState>, headers: HeaderMap) -> Json<Value> {
    if !check_sign(&headers) {
        return Json(tok_error());
    }
    Json(json!({
        "code": 0,
        "message": "ok",
        "data": {
            "did": "miot-dev-1",
            "status": [{"siid": 2, "piid": 1, "code": "temp", "value": 25.0}]
        }
    }))
}

async fn command(State(_): State<MockState>, headers: HeaderMap, body: axum::body::Bytes) -> Json<Value> {
    if !check_sign(&headers) {
        return Json(tok_error());
    }
    let _ = body;
    Json(json!({"code": 0, "message": "ok", "data": {}}))
}

static MOCK: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn spawn() {
    MOCK.get_or_init(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:18085")
            .unwrap_or_else(|e| panic!("mock miot: bind 127.0.0.1:18085 failed (端口被其他进程占用？): {e}"));
        listener.set_nonblocking(true).unwrap();
        let router = Router::new()
            .route("/oauth/token", get(token))
            .route("/v1/device/list", get(device_list))
            .route("/v1/device/status", get(device_status))
            .route("/v1/device/command", post(command))
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
