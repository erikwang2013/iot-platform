use ecat_security::crypto::{decrypt, derive_key, encrypt};
use ecat_access::oauth::{decode_state, encode_state, encode_state_at};
use ecat_access::store::creds_json;

fn set_key() {
    // edition 2024 中 set_var 为 unsafe；state 签名密钥取自 IOT_CRED_ENCRYPT_KEY
    unsafe { std::env::set_var("IOT_CRED_ENCRYPT_KEY", "test-key-0123456789abcdef") };
}

#[test]
fn state_roundtrip() {
    set_key();
    let s = encode_state("t1", "tuya");
    assert_eq!(decode_state(&s).unwrap(), ("t1".to_string(), "tuya".to_string()));
}

#[test]
fn state_tamper_rejected() {
    use base64::Engine;
    set_key();
    let s = encode_state("t1", "tuya");
    let mut raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&s).unwrap();
    // 翻转 HMAC 最后一字节
    let last = raw.len() - 1;
    raw[last] ^= 1;
    let forged = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&raw);
    assert!(decode_state(&forged).is_err());
}

#[test]
fn state_expired_rejected() {
    set_key();
    let s = encode_state_at("t1", "tuya", 1000); // 1970 年的过期时间
    let err = decode_state(&s).unwrap_err();
    assert!(err.contains("expired"), "got: {err}");
}

#[test]
fn creds_json_roundtrip() {
    let key = derive_key("dev-key-0123456789abcdef");
    let cfg = serde_json::json!({
        "client_id": "cid", "client_secret": "cs",
        "uid": "u1", "access_token": "at", "refresh_token": "rt", "expires_at": 1690000000
    });
    let enc = encrypt(&key, &creds_json(&cfg)).unwrap();
    let dec = decrypt(&key, &enc).unwrap();
    assert_eq!(dec, creds_json(&cfg));
}
