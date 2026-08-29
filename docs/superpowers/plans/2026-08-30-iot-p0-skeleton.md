# IoT 平台 P0 骨架实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 跑通平台骨架——根 workspace、iot-gateway（双 API 面 + X-API-Version header + security-rust 扫描 + JWT）、iot-device（PostgreSQL 连通 + 建表 + 最小查询）、Docker Compose 基础设施，冒烟脚本全绿。

**Architecture:** 根 Cargo workspace 挂 `services/` 下两个二进制 crate；iot-gateway 用 ecat::App + HttpServer（axum 0.8），按 tower 中间件栈：JWT（分面）→ security-rust 扫描 → X-API-Version 校验 → 路由；iot-device 用 ecat-data-sqlx 的 SqlxClient + RdbmsClient 查询 PG。服务间通信（gRPC/Kafka）P1 再做，P0 各自独立端口。

**Tech Stack:** Rust 2024 edition、e-cat v3.0.3（本地 `e-cat/` path 依赖）、axum 0.8、tower 0.5、security-rust（本地 `/home/wwwroot/erikwang2013/security-rust`）、sqlx Any 驱动（mysql feature 已内置）、MySQL 8、Docker Compose。

**约定:** API 版本放 header `X-API-Version: v1`，缺失 400、不支持 406（spec §8）。管理端 `/api/*`（JWT 需 sub+role）、客户端 `/admin/*`（JWT 需 sub）。`/health`、`/metrics` 豁免鉴权与版本校验。

---

### Task 1: 根 workspace 与 services 目录

**Files:**
- Create: `Cargo.toml`（根）
- Create: `services/iot-gateway/Cargo.toml`
- Create: `services/iot-gateway/src/main.rs`
- Create: `services/iot-device/Cargo.toml`
- Create: `services/iot-device/src/main.rs`

- [ ] **Step 1: 写根 Cargo.toml**

```toml
[workspace]
members = ["services/iot-gateway", "services/iot-device"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
```

- [ ] **Step 2: 写 iot-gateway Cargo.toml**

```toml
[package]
name = "iot-gateway"
version.workspace = true
edition.workspace = true

[dependencies]
ecat = { path = "../../e-cat/ecat" }
ecat-auth = { path = "../../e-cat/ecat-auth" }
ecat-transport-http = { path = "../../e-cat/ecat-transport-http" }
security-rust = { path = "/home/wwwroot/erikwang2013/security-rust" }
axum = "0.8"
tower = "0.5"
http = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"

[dev-dependencies]
jsonwebtoken = "9"
tower-util = "0.3"
```

- [ ] **Step 3: 写 iot-device Cargo.toml**

```toml
[package]
name = "iot-device"
version.workspace = true
edition.workspace = true

[dependencies]
ecat = { path = "../../e-cat/ecat" }
ecat-transport-http = { path = "../../e-cat/ecat-transport-http" }
ecat-data-sqlx = { path = "../../e-cat/ecat-data-sqlx" }
ecat-data = { path = "../../e-cat/ecat-data" }
axum = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
```

- [ ] **Step 4: 写两个最小 main.rs（仅 /health，占位）**

`services/iot-gateway/src/main.rs`:

```rust
use axum::{Router, routing::get};

async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let router = Router::new().route("/health", get(health));
    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
    let mut app = ecat::App::builder().name("iot-gateway").version("0.1.0").server(srv).build()?;
    app.run().await?;
    Ok(())
}
```

`services/iot-device/src/main.rs`: 同上，端口 `0.0.0.0:8081`，name `iot-device`。

- [ ] **Step 5: 构建验证**

