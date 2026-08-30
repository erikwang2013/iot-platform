//! 限流集成测试：镜像 main.rs 的挂载方式
//! （HandleErrorLayer 包住 RateLimitLayer，超限错误映射 429）。
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::get,
};
use ecat_middleware::{MemoryStore, RateLimitError, RateLimitLayer, RateLimitStore, RedisRateLimitStore};
use std::sync::Arc;
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

/// 镜像 main.rs 登录限流的挂载方式：两个"实例"（独立 Router）共享同一
/// 存储 —— 一个实例刷满，另一个实例同样被限（多实例一致生效的前提）。
fn login_app(store: Arc<dyn RateLimitStore>, max: u32) -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(
            tower::ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(rate_limit_error))
                .layer(
                    RateLimitLayer::new(max, Duration::from_secs(60))
                        .with_store(store)
                        .with_key_fn(|_req: &axum::http::Request<Body>| "shared-test-key".into()),
                )
                .into_inner(),
        )
}

#[tokio::test]
async fn shared_store_blocks_across_instances() {
    // 无 Redis 时用内存存储验证共享计数语义；Redis 版见下（有 Redis 才跑）
    let store: Arc<dyn RateLimitStore> = Arc::new(MemoryStore::new());
    let instance_a = login_app(store.clone(), 2);
    let instance_b = login_app(store, 2);

    for _ in 0..2 {
        instance_a
            .clone()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
    }
    // 实例 A 刷满后，实例 B 同样被限（共享存储，多实例计数一致）
    let resp = instance_b
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn redis_store_shared_across_instances() {
    // Redis 集成测试：REDIS_URL 不可达则跳过（本地 CI 无 Redis 也能跑）
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let Ok(store) = RedisRateLimitStore::connect(&url).await else {
        eprintln!("skip: redis unavailable at {url}");
        return;
    };
    // 独立 key 命名空间（按进程 ID），避免测试污染共享 Redis
    let ns = format!("test-login-{}", std::process::id());
    let store: Arc<dyn RateLimitStore> = Arc::new(store);
    let a = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(
            tower::ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(rate_limit_error))
                .layer(
                    RateLimitLayer::new(2, Duration::from_secs(60))
                        .with_store(store.clone())
                        .with_key_fn({
                            let ns = ns.clone();
                            move |_: &axum::http::Request<Body>| format!("{ns}:key-a")
                        }),
                )
                .into_inner(),
        );
    let b = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(
            tower::ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(rate_limit_error))
                .layer(
                    RateLimitLayer::new(2, Duration::from_secs(60))
                        .with_store(store)
                        .with_key_fn({
                            let ns = ns.clone();
                            move |_: &axum::http::Request<Body>| format!("{ns}:key-a")
                        }),
                )
                .into_inner(),
        );

    for _ in 0..2 {
        a.clone()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
    }
    // 实例 B 走同一 Redis 桶（模拟另一副本），已被实例 A 刷满 → 429
    let resp = b
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}
