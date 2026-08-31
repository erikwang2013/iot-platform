// Copyright (c) 2026 erik <erik@erik.xyz> — https://erik.xyz
//! 审计日志层：对写方法（POST/PUT/PATCH/DELETE）提取 JWT 上下文记录审计事件。
//! 存储经 `AuditSink` 抽象，middleware 不持有任何 DB 依赖；
//! 网关侧用 MySQL sink 落库，测试用内存 sink 断言。

use async_trait::async_trait;
use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use ecat_auth::AuthClaims;
use std::sync::Arc;

/// 审计事件：租户 + 角色 + 请求方法/路径 + 响应状态码。
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub tenant_id: String,
    pub role: String,
    pub method: String,
    pub path: String,
    pub status: u16,
}

/// 审计事件落库抽象。
#[async_trait]
pub trait AuditSink: Send + Sync {
    async fn record(&self, event: AuditEvent);
}

/// 无操作 sink：审计未配置时静默跳过。
pub struct NullSink;
#[async_trait]
impl AuditSink for NullSink {
    async fn record(&self, _event: AuditEvent) {}
}

/// 内存 sink：测试用，收集事件以便断言。
#[derive(Default)]
pub struct MemorySink {
    pub events: tokio::sync::Mutex<Vec<AuditEvent>>,
}
#[async_trait]
impl AuditSink for MemorySink {
    async fn record(&self, event: AuditEvent) {
        self.events.lock().await.push(event);
    }
}

const WRITE_METHODS: [Method; 4] = [
    Method::POST,
    Method::PUT,
    Method::PATCH,
    Method::DELETE,
];

/// 审计中间件状态：注入 AuditSink。
#[derive(Clone)]
pub struct AuditState(pub Arc<dyn AuditSink>);

/// axum 中间件：写操作（POST/PUT/PATCH/DELETE）在响应完成后异步记录审计事件。
/// 挂载在 JWT 层内层（extensions 已注入 AuthClaims），缺 claims 记 anonymous
/// （如登录端点防爆破审计）。
pub async fn audit_middleware(
    State(state): State<AuditState>,
    req: Request,
    next: Next,
) -> Response {
    let is_write = WRITE_METHODS.contains(req.method());
    let claims = req.extensions().get::<AuthClaims>().cloned();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let resp = next.run(req).await;
    if is_write {
        let status = resp.status().as_u16();
        let (tenant_id, role) = match &claims {
            Some(c) => (c.sub.clone(), c.role.clone().unwrap_or_default()),
            None => ("anonymous".into(), String::new()),
        };
        let sink = state.0.clone();
        let event = AuditEvent {
            tenant_id,
            role,
            method,
            path,
            status,
        };
        // 非阻塞落库：失败仅 sink 内 warn，不阻塞业务响应。
        tokio::spawn(async move { sink.record(event).await });
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::middleware::from_fn_with_state;
    use axum::routing::post;
    use axum::Router;
    use tower::ServiceExt;

    fn claims_ext(tenant: &str, role: &str) -> AuthClaims {
        AuthClaims {
            sub: tenant.into(),
            exp: None,
            iat: None,
            role: Some(role.into()),
            extra: Default::default(),
        }
    }

    fn app_with(sink: Arc<MemorySink>) -> Router {
        Router::new()
            .route("/devices", post(|| async { "ok" }).get(|| async { "ok" }))
            .route("/auth/login", post(|| async { "ok" }))
            .layer(from_fn_with_state(AuditState(sink), audit_middleware))
    }

    #[tokio::test]
    async fn write_requests_are_audited_with_claims() {
        let sink = Arc::new(MemorySink::default());
        let app = app_with(sink.clone());

        let mut req = Request::builder()
            .method(Method::POST)
            .uri("/devices")
            .body(Body::empty())
            .unwrap();
        req.extensions_mut().insert(claims_ext("tenant-1", "admin"));

        let _ = app.oneshot(req).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let events = sink.events.lock().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tenant_id, "tenant-1");
        assert_eq!(events[0].role, "admin");
        assert_eq!(events[0].path, "/devices");
        assert_eq!(events[0].status, 200);
    }

    #[tokio::test]
    async fn read_requests_are_not_audited() {
        let sink = Arc::new(MemorySink::default());
        let app = app_with(sink.clone());

        let req = Request::builder()
            .uri("/devices")
            .body(Body::empty())
            .unwrap();
        let _ = app.oneshot(req).await.unwrap();
        assert!(sink.events.lock().await.is_empty());
    }

    #[tokio::test]
    async fn anonymous_write_is_audited() {
        let sink = Arc::new(MemorySink::default());
        let app = app_with(sink.clone());

        let req = Request::builder()
            .method(Method::POST)
            .uri("/auth/login")
            .body(Body::empty())
            .unwrap();
        let _ = app.oneshot(req).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let events = sink.events.lock().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tenant_id, "anonymous");
    }
}