Run: `cd /home/wwwroot/iot-platform && cargo check`
Expected: 两个 crate 编译通过（首次编译较慢）。

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml services/
git commit -m "chore: P0 workspace skeleton with iot-gateway and iot-device"
```

---

### Task 2: X-API-Version 中间件

**Files:**
- Create: `services/iot-gateway/src/api_version.rs`
- Modify: `services/iot-gateway/src/main.rs`
- Create: `services/iot-gateway/tests/api_version.rs`

- [ ] **Step 1: 写失败测试**

`services/iot-gateway/tests/api_version.rs`:

```rust
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
```

- [ ] **Step 2: 运行确认失败**

Run: `cd /home/wwwroot/iot-platform && cargo test -p iot-gateway --test api_version`
Expected: 编译失败（`iot_gateway` 库不存在——main.rs 需转 lib+bin，见 Step 4）。

- [ ] **Step 3: 实现中间件**

`services/iot-gateway/src/api_version.rs`:

```rust
use axum::body::Body;
use http::{Request, Response, StatusCode};
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
    S::Error: std::error::Error + Send + Sync + 'static,
    B: Send + 'static,
{
    type Response = Response<Body>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
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
                return inner.call(req).await;
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
                Some(_) => inner.call(req).await,
            }
        })
    }
}
```

- [ ] **Step 4: 拆 lib.rs 并挂载中间件**

`services/iot-gateway/src/lib.rs`:

```rust
pub mod api_version;
```

`services/iot-gateway/src/main.rs`（替换）：

```rust
use axum::{Router, routing::get};
use iot_gateway::api_version::ApiVersionLayer;

