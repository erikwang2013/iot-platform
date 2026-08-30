use crate::store::Store;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use ecat_auth::issue_token;
use ecat_security::crypto::verify_hmac_sha256_hex;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthState {
    pub store: Arc<Store>,
    pub jwt_secret: String,
    pub token_ttl_secs: u64,
}

#[derive(Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

/// 密码校验：pepper 为密钥的 HMAC-SHA256 摘要比对（常数时间）。
pub fn password_matches(stored_hash: &str, pepper: &str, password: &str) -> bool {
    verify_hmac_sha256_hex(pepper, password.as_bytes(), stored_hash)
}

/// POST /api/auth/login 与 /admin/auth/login（网关公开代理，无 JWT 前置）
/// 校验 users 表 → 签发 {sub: tenant_id, role, exp, iat} 的 JWT。
/// 失败统一 401 通用文案，细节只进日志（防用户枚举）。
pub async fn login(
    State(auth): State<AuthState>,
    Json(req): Json<LoginReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if req.username.trim().is_empty() || req.password.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "username and password required".into()));
    }
    let user = match auth.store.find_user(req.username.trim()).await {
        Ok(Some(u)) => u,
        Ok(None) | Err(_) => {
            tracing::warn!("login failed: user not found or lookup error");
            return Err((StatusCode::UNAUTHORIZED, "invalid username or password".into()));
        }
    };
    let pepper = std::env::var("IOT_PASSWORD_PEPPER")
        .unwrap_or_else(|_| "iot-password-pepper-v1".into());
    if !password_matches(&user.password_hash, &pepper, &req.password) {
        tracing::warn!(user = %user.username, "login failed: bad password");
        return Err((StatusCode::UNAUTHORIZED, "invalid username or password".into()));
    }
    let token = issue_token(&auth.jwt_secret, &user.tenant_id, &user.role, auth.token_ttl_secs)
        .map_err(|e| {
            tracing::error!(error = %e, "token issue failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
        })?;
    Ok(Json(json!({
        "token": token,
        "user": { "id": user.id, "username": user.username, "role": user.role, "tenant_id": user.tenant_id },
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecat_security::crypto::hmac_sha256_hex;

    #[test]
    fn password_hash_matches_and_rejects() {
        let pepper = "test-pepper";
        let hash = hmac_sha256_hex(pepper, b"admin123");
        assert!(password_matches(&hash, pepper, "admin123"));
        assert!(!password_matches(&hash, pepper, "wrong"));
        assert!(!password_matches(&hash, "other-pepper", "admin123"));
    }
}
