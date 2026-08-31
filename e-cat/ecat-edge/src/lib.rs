//! 轻量边缘网关（A-2）：本地 SQLite 缓冲 + MQTT 上报 + 断网补传。
//! 协议见 docs/edge-protocol.md。核心逻辑（缓冲/重放）与运行时解耦，
//! 便于无 broker 的单元测试。
pub mod buffer;
pub mod relay;