async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let router = Router::new()
        .route("/health", get(health))
        .route("/api/ping", get(|| async { "pong" }))
        .layer(ApiVersionLayer);
    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
    let mut app = ecat::App::builder().name("iot-gateway").version("0.1.0").server(srv).build()?;
    app.run().await?;
    Ok(())
}
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd /home/wwwroot/iot-platform && cargo test -p iot-gateway --test api_version`
Expected: 4 个测试全 PASS。

- [ ] **Step 6: Commit**

```bash
git add services/iot-gateway/
git commit -m "feat(gateway): X-API-Version header middleware (400 missing / 406 unsupported)"
```

---

### Task 3: security-rust 扫描中间件

**Files:**
- Create: `services/iot-gateway/src/scan.rs`
- Modify: `services/iot-gateway/src/lib.rs`
- Modify: `services/iot-gateway/src/main.rs`
- Create: `services/iot-gateway/tests/scan.rs`

- [ ] **Step 1: 写失败测试**

`services/iot-gateway/tests/scan.rs`:

```rust
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
                .uri("/submit?q=<script>alert(1)</script>")
                .body(axum::body::Body::empty()),
        )
        .await
        .unwrap()
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
                .body(axum::body::Body::from(r#"{"name":"room1","temp":23.5}"#)),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cd /home/wwwroot/iot-platform && cargo test -p iot-gateway --test scan`
Expected: 编译失败（`iot_gateway::scan` 不存在）。

- [ ] **Step 3: 实现中间件**

`services/iot-gateway/src/scan.rs`:

```rust
use axum::body::{Body, to_bytes};
use http::{Method, Request, Response, StatusCode};
use security_rust::Scanner;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Layer, Service};

// ponytail: 1MB 上限防内存 DoS；超大请求直接 413
const MAX_BODY_BYTES: usize = 1024 * 1024;

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
    S::Error: std::error::Error + Send + Sync + 'static,
{
    type Response = Response<Body>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let scanner = Arc::clone(&self.scanner);
        let scan_body = matches!(
            req.method(),
            Method::POST | Method::PUT | Method::PATCH
        );
        let query = req.uri().query().unwrap_or("").to_string();
        let (parts, body) = req.into_parts();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let body_bytes = if scan_body {
                match to_bytes(body, MAX_BODY_BYTES).await {
                    Ok(b) => Some(b),
                    Err(_) => {
                        return Ok(Response::builder()
                            .status(StatusCode::PAYLOAD_TOO_LARGE)
                            .body(Body::empty())
                            .unwrap())
                    }
                }
            } else {
                None
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

            let req = Request::from_parts(parts, Body::from(body_bytes.unwrap_or_default()));
            inner.call(req).await
        })
    }
}
```

- [ ] **Step 4: 挂载**

`services/iot-gateway/src/lib.rs`:

```rust
pub mod api_version;
pub mod scan;
```

`services/iot-gateway/src/main.rs`（替换）：

```rust
use axum::{Router, routing::{get, post}};
use iot_gateway::{api_version::ApiVersionLayer, scan::ScanLayer};

async fn health() -> &'static str {
    "OK"
}

async fn submit() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let router = Router::new()
        .route("/health", get(health))
        .route("/api/ping", get(|| async { "pong" }))
        .route("/api/submit", post(submit))
        .layer(ApiVersionLayer)
        .layer(ScanLayer::new());
    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
    let mut app = ecat::App::builder().name("iot-gateway").version("0.1.0").server(srv).build()?;
    app.run().await?;
    Ok(())
}
```

注：`axum::body::Body` 为 `http_body::Body<Data = Bytes>`，ScanService 的 `S: Service<Request<Body>>` 约束要求 layer 加在 Router 上（Router 的 InfallibleError 满足 `S::Error: Error`）。若报 `S::Error` 不满足约束，把 `.layer()` 顺序调成 `Router::new()... .layer(ScanLayer)` 前先 `.with_state(())` 或按错误信息微调。

- [ ] **Step 5: 运行测试确认通过**

Run: `cd /home/wwwroot/iot-platform && cargo test -p iot-gateway --test scan`
Expected: 3 个测试全 PASS。

- [ ] **Step 6: Commit**

```bash
git add services/iot-gateway/
git commit -m "feat(gateway): security-rust input scan middleware (403 on attack patterns)"
```

---

### Task 4: JWT 双面鉴权路由

**Files:**
- Modify: `services/iot-gateway/src/main.rs`
- Create: `services/iot-gateway/tests/auth.rs`

- [ ] **Step 1: 写失败测试**

`services/iot-gateway/tests/auth.rs`:

```rust
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::get,
};
use ecat_auth::{AuthClaims, JwtAuthLayer};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;
use std::collections::HashMap;
use tower::ServiceExt;

const SECRET: &str = "p0-test-secret-key-0123456789abcdefghijklmn";

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
    let admin = Router::new()
        .route("/devices", get(|| async { "admin-devices" }))
        .layer(JwtAuthLayer::new(SECRET).unwrap().require_claims(&["sub", "role"]));
    let client = Router::new()
        .route("/me", get(|| async { "client-me" }))
        .layer(JwtAuthLayer::new(SECRET).unwrap().require_claims(&["sub"]));
    Router::new()
        .route("/health", get(|| async { "OK" }))
        .nest("/api", admin)
        .nest("/admin", client)
}

fn req(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().uri(uri);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
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
```

- [ ] **Step 2: 运行确认失败**

Run: `cd /home/wwwroot/iot-platform && cargo test -p iot-gateway --test auth`
Expected: 编译失败或 401 失败（main.rs 未实现双面路由）。

- [ ] **Step 3: 实现双面路由 + 签名 header**

`services/iot-gateway/src/main.rs`（替换）：

```rust
use axum::{Router, routing::{get, post}};
use ecat_auth::JwtAuthLayer;
use iot_gateway::{api_version::ApiVersionLayer, scan::ScanLayer};

async fn health() -> &'static str {
    "OK"
}

async fn submit() -> &'static str {
    "ok"
}

async fn devices() -> &'static str {
    "admin-devices"
}

async fn me() -> &'static str {
    "client-me"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-key-0123456789abcdefghijklmn".into());

    let admin_api = Router::new()
        .route("/devices", get(devices))
        .layer(JwtAuthLayer::new(&secret)?.require_claims(&["sub", "role"]));
    let client_api = Router::new()
        .route("/me", get(me))
        .layer(JwtAuthLayer::new(&secret)?.require_claims(&["sub"]));

    let router = Router::new()
        .route("/health", get(health))
        .route("/api/ping", get(|| async { "pong" }))
        .route("/api/submit", post(submit))
        .nest("/api", admin_api)
        .nest("/admin", client_api)
        .layer(ApiVersionLayer)
        .layer(ScanLayer::new());

    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8080").router(router);
    let mut app = ecat::App::builder()
        .name("iot-gateway")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
```

注意：`JWT_SECRET` 环境变量缺省用 dev secret（≥32 字节）；测试用固定 `SECRET` 常量，与路由配置无关（测试构造自己的 Router）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd /home/wwwroot/iot-platform && cargo test -p iot-gateway --test auth`
Expected: 5 个测试全 PASS。

- [ ] **Step 5: Commit**

```bash
git add services/iot-gateway/
git commit -m "feat(gateway): JWT dual-face routing (/api admin, /admin client)"
```

---

### Task 5: iot-device 服务（MySQL 连通 + 建表 + 查询）

**Files:**
- Modify: `services/iot-device/src/main.rs`
- Create: `services/iot-device/migrations/0001_init.sql`

- [ ] **Step 1: 写建表 SQL（幂等）**

`services/iot-device/migrations/0001_init.sql`:

```sql
CREATE TABLE IF NOT EXISTS tenants (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS devices (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(36) NOT NULL,
    name VARCHAR(255) NOT NULL,
    vendor VARCHAR(64) NOT NULL DEFAULT '',
    status VARCHAR(32) NOT NULL DEFAULT 'offline',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_devices_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
) ENGINE = InnoDB;

CREATE INDEX idx_devices_tenant ON devices(tenant_id);
```

- [ ] **Step 2: 写 main.rs（启动时迁移 + 双端点）**

`services/iot-device/src/main.rs`（替换）：

```rust
use axum::{
    Router,
    extract::{Query, State},
    routing::get,
};
use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Clone)]
struct Db(SqlxClient);

async fn migrate(db: &SqlxClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sql = std::fs::read_to_string("migrations/0001_init.sql")?;
    db.execute(&sql).await?;
    Ok(())
}

async fn health(State(db): State<Db>) -> Result<axum::Json<Value>, (axum::http::StatusCode, String)> {
    match db.0.query("SELECT 1").await {
        Ok(_) => Ok(axum::Json(json!({"status": "ok", "db": true}))),
        Err(e) => Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            format!("db unreachable: {e}"),
        )),
    }
}

