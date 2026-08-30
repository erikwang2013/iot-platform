use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::adapter::CdnError;
use crate::models::{NewProvider, TaskReq, UpdateProvider, UrlReq, validate_expires, validate_url};
use crate::store::CdnStore;

#[derive(Clone)]
pub struct ApiState {
    pub store: Arc<CdnStore>,
}

/// 错误净化：细节只进日志，客户端只收到通用文案。
fn err_resp(err: String, status: StatusCode) -> (StatusCode, Json<Value>) {
    tracing::warn!("cdn api error: {err}");
    (status, Json(json!({ "error": "request failed" })))
}

fn bad_request(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": msg })))
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/providers", post(create_provider).get(list_providers))
        .route(
            "/providers/{id}",
            get(get_provider).put(update_provider).delete(delete_provider),
        )
        .route("/providers/{id}/enable", post(enable_provider))
        .route("/providers/{id}/disable", post(disable_provider))
        .route("/providers/{id}/test", post(test_provider))
        .route("/providers/{id}/signed-url", post(signed_url))
        .route("/providers/{id}/purge", post(purge_provider))
        .route("/providers/{id}/prefetch", post(prefetch_provider))
        .route("/tasks", get(list_tasks))
        .route("/stats", get(provider_stats))
        .with_state(state)
}

async fn create_provider(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
    Json(req): Json<NewProvider>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    if req.name.trim().is_empty() || req.name.len() > 255 {
        return Err(bad_request("name required, max 255 chars".into()));
    }
    if crate::adapter::adapter_for(&req.vendor).is_err() {
        return Err(bad_request(format!("unknown vendor: {}", req.vendor)));
    }
    if let Some(d) = &req.domain {
        if d.len() > 255 {
            return Err(bad_request("domain too long".into()));
        }
    }
    let id = st
        .store
        .create(&tenant, &req.name, &req.vendor, req.domain.as_deref().unwrap_or(""), &req.config)
        .await
        .map_err(|e| err_resp(e, StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

async fn list_providers(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let list = st
        .store
        .list(&tenant)
        .await
        .map_err(|e| err_resp(e, StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(json!(list)))
}

async fn get_provider(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let p = st
        .store
        .get(&tenant, &id)
        .await
        .map_err(|e| err_resp(e, StatusCode::NOT_FOUND))?;
    Ok(Json(json!(p)))
}

async fn update_provider(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProvider>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Some(name) = &req.name {
        if name.trim().is_empty() || name.len() > 255 {
            return Err(bad_request("name too long".into()));
        }
    }
    let p = st
        .store
        .update(&tenant, &id, &req)
        .await
        .map_err(|e| err_resp(e, StatusCode::NOT_FOUND))?;
    Ok(Json(json!(p)))
}

async fn delete_provider(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    st.store
        .delete(&tenant, &id)
        .await
        .map_err(|e| err_resp(e, StatusCode::NOT_FOUND))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_status_impl(
    st: &ApiState,
    tenant: &str,
    id: &str,
    status: &str,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let p = st
        .store
        .set_status(tenant, id, status)
        .await
        .map_err(|e| err_resp(e, StatusCode::NOT_FOUND))?;
    Ok(Json(json!(p)))
}

async fn enable_provider(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    set_status_impl(&st, &tenant, &id, "enabled").await
}

async fn disable_provider(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    set_status_impl(&st, &tenant, &id, "disabled").await
}

async fn test_provider(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let p = st
        .store
        .get(&tenant, &id)
        .await
        .map_err(|e| err_resp(e, StatusCode::NOT_FOUND))?;
    let adapter = crate::adapter::adapter_for(&p.vendor).map_err(|e| {
        tracing::warn!("unknown vendor: {e}");
        (StatusCode::BAD_REQUEST, Json(json!({ "error": "unknown vendor" })))
    })?;
    adapter
        .ping(&p.config)
        .await
        .map_err(|e| err_resp(format!("{e}"), StatusCode::BAD_GATEWAY))?;
    Ok(Json(json!({ "ok": true })))
}

async fn signed_url(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
    Path(id): Path<String>,
    Json(req): Json<UrlReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    validate_url(&req.url).map_err(bad_request)?;
    validate_expires(req.expires_secs).map_err(bad_request)?;
    let p = st
        .store
        .get(&tenant, &id)
        .await
        .map_err(|e| err_resp(e, StatusCode::NOT_FOUND))?;
    let adapter = crate::adapter::adapter_for(&p.vendor).map_err(|_| bad_request("unknown vendor".into()))?;
    match adapter.sign_url(&p.config, &req.url, req.expires_secs) {
        Ok(url) => Ok(Json(json!({ "url": url }))),
        Err(CdnError::Internal(e)) => Err(err_resp(e, StatusCode::BAD_REQUEST)),
        Err(e) => Err(err_resp(format!("{e}"), StatusCode::INTERNAL_SERVER_ERROR)),
    }
}

async fn run_task_impl(
    st: &ApiState,
    tenant: &str,
    id: &str,
    kind: &str,
    req: TaskReq,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if req.urls.is_empty() || req.urls.len() > 100 {
        return Err(bad_request("urls required, max 100".into()));
    }
    for u in &req.urls {
        validate_url(u).map_err(bad_request)?;
    }
    let p = st
        .store
        .get(tenant, id)
        .await
        .map_err(|e| err_resp(e, StatusCode::NOT_FOUND))?;
    if p.status != "enabled" {
        return Err(bad_request("provider not enabled".into()));
    }
    let adapter = crate::adapter::adapter_for(&p.vendor).map_err(|_| bad_request("unknown vendor".into()))?;
    let result = match kind {
        "purge" => adapter.purge(&p.config, &req.urls).await,
        "prefetch" => adapter.prefetch(&p.config, &req.urls).await,
        _ => unreachable!(),
    };
    let (status, error) = match result {
        Ok(()) => ("done", String::new()),
        Err(e) => ("failed", format!("{e}")),
    };
    st.store
        .record_task(tenant, id, kind, &req.urls, status, &error)
        .await
        .map_err(|e| err_resp(e, StatusCode::INTERNAL_SERVER_ERROR))?;
    if status == "failed" {
        Err(err_resp(error, StatusCode::BAD_GATEWAY))
    } else {
        Ok(Json(json!({ "ok": true, "status": status })))
    }
}

async fn purge_provider(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
    Path(id): Path<String>,
    Json(req): Json<TaskReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    run_task_impl(&st, &tenant, &id, "purge", req).await
}

async fn prefetch_provider(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
    Path(id): Path<String>,
    Json(req): Json<TaskReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    run_task_impl(&st, &tenant, &id, "prefetch", req).await
}

/// GET /api/cdn/stats：供应商总数 + 已启用数。
async fn provider_stats(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (total, enabled) = st
        .store
        .stats(&tenant)
        .await
        .map_err(|e| err_resp(e, StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(json!({ "total": total, "enabled": enabled })))
}

async fn list_tasks(
    State(st): State<ApiState>,
    Extension(tenant): Extension<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tasks = st
        .store
        .list_tasks(&tenant)
        .await
        .map_err(|e| err_resp(e, StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(json!(tasks)))
}
