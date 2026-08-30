use axum::{Router, body::Body, http::Request, middleware, routing::get};
use tower::ServiceExt;

async fn handler() -> &'static str {
    "ok"
}

/// 镜像 ecat-device/src/main.rs 的保护挂载：/api/devices 必须过
/// tenant_from_header（x-gateway-secret + x-tenant-id）才可达。
fn app() -> Router {
    Router::new()
        .route("/api/devices", get(handler))
        .layer(middleware::from_fn(ecat_middleware::tenant_from_header))
}

#[tokio::test]
async fn rejects_without_headers() {
    // edition 2024 起 set_var 为 unsafe；测试单线程串行执行，无并发读 env 风险
    unsafe { std::env::set_var("IOT_GATEWAY_SECRET", "s3cr3t") };
    let resp = app()
        .oneshot(Request::get("/api/devices").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rejects_with_secret_but_without_tenant() {
    unsafe { std::env::set_var("IOT_GATEWAY_SECRET", "s3cr3t") };
    let resp = app()
        .oneshot(
            Request::get("/api/devices")
                .header("x-gateway-secret", "s3cr3t")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accepts_with_secret_and_tenant() {
    unsafe { std::env::set_var("IOT_GATEWAY_SECRET", "s3cr3t") };
    let resp = app()
        .oneshot(
            Request::get("/api/devices")
                .header("x-gateway-secret", "s3cr3t")
                .header("x-tenant-id", "t-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}
