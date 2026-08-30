use crate::models::AlertMessage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// 单通道容量：慢消费者会丢最旧消息（broadcast 语义），WS 侧只是展示，可接受。
const CHANNEL_CAPACITY: usize = 256;

/// 每租户一个 broadcast channel：send/subscribe 均为同步操作，
/// 用 std::sync::Mutex 保护注册表即可（临界区极短），无需 async 锁。
#[derive(Clone)]
pub struct PushHub {
    tenants: Arc<Mutex<HashMap<String, broadcast::Sender<AlertMessage>>>>,
}

impl PushHub {
    pub fn new() -> Self {
        Self { tenants: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// 向租户广播告警；无订阅者时 send 返回 Err，忽略（不保留历史）。
    pub fn publish(&self, tenant: &str, msg: &AlertMessage) {
        let sender = self.sender(tenant);
        let _ = sender.send(msg.clone());
    }

    pub fn subscribe(&self, tenant: &str) -> broadcast::Receiver<AlertMessage> {
        self.sender(tenant).subscribe()
    }

    fn sender(&self, tenant: &str) -> broadcast::Sender<AlertMessage> {
        let mut map = self.tenants.lock().unwrap();
        map.entry(tenant.to_string())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .clone()
    }
}
