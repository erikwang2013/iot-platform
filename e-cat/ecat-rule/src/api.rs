use crate::models::{NewNotifyChannel, NewRule};
use crate::store::RuleStore;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiState {
    pub store: Arc<RuleStore>,
}

#[derive(Deserialize)]
pub struct AlertQuery {
    pub status: Option<String>,
}

/// 受保护路由（挂载见 main.rs，路径前缀 /api/rule，租户经 tenant_from_header 写入 extensions）。
pub fn router(api: ApiState) -> axum::Router {
    axum::Router::new()
        .route("/rules", axum::routing::get(list_rules).post(create_rule))
        .route("/rules/{id}", axum::routing::put(update_rule).delete(delete_rule))
        .route("/alerts", axum::routing::get(list_alerts))
        .route("/alerts/{id}/ack", axum::routing::post(ack_alert))
        .route("/stats", axum::routing::get(alert_stats))
        .route(
            "/channels/{channel}",
            axum::routing::put(upsert_channel).delete(delete_channel),
        )
        .route("/channels", axum::routing::get(list_channels))
        .route("/reports", axum::routing::get(list_reports))
        .with_state(api)
}

#[derive(Deserialize)]
pub struct ReportQuery {
    pub date: Option<String>,
}

/// GET /api/rule/reports?date=YYYY-MM-DD：本租户每日汇总报表列表
/// （倒序最多 30 条；date 可选过滤）。报表由定时任务生成（见 report.rs）。
pub async fn list_reports(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
    Query(q): Query<ReportQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let date = q.date.as_deref().filter(|d| !d.is_empty());
    let reports = crate::report::list_reports(&api.store.db, &tenant, date)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::json!({ "reports": reports })))
}

/// GET /api/rule/rules
pub async fn list_rules(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
) -> Result<Json<Vec<crate::models::Rule>>, (StatusCode, String)> {
    let rules = api
        .store
        .list_rules(&tenant)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(rules))
}

/// POST /api/rule/rules
pub async fn create_rule(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
    Json(body): Json<NewRule>,
) -> Result<(StatusCode, Json<crate::models::Rule>), (StatusCode, String)> {
    let rule = api
        .store
        .insert_rule(&tenant, &body)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok((StatusCode::CREATED, Json(rule)))
}

/// PUT /api/rule/rules/{id}
pub async fn update_rule(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
    Json(body): Json<NewRule>,
) -> Result<Json<crate::models::Rule>, (StatusCode, String)> {
    let ok = api
        .store
        .update_rule(&tenant, &id, &body)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    if !ok {
        return Err((StatusCode::NOT_FOUND, "rule not found".into()));
    }
    let rules = api
        .store
        .list_rules(&tenant)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    rules
        .into_iter()
        .find(|r| r.id == id)
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "rule not found".into()))
}

/// DELETE /api/rule/rules/{id}
pub async fn delete_rule(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let ok = api
        .store
        .delete_rule(&tenant, &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    if ok { Ok(StatusCode::NO_CONTENT) } else { Err((StatusCode::NOT_FOUND, "rule not found".into())) }
}

/// GET /api/rule/channels：通知渠道列表。
pub async fn list_channels(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
) -> Result<Json<Vec<crate::models::NotifyChannel>>, (StatusCode, String)> {
    let channels = api
        .store
        .list_channels(&tenant)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(channels))
}

/// PUT /api/rule/channels/{channel}：创建或更新（单租户单渠道唯一）。
pub async fn upsert_channel(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
    Path(channel): Path<String>,
    Json(body): Json<NewNotifyChannel>,
) -> Result<Json<crate::models::NotifyChannel>, (StatusCode, String)> {
    let ch = api
        .store
        .upsert_channel(&tenant, &channel, &body)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(ch))
}

/// DELETE /api/rule/channels/{channel}
pub async fn delete_channel(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
    Path(channel): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let ok = api
        .store
        .delete_channel(&tenant, &channel)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    if ok { Ok(StatusCode::NO_CONTENT) } else { Err((StatusCode::NOT_FOUND, "channel not found".into())) }
}

/// GET /api/rule/stats：告警总数 + 未处理数。
pub async fn alert_stats(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let (total, active) = api
        .store
        .stats(&tenant)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::json!({ "total": total, "active": active })))
}

/// GET /api/rule/alerts?status=active|acknowledged
pub async fn list_alerts(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
    Query(q): Query<AlertQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let status = q.status.as_deref().filter(|s| !s.is_empty());
    if let Some(s) = status {
        if !["active", "acknowledged"].contains(&s) {
            return Err((StatusCode::BAD_REQUEST, "status must be active|acknowledged".into()));
        }
    }
    let alerts = api
        .store
        .list_alerts(&tenant, status)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::json!({ "alerts": alerts })))
}

/// POST /api/rule/alerts/{id}/ack
pub async fn ack_alert(
    State(api): State<ApiState>,
    tenant: axum::Extension<String>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let ok = api
        .store
        .ack_alert(&tenant, &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    if ok { Ok(StatusCode::NO_CONTENT) } else { Err((StatusCode::NOT_FOUND, "alert not found".into())) }
}
