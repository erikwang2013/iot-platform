// Copyright (c) 2026 erik <erik@erik.xyz> — https://erik.xyz
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// 恒定时间比较，避免通过响应时序探测 secret（长度提前返回只泄露长度）。
pub fn secret_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// 受保护路由前置门：请求必须携带与 IOT_GATEWAY_SECRET 一致的 x-gateway-secret
/// （该 secret 只由网关反代持有，客户端拿不到），x-tenant-id 格式合法才放行，
/// 防止客户端绕过网关直接自报任意租户。租户写入 request extensions 供 handler 用。
pub async fn tenant_from_header(mut req: Request, next: Next) -> Response {
    let expected = std::env::var("IOT_GATEWAY_SECRET").unwrap_or_default();
    let secret_ok = req
        .headers()
        .get("x-gateway-secret")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| secret_eq(v, expected.as_str()));
    if !secret_ok {
        return (StatusCode::UNAUTHORIZED, "missing or bad x-gateway-secret").into_response();
    }
    let tenant = match req
        .headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
    {
        Some(t)
            if !t.is_empty()
                && t.len() <= 64
                && t.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') =>
        {
            t
        }
        _ => return (StatusCode::UNAUTHORIZED, "missing or invalid x-tenant-id").into_response(),
    };
    req.extensions_mut().insert(tenant);
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get, Router};
    use http::Request;
    use tower::ServiceExt;

    async fn echo_tenant(axum::extract::Extension(t): axum::extract::Extension<String>) -> String {
        t
    }

    async fn app_with_secret(secret: &str) -> Router {
        // edition 2024 起 set_var 为 unsafe；测试单线程串行执行，无并发读 env 风险
        unsafe { std::env::set_var("IOT_GATEWAY_SECRET", secret) };
        Router::new()
            .route("/", get(echo_tenant))
            .layer(axum::middleware::from_fn(tenant_from_header))
    }

    #[tokio::test]
    async fn secret_eq_constant_time() {
        assert!(secret_eq("abc", "abc"));
        assert!(!secret_eq("abc", "abd"));
        assert!(!secret_eq("ab", "abc"));
        assert!(!secret_eq("", "a"));
        assert!(secret_eq("", ""));
    }

    #[tokio::test]
    async fn gate_accepts_valid_secret_and_tenant() {
        let app = app_with_secret("s3cr3t").await;
        let req = Request::builder()
            .uri("/")
            .header("x-gateway-secret", "s3cr3t")
            .header("x-tenant-id", "t-1")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64).await.unwrap();
        assert_eq!(&body[..], b"t-1");
    }

    #[tokio::test]
    async fn gate_rejects_bad_secret() {
        let app = app_with_secret("s3cr3t").await;
        let req = Request::builder()
            .uri("/")
            .header("x-gateway-secret", "wrong")
            .header("x-tenant-id", "t-1")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn gate_rejects_missing_tenant() {
        let app = app_with_secret("s3cr3t").await;
        let req = Request::builder()
            .uri("/")
            .header("x-gateway-secret", "s3cr3t")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn gate_rejects_invalid_tenant_chars() {
        let app = app_with_secret("s3cr3t").await;
        let req = Request::builder()
            .uri("/")
            .header("x-gateway-secret", "s3cr3t")
            .header("x-tenant-id", "t-1; DROP TABLE")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
