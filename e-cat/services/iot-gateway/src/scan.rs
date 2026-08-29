use axum::body::{Body, to_bytes};
use http::{Method, Request, Response, StatusCode};
use security_rust::Scanner;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Layer, Service};

// ponytail: 1MB 上限防内存 DoS；超大请求直接 413
const MAX_BODY_BYTES: usize = 1024 * 1024;

// 解码 %XX 查询串后再扫描，否则编码过的 payload 会绕过规则
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[derive(Clone)]
pub struct ScanLayer {
    scanner: Arc<Scanner>,
}

impl ScanLayer {
    pub fn new() -> Self {
        Self {
            scanner: Arc::new(Scanner::default()),
        }
    }
}

impl Default for ScanLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for ScanLayer {
    type Service = ScanService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ScanService {
            inner,
            scanner: Arc::clone(&self.scanner),
        }
    }
}

#[derive(Clone)]
pub struct ScanService<S> {
    inner: S,
    scanner: Arc<Scanner>,
}

impl<S> Service<Request<Body>> for ScanService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Infallible>,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let scanner = Arc::clone(&self.scanner);
        let scan_body = matches!(req.method(), &Method::POST | &Method::PUT | &Method::PATCH);
        let query = percent_decode(req.uri().query().unwrap_or(""));
        let (parts, body) = req.into_parts();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let (body_bytes, pass_body) = if scan_body {
                match to_bytes(body, MAX_BODY_BYTES).await {
                    Ok(b) => (Some(b), None),
                    Err(_) => {
                        return Ok(Response::builder()
                            .status(StatusCode::PAYLOAD_TOO_LARGE)
                            .body(Body::empty())
                            .unwrap())
                    }
                }
            } else {
                (None, Some(body))
            };

            let mut input = query;
            if let Some(b) = &body_bytes {
                input.push('\n');
                input.push_str(&String::from_utf8_lossy(b));
            }

            let hits = scanner.scan(&input);
            if let Some(first) = hits.first() {
                tracing::warn!(
                    attack_type = %first.attack_type,
                    severity = ?first.severity,
                    "request blocked by security scan"
                );
                return Ok(Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::from(format!(
                        r#"{{"error":"blocked by security scan: {}"}}"#,
                        first.attack_type
                    )))
                    .unwrap());
            }

            let body = pass_body.unwrap_or_else(|| Body::from(body_bytes.unwrap_or_default()));
            let req = Request::from_parts(parts, body);
            inner.call(req).await.map_err(Into::into)
        })
    }
}
