//! 上报中继（A-2，docs/edge-protocol.md §3）：采集先落 SQLite 再发 MQTT，
//! 成功后删除；断网恢复后按 ts 升序补发，节流 ≤200 msg/s。
//! 与具体 MQTT/采集解耦：`Sender` trait 抽象"发一条属性报文"，可用假发送器测试。
use crate::buffer::{Buffer, BufferedPoint};

/// 补发节流上限（msg/s），协议文档 §3。
pub const MAX_REPLAY_RATE: u64 = 200;

/// 发送一条属性报文。返回 Ok 表示 broker 已确认（可删除缓冲）；Err 保留。
#[async_trait::async_trait]
pub trait Sender: Send + Sync {
    async fn send(&self, p: &BufferedPoint) -> Result<(), String>;
}

/// 新采集数据入口：先落库（事务）再尝试发送；发送成功即删，失败保留（断网积压）。
/// 返回是否已成功发送。
pub async fn ingest_and_send(
    buffer: &Buffer,
    sender: &dyn Sender,
    p: &BufferedPoint,
) -> bool {
    if let Err(e) = buffer.insert(p) {
        tracing::warn!(error = %e, "buffer insert failed; dropping point");
        return false;
    }
    match sender.send(p).await {
        Ok(()) => {
            if let Err(e) = buffer.remove(p) {
                tracing::warn!(error = %e, "buffer remove failed");
            }
            true
        }
        Err(e) => {
            tracing::debug!(error = %e, ts = p.ts, "send failed; buffered for replay");
            false
        }
    }
}

/// 补发积压：从最早（ts 升序）开始逐条发送，成功即删；按节流速率限速。
/// 返回本次成功补发条数。
pub async fn replay(buffer: &Buffer, sender: &dyn Sender) -> u64 {
    // 每次最多取一批（避免无限循环占用）；循环直到积压清空或全部发送失败。
    let mut sent = 0u64;
    loop {
        let pending = match buffer.drain_pending(MAX_REPLAY_RATE as usize) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "replay drain failed");
                break;
            }
        };
        if pending.is_empty() {
            break;
        }
        let mut batch_sent = 0u64;
        for p in pending {
            match sender.send(&p).await {
                Ok(()) => {
                    if let Err(e) = buffer.remove(&p) {
                        tracing::warn!(error = %e, "replay remove failed");
                    }
                    batch_sent += 1;
                }
                Err(_) => {
                    // 一条失败即停止本批（网络仍不可达），保留后续
                    tracing::debug!("replay send failed; pausing");
                    return sent + batch_sent;
                }
            }
        }
        sent += batch_sent;
        // 节流：整批以 ≤MAX_REPLAY_RATE msg/s 速率发送，批次间按比例暂停
        if batch_sent > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    sent
}

