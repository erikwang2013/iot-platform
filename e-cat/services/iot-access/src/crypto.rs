use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::Engine;

/// 密钥派生：SHA-256(环境变量 IOT_CRED_ENCRYPT_KEY 的值)，任意长度输入 → 32 字节。
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
