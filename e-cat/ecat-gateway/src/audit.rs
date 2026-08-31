//! 审计落库 sink：写操作审计事件持久化到 MySQL audit_log 表。
//! 连接失败（如开发环境无 MySQL）降级为空实现——审计属可观测性增强，
//! 不阻塞网关启动；降级有日志告警。
use async_trait::async_trait;
use ecat_data::{RdbmsClient, SearchClient};
use ecat_data_sqlx::SqlxClient;
use ecat_middleware::{AuditEvent, AuditSink};
use serde_json::{Value, json};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct MysqlAuditSink {
    db: Option<Arc<SqlxClient>>,
    search: Option<Arc<dyn SearchClient>>,
}

impl MysqlAuditSink {
    pub async fn connect(dsn: &str) -> Self {
        match SqlxClient::connect(dsn).await {
            Ok(db) => Self {
                db: Some(Arc::new(db)),
                search: None,
            },
            Err(e) => {
                tracing::warn!(error = %e, "audit sink db unavailable; audit disabled");
                Self {
                    db: None,
                    search: None,
                }
            }
        }
    }

    /// 挂接检索索引：审计落库成功后同步 index logs 文档（未配置时跳过）。
    pub fn with_search(mut self, search: Option<Arc<dyn SearchClient>>) -> Self {
        self.search = search;
        self
    }
}

#[async_trait]
impl AuditSink for MysqlAuditSink {
    async fn record(&self, event: AuditEvent) {
        let Some(db) = &self.db else { return };
        // 索引文档先于 INSERT 构造（INSERT 会 move event 字段）。
        // doc id 取 tenant|method|path|ts 的 hash。
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut h = DefaultHasher::new();
        format!("{}|{}|{}|{}", event.tenant_id, event.method, event.path, ts_ms).hash(&mut h);
        let doc = json!({
            "tenant_id": &event.tenant_id,
            "role": &event.role,
            "method": &event.method,
            "path": &event.path,
            "status": event.status,
            "created_at": ts_ms,
        });
        let sql = "INSERT INTO audit_log (tenant_id, role, method, path, status) \
                   VALUES (?, ?, ?, ?, ?)";
        let params = vec![
            Value::String(event.tenant_id),
            Value::String(event.role),
            Value::String(event.method),
            Value::String(event.path),
            Value::Number(event.status.into()),
        ];
        if let Err(e) = db.execute_with(sql, &params).await {
            tracing::warn!(error = %e, "audit record failed");
            return;
        }
        // 同步索引日志（logs 索引，doc 无删除——审计保留期外由索引生命周期管理；
        // ponytail: 索引失败仅 warn 不阻断审计主流程）。
        if let Some(search) = &self.search
            && let Err(e) = search.index("logs", &h.finish().to_string(), &doc).await
        {
            tracing::warn!(error = %e, "audit log index failed");
        }
    }
}
