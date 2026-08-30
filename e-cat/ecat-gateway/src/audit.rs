//! 审计落库 sink：写操作审计事件持久化到 MySQL audit_log 表。
//! 连接失败（如开发环境无 MySQL）降级为空实现——审计属可观测性增强，
//! 不阻塞网关启动；降级有日志告警。
use async_trait::async_trait;
use ecat_data::RdbmsClient;
use ecat_data_sqlx::SqlxClient;
use ecat_middleware::{AuditEvent, AuditSink};
use std::sync::Arc;

#[derive(Clone)]
pub struct MysqlAuditSink {
    db: Option<Arc<SqlxClient>>,
}

impl MysqlAuditSink {
    pub async fn connect(dsn: &str) -> Self {
        match SqlxClient::connect(dsn).await {
            Ok(db) => Self {
                db: Some(Arc::new(db)),
            },
            Err(e) => {
                tracing::warn!(error = %e, "audit sink db unavailable; audit disabled");
                Self { db: None }
            }
        }
    }
}

#[async_trait]
impl AuditSink for MysqlAuditSink {
    async fn record(&self, event: AuditEvent) {
        let Some(db) = &self.db else { return };
        let sql = "INSERT INTO audit_log (tenant_id, role, method, path, status) \
                   VALUES (?, ?, ?, ?, ?)";
        let params = vec![
            serde_json::Value::String(event.tenant_id),
            serde_json::Value::String(event.role),
            serde_json::Value::String(event.method),
            serde_json::Value::String(event.path),
            serde_json::Value::Number(event.status.into()),
        ];
        if let Err(e) = db.execute_with(sql, &params).await {
            tracing::warn!(error = %e, "audit record failed");
        }
    }
}
