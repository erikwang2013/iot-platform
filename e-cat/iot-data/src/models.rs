use serde::{Deserialize, Serialize};

/// 统一事件消息：契约定义在 ecat-iot（跨服务共享）。
pub use ecat_iot::EventMessage;

/// 历史曲线单点：ts 为 epoch 毫秒，value 为原始值（数值或字符串）。
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct HistoryPoint {
    pub ts: i64,
    pub value: serde_json::Value,
}
