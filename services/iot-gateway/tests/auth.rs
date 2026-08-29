use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::get,
};
use iot_gateway::auth_compat::JwtAuthCompat;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;
use tower::ServiceExt;

const SECRET: &str = "p0-test-secret-key-0123456789abcdefghijklmn";

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
        exp: 4_000_000_000, // 未来时间，固定值保证测试确定性
    };
    jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .unwrap()
}

fn router() -> Router {
    let admin = Router::new()
        .route("/devices", get(|| async { "admin-devices" }))
        .layer(JwtAuthCompat::new(SECRET, &["sub", "role"]).unwrap());
    let client = Router::new()
        .route("/me", get(|| async { "client-me" }))
        .layer(JwtAuthCompat::new(SECRET, &["sub"]).unwrap());
    Router::new()
        .route("/health", get(|| async { "OK" }))
        .nest("/api", admin)
        .nest("/admin", client)
}

fn req(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().uri(uri);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn admin_api_with_valid_token_passes() {
    let resp = router()
        .oneshot(req("/api/devices", Some(&make_token("u1", "admin"))))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_api_without_token_returns_401() {
    let resp = router().oneshot(req("/api/devices", None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn client_api_with_valid_token_passes() {
    let resp = router()
        .oneshot(req("/admin/me", Some(&make_token("u1", "user"))))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn client_api_without_token_returns_401() {
    let resp = router().oneshot(req("/admin/me", None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_needs_no_token() {
    let resp = router().oneshot(req("/health", None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
