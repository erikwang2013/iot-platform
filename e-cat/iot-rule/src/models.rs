use serde::{Deserialize, Serialize};

/// 统一事件消息：契约定义在 ecat-iot（跨服务共享）。
pub use ecat_iot::EventMessage;

/// 阈值规则（rules 表行）。operator：gt|gte|lt|lte|eq|neq。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rule {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub device_id: String,
    pub code: String,
    pub operator: String,
    pub threshold: f64,
    /// 命中时额外 POST 告警 JSON 到该 URL；None = 仅推送
    pub webhook_url: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建/更新规则的请求体（id/tenant_id/时间戳由服务端生成）。
#[derive(Deserialize, Clone, Debug)]
pub struct NewRule {
    pub name: String,
    pub device_id: String,
    pub code: String,
    pub operator: String,
    pub threshold: f64,
    pub webhook_url: Option<String>,
    pub enabled: Option<bool>,
}

/// WebSocket 推送与 webhook 载荷（引擎产出物）。
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AlertMessage {
    pub rule_id: String,
    pub rule_name: String,
    pub tenant_id: String,
    pub device_id: String,
    pub code: String,
    pub operator: String,
    pub threshold: f64,
    pub value: serde_json::Value,
    pub ts: i64,
}

/// 告警记录（alert_records 表行）。status：active|acknowledged。
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AlertRecord {
    pub id: String,
    pub rule_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub code: String,
    pub operator: String,
    pub threshold: f64,
    pub value: serde_json::Value,
    pub status: String,
    pub created_at: String,
}
