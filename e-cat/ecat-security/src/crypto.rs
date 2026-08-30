// Copyright (c) 2026 erik <erik@erik.xyz> — https://erik.xyz
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::Engine;

/// 密钥派生：SHA-256(环境变量 IOT_CRED_ENCRYPT_KEY 的值)，任意长度输入 → 32 字节。
/// 派生方案不可变更：数据库中的既有密文依赖此派生结果。
pub fn derive_key(env_value: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(env_value.as_bytes()).into()
}

pub fn encrypt(key: &[u8; 32], plain: &[u8]) -> Result<String, String> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce = [0u8; 12];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plain)
        .map_err(|e| format!("aes encrypt: {e}"))?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    Ok(base64::engine::general_purpose::STANDARD.encode(out))
}

pub fn decrypt(key: &[u8; 32], enc: &str) -> Result<Vec<u8>, String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(enc)
        .map_err(|e| format!("base64 decode: {e}"))?;
    if raw.len() < 28 {
        return Err("ciphertext too short".into());
    }
    let (nonce, ct) = raw.split_at(12);
    let cipher = Aes256Gcm::new(key.into());
    cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|e| format!("aes decrypt (密钥错误或密文被篡改): {e}"))
}

/// HMAC-SHA256 十六进制签名验签（如涂鸦 webhook x-tuya-signature）。
/// 十六进制解码失败即视为不匹配；比较为常数时间。
pub fn verify_hmac_sha256_hex(secret: &str, raw: &[u8], sig_hex: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    // 全限定 new_from_slice：模块级 aes_gcm 的 KeyInit 与 hmac 的 Mac 同名方法
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(raw);
    let expect = mac.finalize().into_bytes();
    let got = match hex::decode(sig_hex) {
        Ok(g) => g,
        Err(_) => return false,
    };
    ct_eq(&expect, &got)
}

/// 常数时间字节比较（等长逐字节 XOR；长度差直接失败）。
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = derive_key("test-env-value");
        let plain = b"hello credentials";
        let enc = encrypt(&key, plain).unwrap();
        assert_ne!(enc, String::from_utf8_lossy(plain));
        assert_eq!(decrypt(&key, &enc).unwrap(), plain);
    }

    #[test]
    fn decrypt_wrong_key_fails() {
        let key = derive_key("correct");
        let wrong = derive_key("wrong");
        let enc = encrypt(&key, b"secret").unwrap();
        assert!(decrypt(&wrong, &enc).is_err());
    }

    #[test]
    fn decrypt_garbage_fails() {
        let key = derive_key("test");
        assert!(decrypt(&key, "not-base64!!").is_err());
        assert!(decrypt(&key, "c2hvcnQ=").is_err(), "短密文应拒绝");
    }

    #[test]
    fn derive_key_is_deterministic_and_32_bytes() {
        let a = derive_key("env-value");
        let b = derive_key("env-value");
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
        assert_ne!(derive_key("other"), a);
    }

    #[test]
    fn verify_hmac_accepts_valid_hex() {
        let secret = "client-secret";
        let raw = b"{\"type\":\"report\"}";
        let sig = {
            use hmac::{Hmac, Mac};
            type HmacSha256 = Hmac<sha2::Sha256>;
            let mut mac = <HmacSha256 as Mac>::new_from_slice(secret.as_bytes()).unwrap();
            mac.update(raw);
            hex::encode(mac.finalize().into_bytes())
        };
        assert!(verify_hmac_sha256_hex(secret, raw, &sig));
    }

    #[test]
    fn verify_hmac_rejects_tampered() {
        let secret = "client-secret";
        let raw = b"{\"type\":\"report\"}";
        let sig = {
            use hmac::{Hmac, Mac};
            type HmacSha256 = Hmac<sha2::Sha256>;
            let mut mac = <HmacSha256 as Mac>::new_from_slice(secret.as_bytes()).unwrap();
            mac.update(raw);
            hex::encode(mac.finalize().into_bytes())
        };
        assert!(!verify_hmac_sha256_hex(secret, b"{\"type\":\"delete\"}", &sig));
        assert!(!verify_hmac_sha256_hex(secret, raw, "deadbeef"));
        assert!(!verify_hmac_sha256_hex(secret, raw, "not-hex!"));
        assert!(!verify_hmac_sha256_hex("other-secret", raw, &sig));
    }
}
