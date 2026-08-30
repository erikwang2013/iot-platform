//! CDN 供应商适配器 × 本地 mock 的集成测试。
//! mock 独立重实现各厂商签名（交叉验证，不信任被测代码的签名逻辑）。
use ecat_cdn::adapter::CdnAdapter;
use ecat_cdn::adapters::{aliyun::AliyunAdapter, cloudflare::CloudflareAdapter, tencent::TencentAdapter};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

static ENV_LOCK: Mutex<()> = Mutex::new(());
static SPAWNED_PORTS: Mutex<Vec<u16>> = Mutex::new(Vec::new());

struct EnvGuard(Vec<(&'static str, String)>);
impl EnvGuard {
    fn set(k: &'static str, v: &str) -> Self {
        // Rust 2024：set_var 为 unsafe；单线程测试下修改是安全的
        unsafe { std::env::set_var(k, v) };
        EnvGuard(vec![(k, v.to_string())])
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, _) in &self.0 {
            unsafe { std::env::remove_var(k) };
        }
    }
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

/// 极简 HTTP/1.1 mock：一次请求 → 处理 → 响应。
async fn serve(port: u16, handler: impl Fn(Request) -> (u16, String) + Send + Sync + 'static) {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { continue };
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let mut header_end = None;
        loop {
            let n = sock.read(&mut tmp).await.unwrap();
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            header_end = buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4);
            if header_end.is_some() {
                break;
            }
        }
        let Some(he) = header_end else { break };
        let head = String::from_utf8_lossy(&buf[..he]).to_string();
        let mut lines = head.lines();
        let mut parts = lines.next().unwrap_or_default().split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let path = parts.next().unwrap_or_default().to_string();
        let headers: Vec<(String, String)> = lines
            .filter_map(|l| l.split_once(": "))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let clen = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.parse::<usize>().ok())
            .unwrap_or(0);
        while buf.len() < he + clen {
            let n = sock.read(&mut tmp).await.unwrap();
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
        }
        let body = String::from_utf8_lossy(&buf[he..he + clen]).to_string();
        let (status, resp_body) = handler(Request { method, path, headers, body });
        let resp = format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            resp_body.len(),
            resp_body
        );
        let _ = sock.write_all(resp.as_bytes()).await;
        let _ = sock.shutdown().await;
    }
}

fn spawn_once(port: u16, handler: impl Fn(Request) -> (u16, String) + Send + Sync + 'static) {
    let mut ports = SPAWNED_PORTS.lock().unwrap_or_else(|e| e.into_inner());
    if ports.contains(&port) {
        return;
    }
    ports.push(port);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(serve(port, handler));
    });
}

// ---------- Cloudflare ----------

#[test]
fn cloudflare_ping_purge_prefetch() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("CLOUDFLARE_API_BASE", "http://127.0.0.1:18805");
    let hits = Arc::new(Mutex::new(Vec::new()));
    let h = hits.clone();
    spawn_once(18805, move |req| {
        let auth = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        assert_eq!(auth, "Bearer t0k3n");
        let mut seen = h.lock().unwrap();
        seen.push((req.method.clone(), req.path.clone(), req.body.clone()));
        match seen.len() {
            1 => {
                assert_eq!(req.method, "GET");
                assert_eq!(req.path, "/zones/z1");
            }
            2 => {
                assert_eq!(req.method, "POST");
                assert_eq!(req.path, "/zones/z1/purge_cache");
                assert_eq!(req.body, r#"{"files":["https://x.com/a.mp4"]}"#);
            }
            _ => panic!("unexpected request"),
        }
        (200, json!({ "success": true, "result": [] }).to_string())
    });
    let cfg = json!({ "api_token": "t0k3n", "zone_id": "z1" });
    let a = CloudflareAdapter::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        a.ping(&cfg).await.unwrap();
        a.purge(&cfg, &["https://x.com/a.mp4".into()]).await.unwrap();
        a.prefetch(&cfg, &["https://x.com/a.mp4".into()]).await.unwrap(); // no-op
    });
    assert_eq!(hits.lock().unwrap().len(), 2);
}

// ---------- Aliyun ----------

fn aliyun_mock_handler() -> impl Fn(Request) -> (u16, String) {
    move |req: Request| {
        assert_eq!(req.method, "POST");
        let query: Vec<(String, String)> = req
            .path
            .trim_start_matches('/')
            .trim_start_matches('?')
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let ak = query.iter().find(|(k, _)| k == "AccessKeyId").unwrap().1.clone();
        assert_eq!(ak, "ak1");
        let sig = query.iter().find(|(k, _)| k == "Signature").unwrap().1.clone();
        let action = query.iter().find(|(k, _)| k == "Action").unwrap().1.clone();
        // 独立重算：排序参数（除 Signature）→ 规范化 → HMAC-SHA1(sk&, POST&%2F&enc)
        let mut rest: Vec<&(String, String)> = query.iter().filter(|(k, _)| k != "Signature").collect();
        rest.sort_by(|a, b| a.0.cmp(&b.0));
        let canonical = rest
            .iter()
            .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
            .collect::<Vec<_>>()
            .join("&");
        let sts = format!("POST&{}&{}", enc("/"), enc(&canonical));
        let expect = base64_hmac_sha1(&format!("sk1&"), &sts);
        assert_eq!(sig, expect, "aliyun signature mismatch for {action}");
        match action.as_str() {
            "DescribeCdnService" => (200, json!({ "Code": "Success" }).to_string()),
            "RefreshObjectCaches" => (200, json!({ "Code": "Success", "RefreshTaskId": "t1" }).to_string()),
            "PushObjectCache" => (200, json!({ "Code": "Success", "PushTaskId": "t2" }).to_string()),
            other => panic!("unexpected action {other}"),
        }
    }
}

