use axum::{Router, routing::get};
use iot_gateway::api_version::ApiVersionLayer;
use tower::ServiceExt;

async fn root() -> &'static str {
    "ok"
}

fn router() -> Router {
    Router::new()
        .route("/health", get(root))
        .route("/api/ping", get(root))
        .layer(ApiVersionLayer)
}

#[tokio::test]
async fn missing_header_returns_400() {
    let resp = router().oneshot(
        axum::http::Request::builder()
            .uri("/api/ping")
            .body(axum::body::Body::empty())
            .unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unsupported_version_returns_406() {
    let resp = router().oneshot(
        axum::http::Request::builder()
            .uri("/api/ping")
            .header("x-api-version", "v2")
            .body(axum::body::Body::empty())
            .unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::NOT_ACCEPTABLE);
}

#[tokio::test]
async fn supported_version_passes() {
    let resp = router().oneshot(
        axum::http::Request::builder()
            .uri("/api/ping")
            .header("x-api-version", "v1")
            .body(axum::body::Body::empty())
            .unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}

#[tokio::test]
async fn health_exempt_from_version_check() {
    let resp = router().oneshot(
        axum::http::Request::builder()
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}
