// Copyright (c) 2026 erik <erik@erik.xyz> — https://erik.xyz
//! GET /api/search：跨 devices/alerts/logs 的多字段模糊检索（OpenSearch）。
//! 只读端点，租户隔离强制（DSL 内 term 过滤 tenant_id，取自 JWT sub）。
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use ecat_auth::AuthClaims;
use ecat_data::SearchClient;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

/// 可检索索引白名单：客户端仅可访问受控索引（防任意索引探测）。
const INDEXES: [&str; 3] = ["devices", "alerts", "logs"];

#[derive(Clone)]
pub struct SearchState(pub Option<Arc<dyn SearchClient>>);

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub index: String,
    /// 页码，从 1 起
    #[serde(default = "default_page")]
    pub page: u32,
    /// 每页条数，默认 20，上限 100
    #[serde(default = "default_size")]
    pub size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_size() -> u32 {
    20
}

/// GET /api/search?q=<term>&index=devices|alerts|logs&page=1&size=20
/// q 缺省/空白 → match_all（仅租户过滤）。搜索未配置（client 为 None）→ 503。
pub async fn search(
    State(state): State<SearchState>,
    claims: axum::Extension<AuthClaims>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let Some(client) = &state.0 else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "search not configured".into()));
    };
    if !INDEXES.contains(&q.index.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            "index must be devices|alerts|logs".into(),
        ));
    }
    let page = q.page.clamp(1, 10_000);
    let size = q.size.clamp(1, 100);
    let body = match q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(term) => json!({
            "query": {"bool": {
                "filter": [{"term": {"tenant_id": claims.sub}}],
                "must": [{"multi_match": {"query": term, "fields": ["*"]}}],
            }},
            "from": (page - 1) * size,
            "size": size,
        }),
        None => json!({
            "query": {"bool": {"filter": [{"term": {"tenant_id": claims.sub}}]}},
            "from": (page - 1) * size,
            "size": size,
        }),
    };
    let resp = client
        .search(&q.index, &body)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("search: {e}")))?;
    let total = resp["hits"]["total"]["value"].as_u64().unwrap_or(0);
    let hits: Vec<Value> = resp["hits"]["hits"]
        .as_array()
        .map(|arr| arr.iter().map(|h| h["_source"].clone()).collect())
        .unwrap_or_default();
    Ok(Json(json!({
        "index": q.index,
        "q": q.q,
        "page": page,
        "size": size,
        "total": total,
        "hits": hits,
    })))
}
