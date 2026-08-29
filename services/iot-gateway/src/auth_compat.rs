// JwtAuthLayer 适配层：JwtAuthService 的错误类型是 Box<dyn Error>，
// axum Router::layer 要求 Error: Into<Infallible>，无法直接挂载。
// 此层组合 JwtAuthLayer 并把错误擦除为 Infallible（错误路径实际不可达：
// JwtAuthService 的 401/403 已在内部转为响应，错误仅来自 inner，
// 而 axum Route 是 Infallible）。
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use ecat_auth::{JwtAuthError, JwtAuthLayer};
use tower::{Layer, Service};

/// 一次挂载 JwtAuthLayer 并擦除其错误，可直接用于 Router::layer。
#[derive(Clone)]
pub struct JwtAuthCompat {
    auth: JwtAuthLayer,
}

impl JwtAuthCompat {
    pub fn new(secret: &str, claims: &[&str]) -> Result<Self, JwtAuthError> {
        Ok(Self {
            auth: JwtAuthLayer::new(secret)?.require_claims(claims),
        })
    }
}

impl<S> Layer<S> for JwtAuthCompat {
    type Service = EraseError<<JwtAuthLayer as Layer<S>>::Service>;

    fn layer(&self, inner: S) -> Self::Service {
        EraseError {
            inner: self.auth.layer(inner),
        }
    }
}

/// 把 inner 的 Box<dyn Error> 错误映射为 500 响应，对外呈现 Infallible。
#[derive(Clone)]
pub struct EraseError<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for EraseError<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Error: std::fmt::Display + Send + Sync + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|_| unreachable!())
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        Box::pin(async move {
            match inner.call(req).await {
                Ok(resp) => Ok(resp),
                Err(e) => {
                    tracing::error!(error = %e, "auth middleware error");
                    Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap())
                }
            }
        })
    }
}
