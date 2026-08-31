// Copyright (c) 2026 erik <erik@erik.xyz> — https://erik.xyz
//! 检索后端工厂：按 env 选择 OpenSearch / Elasticsearch 客户端，返回
//! `SearchClient` trait 对象。各服务只需依赖本 crate，不必直接依赖 impl crate。
use ecat_data::SearchClient;
use std::sync::Arc;

static CLIENT: std::sync::OnceLock<Option<Arc<dyn SearchClient>>> = std::sync::OnceLock::new();

/// 按 env 建立检索客户端（进程内缓存，只建一次）：
/// - `SEARCH_KIND` = `opensearch`（默认）| `elasticsearch`
/// - 对应 URL env `OPENSEARCH_URL` / `ELASTICSEARCH_URL` **未设置 → None（禁用）**：
///   写入侧静默跳过、读 API 返回 503，不破坏无 OpenSearch 的本地环境。
pub fn connect_search() -> Option<Arc<dyn SearchClient>> {
    CLIENT.get_or_init(build_from_env).clone()
}

fn build_from_env() -> Option<Arc<dyn SearchClient>> {
    let kind = std::env::var("SEARCH_KIND")
        .unwrap_or_else(|_| "opensearch".into())
        .to_ascii_lowercase();
    match kind.as_str() {
        "elasticsearch" => std::env::var("ELASTICSEARCH_URL")
            .ok()
            .map(|url| Arc::new(ecat_data_elasticsearch::ElasticsearchClient::new(url)) as Arc<dyn SearchClient>),
        _ => std::env::var("OPENSEARCH_URL")
            .ok()
            .map(|url| Arc::new(ecat_data_opensearch::OpenSearchClient::new(url)) as Arc<dyn SearchClient>),
    }
}
