//! 镜像 main.rs 全链中间件栈（版本层 + 严格安全扫描 + JWT）的集成测试：
//! ApiVersionLayer → SecurityBodyCompatLayer(strict, 1MB) → JwtAuthCompat。
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
};
use ecat_auth::JwtAuthCompat;
use ecat_gateway::api_version::ApiVersionLayer;
use ecat_security::SecurityBodyCompatLayer;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;
use tower::ServiceExt;

const SECRET: &str = "p0-full-chain-secret-key-0123456789abcdefghijklmn";

#[derive(Serialize)]
struct TokenClaims<'a> {
    sub: &'a str,
    role: &'a str,
    exp: i64,
}

fn make_token(sub: &str, role: &str) -> String {
    let claims = TokenClaims {
        sub,
        role,
        exp: 4_000_000_000, // 固定未来时间，保证测试确定性
    };
    jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .unwrap()
}

async fn submit() -> &'static str {
    "ok"
}

async fn devices() -> &'static str {
    "admin-devices"
}

fn router() -> Router {
    let admin = Router::new()
        .route("/devices", get(devices))
        .layer(JwtAuthCompat::new(SECRET, &["sub", "role"]).unwrap());
    Router::new()
        .route("/api/ping", get(|| async { "pong" }))
        .route("/api/submit", post(submit))
        .nest("/api", admin)
        .layer(ApiVersionLayer)
        .layer(SecurityBodyCompatLayer::new().strict().body_limit(1024 * 1024))
}

#[tokio::test]
async fn valid_jwt_passes_full_stack() {
    let resp = router()
        .oneshot(
            Request::get("/api/devices")
                .header("x-api-version", "v1")
                .header("authorization", format!("Bearer {}", make_token("u1", "admin")))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn no_token_rejected_by_jwt_layer() {
    let resp = router()
        .oneshot(
            Request::get("/api/devices")
                .header("x-api-version", "v1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn attack_body_blocked_by_strict_scan() {
    // 与 smoke.sh step 6 同款 SQL 注入样本：strict 模式命中任意 severity 即 403
    let resp = router()
        .oneshot(
            Request::post("/api/submit")
                .header("x-api-version", "v1")
                .body(Body::from(r#"{"q":"'; DROP TABLE users; --"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
