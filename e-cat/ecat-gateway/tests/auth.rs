use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode},
    routing::get,
};
use ecat_auth::JwtAuthCompat;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;
use tower::ServiceExt;

const SECRET: &str = "p0-test-secret-key-0123456789abcdefghijklmn";

// 与 main.rs 一致的值级 RBAC 角色表
const ROLES_ALL: &[&str] = &["admin", "operator", "read-only"];
const ROLES_WRITE: &[&str] = &["admin", "operator"];
const ROLES_ADMIN: &[&str] = &["admin"];

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
    // 镜像 main.rs 的路由分组与角色策略
    let devices = Router::new()
        .route(
            "/devices",
            get(|| async { "admin-devices" }).post(|| async { "created" }),
        )
        .layer(
            JwtAuthCompat::new(SECRET, &["sub", "role"])
                .unwrap()
                .role_policy(ROLES_ALL, ROLES_WRITE),
        );
    let tenants = Router::new()
        .route(
            "/tenants",
            get(|| async { "tenants" }).post(|| async { "tenant-created" }),
        )
        .layer(
            JwtAuthCompat::new(SECRET, &["sub", "role"])
                .unwrap()
                .role_policy(ROLES_ALL, ROLES_ADMIN),
        );
    let ota = Router::new()
        .route(
            "/ota/firmwares",
            get(|| async { "firmwares" }).post(|| async { "firmware-created" }),
        )
        .layer(
            JwtAuthCompat::new(SECRET, &["sub", "role"])
                .unwrap()
                .role_policy(ROLES_ALL, ROLES_ADMIN),
        );
    let client = Router::new()
        .route("/me", get(|| async { "client-me" }))
        .layer(JwtAuthCompat::new(SECRET, &["sub"]).unwrap());
    Router::new()
        .route("/health", get(|| async { "OK" }))
        .nest("/api", devices)
        .nest("/api", tenants)
        .nest("/api", ota)
        .nest("/admin", client)
}

fn req(uri: &str, token: Option<&str>) -> Request<Body> {
    req_method(Method::GET, uri, token)
}

fn req_method(method: Method, uri: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

async fn status(uri: &str, method: Method, role: &str) -> StatusCode {
    router()
        .oneshot(req_method(method, uri, Some(&make_token("u1", role))))
        .await
        .unwrap()
        .status()
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

/// RBAC 矩阵-写路由（设备）：admin/operator 200，read-only 403。
#[tokio::test]
async fn write_route_allows_admin_and_operator_only() {
    assert_eq!(status("/api/devices", Method::POST, "admin").await, StatusCode::OK);
    assert_eq!(
        status("/api/devices", Method::POST, "operator").await,
        StatusCode::OK
    );
    assert_eq!(
        status("/api/devices", Method::POST, "read-only").await,
        StatusCode::FORBIDDEN
    );
}

/// RBAC 矩阵-读路由：三角色均 200。
#[tokio::test]
async fn read_route_allows_all_roles() {
    for role in ["admin", "operator", "read-only"] {
        assert_eq!(
            status("/api/devices", Method::GET, role).await,
            StatusCode::OK,
            "GET /api/devices role={role}"
        );
    }
}

/// RBAC 矩阵-租户管理（admin 专属）：admin 200，operator/read-only 403。
#[tokio::test]
async fn tenant_management_is_admin_only() {
    assert_eq!(status("/api/tenants", Method::POST, "admin").await, StatusCode::OK);
    assert_eq!(
        status("/api/tenants", Method::POST, "operator").await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        status("/api/tenants", Method::POST, "read-only").await,
        StatusCode::FORBIDDEN
    );
    // 读不受 admin-only 写策略影响
    assert_eq!(
        status("/api/tenants", Method::GET, "operator").await,
        StatusCode::OK
    );
}

/// RBAC 矩阵-OTA 固件（admin 专属）：admin 200，operator 403。
#[tokio::test]
async fn ota_firmware_management_is_admin_only() {
    assert_eq!(
        status("/api/ota/firmwares", Method::POST, "admin").await,
        StatusCode::OK
    );
    assert_eq!(
        status("/api/ota/firmwares", Method::POST, "operator").await,
        StatusCode::FORBIDDEN
    );
}

/// 值级校验：未知角色值即使带 role claim 也 403。
#[tokio::test]
async fn unknown_role_value_is_forbidden() {
    assert_eq!(
        status("/api/devices", Method::GET, "hacker").await,
        StatusCode::FORBIDDEN
    );
}