/// 心跳载荷（积压条数）。
pub fn heartbeat_payload(ts: i64, buffered: i64) -> serde_json::Value {
    serde_json::json!({ "ts": ts, "buffered": buffered })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    /// 记录收到的点；可配置失败以模拟断网。
    #[derive(Clone)]
    struct FakeSender {
        received: Arc<std::sync::Mutex<Vec<BufferedPoint>>>,
        fail: Arc<AtomicU64>,
    }
    impl FakeSender {
        fn ok() -> Self {
            Self {
                received: Arc::new(std::sync::Mutex::new(Vec::new())),
                fail: Arc::new(AtomicU64::new(0)),
            }
        }
        fn failing() -> Self {
            Self {
                received: Arc::new(std::sync::Mutex::new(Vec::new())),
                fail: Arc::new(AtomicU64::new(u64::MAX)),
            }
        }
        fn count(&self) -> usize {
            self.received.lock().unwrap().len()
        }
    }
    #[async_trait::async_trait]
    impl Sender for FakeSender {
        async fn send(&self, p: &BufferedPoint) -> Result<(), String> {
            if self.fail.load(Ordering::SeqCst) > 0 {
                self.fail.fetch_sub(1, Ordering::SeqCst);
                return Err("offline".into());
            }
            self.received.lock().unwrap().push(p.clone());
            Ok(())
        }
    }

    fn pt(device: &str, code: &str, ts: i64, value: &str) -> BufferedPoint {
        BufferedPoint {
            device_id: device.into(),
            code: code.into(),
            value_json: value.into(),
            ts,
        }
    }

    #[tokio::test]
    async fn ingest_send_success_removes_from_buffer() {
        let b = Buffer::in_memory().unwrap();
        let s = FakeSender::ok();
        assert!(ingest_and_send(&b, &s, &pt("d1", "temp", 1, "23.5")).await);
        // 已成功发送 → 缓冲清空
        assert_eq!(b.pending_count().unwrap(), 0);
        assert_eq!(s.count(), 1);
    }

    #[tokio::test]
    async fn ingest_send_failure_buffers_point() {
        let b = Buffer::in_memory().unwrap();
        let s = FakeSender::failing();
        assert!(!ingest_and_send(&b, &s, &pt("d1", "temp", 1, "23.5")).await);
        // 发送失败 → 点保留在缓冲（积压）
        assert_eq!(b.pending_count().unwrap(), 1);
    }

    #[tokio::test]
    async fn replay_sends_buffered_in_ts_order_and_clears() {
        let b = Buffer::in_memory().unwrap();
        let s = FakeSender::ok();
        // 先积压 3 条（发送失败）
        let s_fail = FakeSender::failing();
        for ts in [3, 1, 2] {
            ingest_and_send(&b, &s_fail, &pt("d1", "temp", ts, &ts.to_string())).await;
        }
        assert_eq!(b.pending_count().unwrap(), 3);
        // 恢复后补发
        let n = replay(&b, &s).await;
        assert_eq!(n, 3);
        assert_eq!(b.pending_count().unwrap(), 0);
        // 按 ts 升序到达
        let recv = s.received.lock().unwrap();
        let ts: Vec<i64> = recv.iter().map(|p| p.ts).collect();
        assert_eq!(ts, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn replay_stops_on_failure_preserving_remaining() {
        let b = Buffer::in_memory().unwrap();
        let s_fail = FakeSender::failing();
        for ts in [1, 2, 3] {
            ingest_and_send(&b, &s_fail, &pt("d1", "temp", ts, &ts.to_string())).await;
        }
        // 发送器持续失败 → 补发 0 条，全部保留
        let n = replay(&b, &s_fail).await;
        assert_eq!(n, 0);
        assert_eq!(b.pending_count().unwrap(), 3);
    }

    #[test]
    fn heartbeat_payload_shape() {
        let v = heartbeat_payload(1700000000, 42);
        assert_eq!(v["ts"], 1700000000);
        assert_eq!(v["buffered"], 42);
    }

    /// 断网重放验收（docs/edge-protocol.md §6 验收方法）。
    /// ponytail: 不真实 sleep 40 分钟 —— 时间抽象为设备侧 ts（每分钟一个采样点），
    /// 断网积压 30 条 ≤ MAX_REPLAY_RATE(200)，走单批补传，仅 1 次真实 1s 节流 sleep。
    #[tokio::test]
    async fn acceptance_offline_30min_full_replay_no_loss_no_dup() {
        const SAMPLE_MS: i64 = 60 * 1000; // 采样间隔 = 1 分钟
        const ONLINE_MIN: i64 = 10;       // §6-1 正常上报 10 分钟
        const OFFLINE_MIN: i64 = 30;      // §6-2 断网 30 分钟
        let start_ts: i64 = 1_725_000_000_000;

        let buffer = Buffer::in_memory().unwrap();
        let online = FakeSender::ok();
        let offline = FakeSender::failing();
        let recovered = FakeSender::ok();

        // §6-1 在线：采集即上报，缓冲不积压
        for i in 0..ONLINE_MIN {
            let ts = start_ts + i * SAMPLE_MS;
            assert!(ingest_and_send(&buffer, &online, &pt("edge-dev-1", "temperature", ts, "23.5")).await);
        }
        assert_eq!(online.count() as i64, ONLINE_MIN);
        assert_eq!(buffer.pending_count().unwrap(), 0, "在线期间不应积压");

        // §6-2 断网：采集继续，全部落 SQLite 积压
        let offline_ts: Vec<i64> = (0..OFFLINE_MIN)
            .map(|i| start_ts + (ONLINE_MIN + i) * SAMPLE_MS)
            .collect();
        for &ts in &offline_ts {
            assert!(!ingest_and_send(&buffer, &offline, &pt("edge-dev-1", "temperature", ts, "23.5")).await);
        }
        assert_eq!(buffer.pending_count().unwrap(), offline_ts.len() as i64, "断网期间应全部积压");
        assert_eq!(offline.count(), 0, "断网期间 broker 不应收到数据");

        // §6-3 恢复网络：积压完整补传
        let replayed = replay(&buffer, &recovered).await;
        assert_eq!(replayed, offline_ts.len() as u64, "补传条数应等于积压条数");
        assert_eq!(buffer.pending_count().unwrap(), 0, "补传后缓冲清零");

        // 无丢失 + ts 连续无空洞 + 按设备侧时间戳升序
        let got = std::mem::take(&mut *recovered.received.lock().unwrap());
        let ts: Vec<i64> = got.iter().map(|p| p.ts).collect();
        assert_eq!(ts, offline_ts, "重放按 ts 升序且无空洞");

        // 无重复：(device_id, code, ts) 唯一（边缘侧主键去重）
        let keys: std::collections::HashSet<_> =
            got.iter().map(|p| (p.device_id.clone(), p.code.clone(), p.ts)).collect();
        assert_eq!(keys.len(), got.len(), "同一数据点不得重复上报");
    }
}
