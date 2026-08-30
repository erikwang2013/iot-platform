//! 限流集成测试：镜像 main.rs 的挂载方式
//! （HandleErrorLayer 包住 RateLimitLayer，超限错误映射 429）。
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::get,
};
use ecat_middleware::{RateLimitError, RateLimitLayer};
use std::time::Duration;
use tower::ServiceExt;

async fn rate_limit_error(
    e: Box<dyn std::error::Error + Send + Sync>,
) -> axum::response::Response {
    if e.is::<RateLimitError>() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "rate limit exceeded",
        )
            .into_response();
    }
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

fn app() -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(
            tower::ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(rate_limit_error))
                .layer(
                    RateLimitLayer::new(2, Duration::from_secs(60))
                        .with_key_fn(|_req: &axum::http::Request<Body>| "fixed-key".into()),
                )
                .into_inner(),
        )
}

#[tokio::test]
async fn within_limit_returns_200() {
    let app = app();
    for _ in 0..2 {
        let resp = app
            .clone()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn over_limit_returns_429() {
    let app = app(); // 单实例共享计数（store 在 Arc 内跨 clone 共享）
    for _ in 0..2 {
        app.clone()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
    }
    let resp = app
        .clone()
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}
