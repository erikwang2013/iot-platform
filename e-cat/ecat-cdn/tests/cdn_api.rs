use ecat_cdn::adapters::{aliyun::AliyunAdapter, cloudflare::CloudflareAdapter, tencent::TencentAdapter};
use ecat_cdn::adapter::CdnAdapter;
use ecat_cdn::models::{validate_expires, validate_url};
use serde_json::json;

#[test]
fn validate_url_accepts_http_https() {
    assert!(validate_url("https://cdn.example.com/a/b.mp4").is_ok());
    assert!(validate_url("http://cdn.example.com/a.mp4").is_ok());
    assert!(validate_url("ftp://cdn.example.com/a.mp4").is_err());
    assert!(validate_url("javascript:alert(1)").is_err());
    assert!(validate_url(&format!("https://x.com/{}", "a".repeat(2100))).is_err());
}

#[test]
fn validate_expires_bounds() {
    assert!(validate_expires(60).is_ok());
    assert!(validate_expires(86400).is_ok());
    assert!(validate_expires(59).is_err());
    assert!(validate_expires(86401).is_err());
}

#[test]
fn cloudflare_sign_url_format_and_determinism() {
    let cfg = json!({ "secret": "s3cret" });
    let a = CloudflareAdapter::new();
    let u1 = a.sign_url(&cfg, "https://cdn.example.com/v/1.mp4", 3600).unwrap();
    assert!(u1.starts_with("https://cdn.example.com/v/1.mp4?verify="));
    assert!(u1.contains("&expires="));
    // 同一输入（路径+过期窗口）→ 同一签名；时间戳推进一秒则变化
    let ts = u1.split("&expires=").nth(1).unwrap().parse::<u64>().unwrap();
    assert!(ts > 1700000000);
    let u2 = a.sign_url(&cfg, "https://cdn.example.com/v/1.mp4", 3600).unwrap();
    assert_eq!(u1, u2); // 同一秒内重复调用 → 结果确定
    // 已有 query 的 URL 用 & 连接
    let u3 = a.sign_url(&cfg, "https://cdn.example.com/v/1.mp4?x=1", 3600).unwrap();
    assert!(u3.contains("?x=1&verify="));
}

#[test]
fn aliyun_auth_key_format() {
    let cfg = json!({ "auth_key": "k" });
    let a = AliyunAdapter::new();
    let u = a.sign_url(&cfg, "https://cdn.example.com/a/b.mp4", 600).unwrap();
    let (path, q) = u.split_once("?auth_key=").unwrap();
    assert_eq!(path, "https://cdn.example.com/a/b.mp4");
    let parts: Vec<&str> = q.split('-').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[2], "0"); // uid
    assert!(parts[0].parse::<u64>().unwrap() > 1700000000);
    // md5(secret+path+ts+rand+uid) 与实现一致
    let expect = {
        use md5::{Digest, Md5};
        let mut d = Md5::new();
        d.update("k".as_bytes());
        d.update("/a/b.mp4".as_bytes());
        d.update(parts[0].as_bytes());
        d.update(parts[1].as_bytes());
        d.update(parts[2].as_bytes());
        hex::encode(d.finalize())
    };
    assert_eq!(parts[3], expect);
}

#[test]
fn tencent_auth_key_same_style_as_aliyun() {
    let cfg = json!({ "auth_key": "k" });
    let a = TencentAdapter::new();
    let u = a.sign_url(&cfg, "https://cdn.example.com/a/b.mp4", 600).unwrap();
    assert!(u.starts_with("https://cdn.example.com/a/b.mp4?auth_key="));
    let parts: Vec<&str> = u.split("?auth_key=").nth(1).unwrap().split('-').collect();
    assert_eq!(parts.len(), 4);
}

#[test]
fn sign_url_missing_secret_reports_internal() {
    let a = AliyunAdapter::new();
    let err = a.sign_url(&json!({}), "https://cdn.example.com/x.mp4", 600).unwrap_err();
    assert!(err.to_string().contains("auth_key missing"));
}
