//! 涂鸦 OpenAPI mock：绑定 127.0.0.1:18084，校验 HMAC 签名后返回固定数据。
use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};
use serde_json::{Value, json};

pub const BASE: &str = "http://127.0.0.1:18084";
pub const CLIENT_ID: &str = "mock-client-id";
pub const CLIENT_SECRET: &str = "mock-client-secret";

#[derive(Clone)]
struct MockState;

pub fn sign(secret: &str, client_id: &str, t: &str, token: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(client_id.as_bytes());
    mac.update(t.as_bytes());
    mac.update(token.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn check_sign(headers: &HeaderMap, token: &str) -> bool {
    let (Some(cid), Some(t), Some(sig)) = (
        headers.get("client_id").and_then(|v| v.to_str().ok()),
        headers.get("t").and_then(|v| v.to_str().ok()),
        headers.get("sign").and_then(|v| v.to_str().ok()),
    ) else {
        return false;
    };
    cid == CLIENT_ID && sign(CLIENT_SECRET, CLIENT_ID, t, token) == sig
}

async fn token(State(_): State<MockState>, headers: HeaderMap, query: axum::extract::Query<Value>) -> Json<Value> {
    // 签名校验（token 交换阶段 access_token 为空串）
    if !check_sign(&headers, "") {
        return Json(json!({"success": false, "code": "BAD_SIGN"}));
    }
    let grant = query.0.get("grant_type").and_then(Value::as_str).unwrap_or("");
    match grant {
        "authorization_code" => {
            let code = query.0.get("code").and_then(Value::as_str).unwrap_or("");
            let at = format!("mock-at-{code}");
            Json(json!({
                "success": true,
                "result": {
                    "access_token": at,
                    "expire_time": 2592000,
                    "refresh_token": format!("mock-rt-{code}"),
                    "uid": "mock-uid-1"
                }
            }))
        }
        "refresh_token" => {
            let rt = query.0.get("refresh_token").and_then(Value::as_str).unwrap_or("");
            Json(json!({
                "success": true,
                "result": {
                    "access_token": format!("mock-at-refreshed-{rt}"),
                    "expire_time": 2592000,
                    "refresh_token": format!("mock-rt-new-{rt}"),
                    "uid": "mock-uid-1"
                }
            }))
        }
        _ => Json(json!({"success": false, "code": "BAD_GRANT"})),
    }
}

async fn devices(State(_): State<MockState>, headers: HeaderMap) -> Json<Value> {
    let at = headers.get("access_token").and_then(|v| v.to_str().ok()).unwrap_or("");
    if at.is_empty() || !check_sign(&headers, at) {
        return Json(json!({"success": false, "code": "BAD_SIGN"}));
    }
    Json(json!({
        "success": true,
        "result": [
            {
                "id": "tuya-dev-1",
                "name": "mock-temp-sensor",
                "category": "temp_sensor",
                "online": true,
                "status": [{"code": "temp", "value": 23.5}]
            },
            {
                "id": "tuya-dev-2",
                "name": "mock-switch",
                "category": "switch",
                "online": false,
                "status": [{"code": "switch_1", "value": false}]
            }
        ]
    }))
}

async fn status(State(_): State<MockState>, headers: HeaderMap) -> Json<Value> {
    let at = headers.get("access_token").and_then(|v| v.to_str().ok()).unwrap_or("");
    if at.is_empty() || !check_sign(&headers, at) {
        return Json(json!({"success": false, "code": "BAD_SIGN"}));
    }
    Json(json!({"success": true, "result": [{"code": "temp", "value": 25.0}]}))
}

async fn commands(State(_): State<MockState>, headers: HeaderMap, body: axum::body::Bytes) -> Json<Value> {
    let at = headers.get("access_token").and_then(|v| v.to_str().ok()).unwrap_or("");
    if at.is_empty() || !check_sign(&headers, at) {
        return Json(json!({"success": false, "code": "BAD_SIGN"}));
    }
    let _ = body; // 记录即可，测试断言响应 success
    Json(json!({"success": true, "result": true}))
}

/// 并行测试复用同一进程内的 mock（进程内只绑一次，服务器跑在独立线程+自有
/// runtime 上，不随某个测试的 runtime 销毁）；端口被其他进程占用时立刻
/// panic 并带地址与 os error，避免测试打到错误服务器。
static MOCK: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub fn spawn() {
    MOCK.get_or_init(|| {
        // bind 在调用线程同步完成：失败立刻 panic（含地址与 os error），且无竞态
        let listener = std::net::TcpListener::bind("127.0.0.1:18084")
            .unwrap_or_else(|e| panic!("mock tuya: bind 127.0.0.1:18084 failed (端口被其他进程占用？): {e}"));
        listener.set_nonblocking(true).unwrap();
        let state = MockState;
        let router = Router::new()
            .route("/v1.0/token", get(token))
            .route("/v1.0/users/{uid}/devices", get(devices))
            .route("/v1.0/devices/{device_id}/status", get(status))
            .route("/v1.0/devices/{device_id}/commands", axum::routing::post(commands))
            .with_state(state);
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                // from_std 必须在驱动它的 runtime 内调用，否则 fd 注册到别的
                // runtime 的 reactor，那个 runtime 一销毁，连接就再也无人唤醒
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                axum::serve(listener, router).await.unwrap();
            });
        });
    });
}
