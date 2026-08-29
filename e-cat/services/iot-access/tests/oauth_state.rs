use iot_access::crypto::{decrypt, derive_key, encrypt};
use iot_access::oauth::{decode_state, encode_state};
use iot_access::store::creds_json;

#[test]
fn state_roundtrip() {
    let s = encode_state("t1", "tuya");
    assert_eq!(decode_state(&s).unwrap(), ("t1".to_string(), "tuya".to_string()));
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
