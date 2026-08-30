use crate::models::AlertMessage;
use futures_util::StreamExt;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
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

/// Redis pub/sub 桥接通道名（固定单通道，消息内携带租户；实例按租户分发到本地 hub）。
pub const ALERT_CHANNEL: &str = "iot:alerts";

/// 跨实例广播信封：instance 为发布实例 ID，订阅端据此跳过自己发的消息
/// （避免本地重复投递——本地直达已在 publish 完成）。
#[derive(Serialize, Deserialize)]
struct BridgeEnvelope {
    instance: String,
    alert: AlertMessage,
}

/// 告警跨实例广播桥：本地直达 + Redis pub/sub 扇出。
/// - Redis 可用：publish 本地直达并发布到 Redis；订阅端收到后分发给本地 WS。
/// - Redis 不可用：降级仅本地直达（fail-open，日志告警，与其他中间件降级策略一致）。
#[derive(Clone)]
pub struct AlertBridge {
    hub: PushHub,
    redis: Option<redis::aio::MultiplexedConnection>,
    instance: Arc<str>,
}

impl AlertBridge {
    pub async fn connect(hub: PushHub, redis_url: &str) -> Self {
        let redis = match redis::Client::open(redis_url) {
            Ok(client) => match client.get_multiplexed_async_connection().await {
                Ok(conn) => Some(conn),
                Err(e) => {
                    tracing::warn!(error = %e, "alert bridge redis unavailable; local-only push");
                    None
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "alert bridge redis url invalid; local-only push");
                None
            }
        };
        Self {
            hub,
            redis,
            instance: Arc::from(uuid::Uuid::new_v4().to_string()),
        }
    }

    /// 发布告警：本地直达 + Redis 扇出（Redis 失败仅本地，下次重试自然恢复）。
    pub async fn publish(&self, tenant: &str, msg: &AlertMessage) {
        self.hub.publish(tenant, msg);
        let Some(redis) = &self.redis else { return };
        // 克隆连接句柄（Arc 共享流水线，开销可忽略）以满足 AsyncCommands 的 &mut
        let mut conn = redis.clone();
        let envelope = BridgeEnvelope {
            instance: self.instance.to_string(),
            alert: msg.clone(),
        };
        let payload = serde_json::to_vec(&envelope).unwrap_or_default();
        if let Err(e) = conn.publish::<_, _, i64>(ALERT_CHANNEL, payload).await {
            tracing::warn!(error = %e, "alert bridge redis publish failed; local-only this time");
        }
    }

    /// 订阅任务：接收其他实例的告警并分发到本地 hub。断线自动重连（1s 退避）。
    pub async fn spawn_subscriber(hub: PushHub, redis_url: String) {
        let instance = uuid::Uuid::new_v4().to_string();
        loop {
            let client = match redis::Client::open(redis_url.as_str()) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "alert bridge subscriber: bad redis url");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            };
            let mut pubsub = match client.get_async_pubsub().await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "alert bridge subscriber: connect failed");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            };
            if let Err(e) = pubsub.subscribe(ALERT_CHANNEL).await {
                tracing::warn!(error = %e, "alert bridge subscriber: subscribe failed");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
            tracing::info!("alert bridge subscriber: listening on redis channel {ALERT_CHANNEL}");
            let mut stream = pubsub.into_on_message();
            while let Some(msg) = stream.next().await {
                let payload: Vec<u8> = match msg.get_payload() {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!(error = %e, "alert bridge subscriber: bad payload");
                        continue;
                    }
                };
                let envelope: BridgeEnvelope = match serde_json::from_slice(&payload) {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!(error = %e, "alert bridge subscriber: drop unparseable");
                        continue;
                    }
                };
                if envelope.instance == instance {
                    continue; // 本地直达已投递，跳过回声
                }
                hub.publish(&envelope.alert.tenant_id, &envelope.alert);
            }
            tracing::warn!("alert bridge subscriber: stream ended; reconnecting in 1s");
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}
