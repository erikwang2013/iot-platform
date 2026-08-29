use iot_access::crypto::{decrypt, encrypt, derive_key};

#[test]
fn roundtrip() {
    let key = derive_key("dev-key-0123456789abcdef");
    let enc = encrypt(&key, br#"{"client_id":"c","secret":"s"}"#).unwrap();
    assert_ne!(enc.as_bytes(), br#"{"client_id":"c","secret":"s"}"#);
    let dec = decrypt(&key, &enc).unwrap();
    assert_eq!(dec, br#"{"client_id":"c","secret":"s"}"#);
}

#[test]
fn wrong_key_fails() {
    let k1 = derive_key("key-one-key-one-key-one");
    let k2 = derive_key("key-two-key-two-key-two");
    let enc = encrypt(&k1, b"data").unwrap();
    assert!(decrypt(&k2, &enc).is_err());
}

#[test]
fn ciphertext_has_nonce_and_tag() {
    let key = derive_key("dev-key-0123456789abcdef");
    let enc = encrypt(&key, b"data").unwrap();
    // base64(12 字节 nonce + 16 字节 tag + 密文) 解码后长度 ≥ 28
    let raw = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &enc).unwrap();
    assert!(raw.len() >= 28);
}