#[derive(Deserialize)]
struct TenantFilter {
    tenant_id: Option<String>,
}

#[derive(Serialize)]
struct DeviceRow {
    id: String,
    name: String,
    vendor: String,
    status: String,
}

async fn list_devices(
    State(db): State<Db>,
    Query(filter): Query<TenantFilter>,
) -> Result<axum::Json<Value>, (axum::http::StatusCode, String)> {
    // ponytail: P0 支持任意/按租户过滤；参数化查询防注入，租户强制隔离 P1 随鉴权一起做
    let sql = match &filter.tenant_id {
        Some(_) => "SELECT id, name, vendor, status FROM devices WHERE tenant_id = ?",
        None => "SELECT id, name, vendor, status FROM devices",
    };
    let rows = db.0.query_with(sql, &[]).await.map_err(db_err)?;
    let devices: Vec<DeviceRow> = rows
        .iter()
        .map(|r| DeviceRow {
            id: r.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
            name: r.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
            vendor: r.get("vendor").and_then(Value::as_str).unwrap_or("").to_string(),
            status: r.get("status").and_then(Value::as_str).unwrap_or("").to_string(),
        })
        .collect();
    Ok(axum::Json(json!({"devices": devices})))
}

fn db_err(e: ecat_data::RdbmsError) -> (axum::http::StatusCode, String) {
    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("db error: {e}"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://iot:iot@localhost:3306/iot".into());
    let db = SqlxClient::connect(&db_url).await?;
    migrate(&db).await?;

    let router = Router::new()
        .route("/health", get(health))
        .route("/api/devices", get(list_devices))
        .with_state(Db(db));

    let srv = ecat_transport_http::HttpServer::new("0.0.0.0:8081").router(router);
    let mut app = ecat::App::builder()
        .name("iot-device")
        .version("0.1.0")
        .server(srv)
        .build()?;
    app.run().await?;
    Ok(())
}
```

注意：`query_with` 的 `?` 占位符是 sqlx Any 驱动的通用写法（Any 驱动把 `?` 翻译成后端参数占位符）。`HashMap` 导入若未用可删（`TenantFilter` 用 serde 默认即可）。`ecat_data` 的 `Row::get` 返回 `Option<&Value>`。

- [ ] **Step 3: 编译验证**

Run: `cd /home/wwwroot/iot-platform && cargo check -p iot-device`
Expected: 编译通过。

- [ ] **Step 4: Commit**

```bash
git add services/iot-device/
git commit -m "feat(device): MySQL connectivity, idempotent schema, health + device list endpoints"
```

---

### Task 6: Docker Compose 基础设施

**Files:**
- Create: `docker-compose.yml`

- [ ] **Step 1: 写 docker-compose.yml**

```yaml
services:
  mysql:
    image: mysql:8
    environment:
      MYSQL_ROOT_PASSWORD: root
      MYSQL_DATABASE: iot
      MYSQL_USER: iot
      MYSQL_PASSWORD: iot
    ports:
      - "3306:3306"
    volumes:
      - mysqldata:/var/lib/mysql
    healthcheck:
      test: ["CMD", "mysqladmin", "ping", "-h", "localhost", "-uiot", "-piot"]
      interval: 5s
      timeout: 3s
      retries: 10

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

  emqx:
    image: emqx/emqx:5.8
    ports:
      - "1883:1883"
      - "18083:18083"

  kafka:
    image: bitnami/kafka:3.7
    environment:
      KAFKA_CFG_NODE_ID: "0"
      KAFKA_CFG_PROCESS_ROLES: controller,broker
      KAFKA_CFG_LISTENERS: PLAINTEXT://:9092,CONTROLLER://:9093
      KAFKA_CFG_ADVERTISED_LISTENERS: PLAINTEXT://localhost:9092
      KAFKA_CFG_CONTROLLER_LISTENER_NAMES: CONTROLLER
      KAFKA_CFG_CONTROLLER_QUORUM_VOTERS: 0@kafka:9093
      KAFKA_CFG_LISTENER_SECURITY_PROTOCOL_MAP: CONTROLLER:PLAINTEXT,PLAINTEXT:PLAINTEXT
    ports:
      - "9092:9092"
    volumes:
      - kafkadata:/bitnami/kafka

  minio:
    image: minio/minio:latest
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: iot
      MINIO_ROOT_PASSWORD: iot-password
    ports:
      - "9000:9000"
      - "9001:9001"
    volumes:
      - miniodata:/data

volumes:
  mysqldata:
  kafkadata:
  miniodata:
```

- [ ] **Step 2: 启动基础设施并等待健康**

Run:
```bash
cd /home/wwwroot/iot-platform
docker compose up -d mysql redis emqx kafka minio
docker compose ps
```
Expected: 5 个容器 Running（mysql healthcheck 最终 healthy）。

- [ ] **Step 3: 运行 iot-device 并验证 MySQL 连通**

Run:
```bash
cd /home/wwwroot/iot-platform/services/iot-device
cargo run &
curl -s http://localhost:8081/health
```
Expected: `{"status":"ok","db":true}`；再 `curl -s http://localhost:8081/api/devices` 返回 `{"devices":[]}`。

- [ ] **Step 4: 插入种子数据验证查询**

Run:
```bash
docker exec -i iot-platform-mysql-1 mysql -uiot -piot iot <<'SQL'
INSERT IGNORE INTO tenants (id, name) VALUES ('11111111-1111-1111-1111-111111111111', 'demo-tenant');
INSERT IGNORE INTO devices (id, tenant_id, name, vendor, status)
VALUES ('22222222-2222-2222-2222-222222222222', '11111111-1111-1111-1111-111111111111', 'temp-sensor-1', 'tuya', 'online');
SQL
curl -s http://localhost:8081/api/devices
```
Expected: `{"devices":[{"id":"22222222-...","name":"temp-sensor-1","vendor":"tuya","status":"online"}]}`

- [ ] **Step 5: Commit**

```bash
git add docker-compose.yml
git commit -m "chore: docker compose infrastructure (mysql/redis/emqx/kafka/minio)"
```

---

### Task 7: 冒烟脚本（全链路验证）

**Files:**
- Create: `scripts/smoke.sh`

- [ ] **Step 1: 写冒烟脚本**

`scripts/smoke.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
# P0 冒烟：gateway 全链路（版本 header / 安全扫描 / JWT）+ device 健康
GATEWAY=${GATEWAY:-http://localhost:8080}
DEVICE=${DEVICE:-http://localhost:8081}
JWT_SECRET=${JWT_SECRET:-dev-secret-key-0123456789abcdefghijklmn}

pass=0; fail=0
check() { # check <desc> <expected> <actual>
  if [ "$2" = "$3" ]; then pass=$((pass+1)); echo "PASS: $1";
  else fail=$((fail+1)); echo "FAIL: $1 (expected $2, got $3)"; fi
}

# 1. gateway 健康（豁免版本校验）
code=$(curl -s -o /dev/null -w "%{http_code}" "$GATEWAY/health")
check "gateway /health 200" 200 "$code"

# 2. 缺版本 header → 400
code=$(curl -s -o /dev/null -w "%{http_code}" "$GATEWAY/api/ping")
check "missing x-api-version -> 400" 400 "$code"

# 3. 不支持的版本 → 406
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v2" "$GATEWAY/api/ping")
check "unsupported version -> 406" 406 "$code"

# 4. 版本正确但无 token → 401
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" "$GATEWAY/api/devices")
check "no token -> 401" 401 "$code"

# 5. 携带 token → 200（用 python3 快速签发 HS256 JWT）
token=$(python3 - "$JWT_SECRET" <<'PY'
import sys, base64, json, hmac, hashlib, time
secret = sys.argv[1].encode()
def b64(d): return base64.urlsafe_b64encode(d).rstrip(b"=").decode()
header = b64(json.dumps({"alg":"HS256","typ":"JWT"}).encode())
payload = b64(json.dumps({"sub":"smoke-user","role":"admin","exp":int(time.time())+3600}).encode())
sig = b64(hmac.new(secret, f"{header}.{payload}".encode(), hashlib.sha256).digest())
print(f"{header}.{payload}.{sig}")
PY
)
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -H "authorization: Bearer $token" "$GATEWAY/api/devices")
check "valid token -> 200" 200 "$code"

# 6. 攻击 payload → 403
code=$(curl -s -o /dev/null -w "%{http_code}" -H "x-api-version: v1" -X POST \
  -d '{"q":"'; DROP TABLE users; --"}' "$GATEWAY/api/submit")
check "sql injection -> 403" 403 "$code"

# 7. device 健康 + 数据
body=$(curl -s "$DEVICE/health")
check "device /health db ok" '{"status":"ok","db":true}' "$body"
code=$(curl -s -o /dev/null -w "%{http_code}" "$DEVICE/api/devices")
check "device list 200" 200 "$code"

echo "----"
echo "smoke: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
```

- [ ] **Step 2: 运行冒烟**

Run:
```bash
chmod +x /home/wwwroot/iot-platform/scripts/smoke.sh
cd /home/wwwroot/iot-platform/services/iot-gateway && cargo run &
cd /home/wwwroot/iot-platform && ./scripts/smoke.sh
```
Expected: 全部 PASS，`smoke: 7 passed, 0 failed`（gateway 与 device 均需运行；device 已在 Task 6 Step 3 启动，可用 `jobs`/`kill %1` 管理）。

- [ ] **Step 3: 收尾（停后台进程）**

Run: `kill %1 %2 2>/dev/null; true`

- [ ] **Step 4: Commit**

```bash
git add scripts/smoke.sh
git commit -m "test: P0 smoke script (version header / scan / jwt / db)"
```

---

## Self-Review 结果

- **Spec 覆盖**：P0 对应 spec §10 P0（仓库结构 + gateway + device + Docker 编排）。双 API 面与版本 header（spec §8）已落地；JWT/RBAC 基础（§6）落地（角色存在性校验，精确角色值过滤 P1 随租户隔离一起做）；security-rust（§6）落地。CDN/直连/时序/规则引擎按阶段规划，不属于 P0。
- **占位符扫描**：无 TBD/TODO；Task 3 Step 4 有一条针对 `S::Error` 约束的注记（真实构建中可能出现的类型问题与处理方式），不是占位符。
- **类型一致性**：`ScanLayer::new()`（Task 3 定义、Task 3 测试使用）、`ApiVersionLayer`（Task 2）、`JwtAuthLayer::new(SECRET).unwrap()`（Task 4）、`SqlxClient::connect` + `db.0.query_with`（Task 5）均前后一致；security-rust `Scanner::default().scan(&str) -> Vec<DetectionResult>`（`attack_type`/`severity` 字段）与源码一致；jsonwebtoken 9 与 ecat-auth 依赖版本一致（HS256）。
