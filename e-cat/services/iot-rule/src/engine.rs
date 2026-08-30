use crate::models::{AlertMessage, AlertRecord};

/// Task 5 实现：AlertMessage → AlertRecord（id/status/created_at 服务端生成）。
pub fn to_alert_record(msg: &AlertMessage) -> AlertRecord {
    AlertRecord {
        id: uuid::Uuid::new_v4().to_string(),
        rule_id: msg.rule_id.clone(),
        tenant_id: msg.tenant_id.clone(),
        device_id: msg.device_id.clone(),
        code: msg.code.clone(),
        operator: msg.operator.clone(),
        threshold: msg.threshold,
        value: msg.value.clone(),
        status: "active".into(),
        created_at: String::new(),
    }
}
