use axum::{Router, routing::post};
use iot_gateway::scan::ScanLayer;
use tower::ServiceExt;

async fn echo() -> &'static str {
    "ok"
}

fn router() -> Router {
    Router::new().route("/submit", post(echo)).layer(ScanLayer::new())
}

#[tokio::test]
async fn sql_injection_body_blocked_with_403() {
    let resp = router()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/submit")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(r#"{"q":"'; DROP TABLE users; --"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn xss_query_blocked_with_403() {
    let resp = router()
        .oneshot(
            axum::http::Request::builder()
                .uri("/submit?q=%3Cscript%3Ealert(1)%3C%2Fscript%3E")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn benign_body_passes() {
    let resp = router()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/submit")
                .body(axum::body::Body::from(r#"{"name":"room1","temp":23.5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}