fn enc(s: &str) -> String {
    const U: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~";
    let mut out = String::new();
    for b in s.bytes() {
        if U.contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn base64_hmac_sha1(key: &str, msg: &str) -> String {
    use base64::Engine;
    use hmac::{Hmac, Mac};
    use sha1::Sha1;
    type H = Hmac<Sha1>;
    let mut m = H::new_from_slice(key.as_bytes()).unwrap();
    m.update(msg.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(m.finalize().into_bytes())
}

#[test]
fn aliyun_ping_purge_prefetch_signed() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("ALIYUN_CDN_BASE", "http://127.0.0.1:18806");
    spawn_once(18806, aliyun_mock_handler());
    let cfg = json!({ "access_key_id": "ak1", "access_key_secret": "sk1" });
    let a = AliyunAdapter::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        a.ping(&cfg).await.unwrap();
        a.purge(&cfg, &["https://x.com/a.mp4".into()]).await.unwrap();
        a.prefetch(&cfg, &["https://x.com/a.mp4".into()]).await.unwrap();
    });
}

// ---------- Tencent ----------

fn sha256(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b))
}

fn hmac_sha256(key: &[u8], msg: &str) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type H = Hmac<Sha256>;
    let mut m = H::new_from_slice(key).unwrap();
    m.update(msg.as_bytes());
    m.finalize().into_bytes().to_vec()
}

fn tencent_mock_handler() -> impl Fn(Request) -> (u16, String) {
    move |req: Request| {
        assert_eq!(req.method, "POST");
        let auth = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .unwrap()
            .1
            .clone();
        let ts = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("x-tc-timestamp"))
            .unwrap()
            .1
            .clone();
        let action = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("x-tc-action"))
            .unwrap()
            .1
            .clone();
        let host = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("host"))
            .unwrap()
            .1
            .clone();
        // 独立重算 TC3 链
        let payload = sha256(req.body.as_bytes());
        let canonical = format!(
            "POST\n/\n\ncontent-type:application/json\nhost:{host}\n\ncontent-type;host\n{payload}"
        );
        let date = &auth.split("Credential=").nth(1).unwrap().split('/').nth(1).unwrap()[..10];
        let scope = format!("{date}/cdn/tc3_request");
        let sts = format!("TC3-HMAC-SHA256\n{ts}\n{scope}\n{}", sha256(canonical.as_bytes()));
        let k_date = hmac_sha256(format!("TC3sk1").as_bytes(), date);
        let k_svc = hmac_sha256(&k_date, "cdn");
        let k_sig = hmac_sha256(&k_svc, "tc3_request");
        let expect = format!(
            "TC3-HMAC-SHA256 Credential=id1/{scope}, SignedHeaders=content-type;host, Signature={}",
            hex::encode(hmac_sha256(&k_sig, &sts))
        );
        assert_eq!(auth, expect, "tencent TC3 signature mismatch for {action}");
        let body: Value = serde_json::from_str(&req.body).unwrap();
        match action.as_str() {
            "DescribeCdnDomains" => {
                assert_eq!(body["Limit"], 1);
                (200, json!({ "Response": { "TotalCount": 0 } }).to_string())
            }
            "PurgeUrlsCache" => {
                assert_eq!(body["Urls"], json!(["https://x.com/a.mp4"]));
                (200, json!({ "Response": { "TaskId": "t1" } }).to_string())
            }
            "PushUrlsCache" => {
                assert_eq!(body["Urls"], json!(["https://x.com/a.mp4"]));
                (200, json!({ "Response": { "TaskId": "t2" } }).to_string())
            }
            other => panic!("unexpected action {other}"),
        }
    }
}

#[test]
fn tencent_ping_purge_prefetch_signed() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("TENCENT_CDN_BASE", "http://127.0.0.1:18807");
    spawn_once(18807, tencent_mock_handler());
    let cfg = json!({ "secret_id": "id1", "secret_key": "sk1" });
    let a = TencentAdapter::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        a.ping(&cfg).await.unwrap();
        a.purge(&cfg, &["https://x.com/a.mp4".into()]).await.unwrap();
        a.prefetch(&cfg, &["https://x.com/a.mp4".into()]).await.unwrap();
    });
}

#[test]
fn unknown_vendor_rejected() {
    assert!(ecat_cdn::adapter::adapter_for("nonexistent").is_err());
    assert!(ecat_cdn::adapter::adapter_for("cloudflare").is_ok());
}
