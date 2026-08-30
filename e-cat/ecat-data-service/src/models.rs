/// 统一事件消息：契约定义在 ecat-iot（跨服务共享）。
pub use ecat_iot::EventMessage;

/// 历史曲线单点：框架 TDengine 工具类型（ts 为 epoch 毫秒，value 为原始值）。
pub type HistoryPoint = ecat_data_tdengine::sql::TsPoint;
