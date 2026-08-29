use axum::body::Body;
use http::{Request, Response, StatusCode};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

pub const API_VERSION_HEADER: &str = "x-api-version";
pub const SUPPORTED_VERSIONS: &[&str] = &["v1"];

#[derive(Clone, Copy)]
pub struct ApiVersionLayer;

impl<S> Layer<S> for ApiVersionLayer {
    type Service = ApiVersionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ApiVersionService { inner }
    }
}

#[derive(Clone)]
pub struct ApiVersionService<S> {
    inner: S,
}

impl<S, B> Service<Request<B>> for ApiVersionService<S>
where
    S: Service<Request<B>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Infallible>,
    B: Send + 'static,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let path = req.uri().path().to_string();
        let version = req
            .headers()
            .get(API_VERSION_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let mut inner = self.inner.clone();

        Box::pin(async move {
            if path == "/health" || path == "/metrics" {
                return inner.call(req).await.map_err(Into::into);
            }
            match version {
                None => Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(r#"{"error":"missing x-api-version header"}"#))
                    .unwrap()),
                Some(v) if !SUPPORTED_VERSIONS.contains(&v.as_str()) => Ok(Response::builder()
                    .status(StatusCode::NOT_ACCEPTABLE)
                    .body(Body::from(format!(
                        r#"{{"error":"unsupported api version: {v}"}}"#
                    )))
                    .unwrap()),
                Some(_) => inner.call(req).await.map_err(Into::into),
            }
        })
    }
}
