// Copyright (c) 2026 erik <erik@erik.xyz> — https://erik.xyz
//! 平台自生成主键：snowflake i64。
//!
//! 约定：DB 列 BIGINT 按数字读写（绑定/读取），JSON/API/struct 层为十进制字符串
//! （snowflake ≈ t<<22，自 epoch 约 25 天即超 2^53，JS 数字表达会丢精度）。
//! 外部设备标识（tenant_id/device 上报的 vendor 侧 id 等）一律不走这里。
use idgen_rs::{FastIdGenerator, IGOptions};
use std::sync::OnceLock;

/// epoch 固定 2026-01-01T00:00:00Z（毫秒）。库默认 epoch 为 2020，此处显式设定，
/// 与代码诞生时间对齐，留足低 63 位空间。
const EPOCH_MS: i64 = 1_767_225_600_000;

static GEN: OnceLock<FastIdGenerator> = OnceLock::new();

fn parse_worker_id(raw: &str) -> u16 {
    let worker_id: u16 = raw.parse().unwrap_or_else(|_| {
        panic!("SNOWFLAKE_WORKER_ID 必须为 0-1023 的数字，实际值：{raw:?}");
    });
    assert!(
        worker_id <= 1023,
        "SNOWFLAKE_WORKER_ID 超出 0-1023：{worker_id}。多副本部署请给每实例唯一 worker_id（10bit=1024 节点），防同 worker 撞号"
    );
    worker_id
}

/// hostname → 0..=1023 的确定性哈希（DefaultHasher 固定种子，跨进程/跨重启稳定）。
/// 哈希碰撞=同 ms 同 worker 撞号，10bit 空间固有边界；多副本需钉住时显式配
/// SNOWFLAKE_WORKER_ID（此时 hostname 分支不生效）。
fn worker_id_for_hostname(hostname: &str) -> u16 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    h.write(hostname.as_bytes());
    (h.finish() % 1024) as u16
}

/// worker 号三阶解析（纯函数，env 由调用方注入便于测试）：
/// SNOWFLAKE_WORKER_ID 显式 → 校验后使用（越界/非数字 panic）；
/// 否则 HOSTNAME（k8s/docker 运行时必设，天然唯一）哈希 → 1024 内；
/// 都没有（裸机本地开发）→ 0。返回 (worker_id, 来源)。
fn resolve_from(explicit: Option<&str>, hostname: Option<&str>) -> (u16, &'static str) {
    match explicit {
        Some(raw) => (parse_worker_id(raw), "env"),
        None => match hostname {
            Some(h) => (worker_id_for_hostname(h), "hostname"),
            None => (0, "default"),
        },
    }
}

fn init() -> &'static FastIdGenerator {
    GEN.get_or_init(|| {
        let (worker_id, source) = resolve_from(
            std::env::var("SNOWFLAKE_WORKER_ID").ok().as_deref(),
            std::env::var("HOSTNAME").ok().as_deref(),
        );
        tracing::info!(worker_id, source, "snowflake id generator initialized");
        FastIdGenerator::new(
            &IGOptions::builder(worker_id)
                .worker_id_bit_length(10) // 1024 节点
                .seq_bit_length(12) // 4096/ms
                .base_time_ms(EPOCH_MS)
                .build(),
        )
    })
}

/// 生成下一个平台主键（DB BIGINT 侧绑定数字；需要 String 时 `.to_string()`）。
/// 懒初始化：首次调用读 env。时钟回跳 >10ms 时库内直接 panic（防重复 ID）。
pub fn next_id() -> i64 {
    init().next_id() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn next_id_monotonic() {
        let mut prev = next_id();
        for _ in 0..10_000 {
            let cur = next_id();
            assert!(cur > prev, "snowflake id 必须严格递增：{prev} >= {cur}");
            prev = cur;
        }
    }

    #[test]
    fn next_id_concurrent_unique() {
        let handles: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| (0..2_000).map(|_| next_id()).collect::<Vec<_>>()))
            .collect();
        let all: Vec<i64> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();
        let unique: HashSet<i64> = all.iter().copied().collect();
        assert_eq!(unique.len(), all.len(), "8 线程 × 2000 次出现重复 id");
    }

    #[test]
    #[should_panic(expected = "SNOWFLAKE_WORKER_ID")]
    fn worker_id_rejects_non_numeric() {
        parse_worker_id("abc");
    }

    #[test]
    #[should_panic(expected = "SNOWFLAKE_WORKER_ID 超出 0-1023")]
    fn worker_id_rejects_out_of_range() {
        parse_worker_id("1024");
    }

    #[test]
    fn worker_id_bounds() {
        assert_eq!(parse_worker_id("0"), 0);
        assert_eq!(parse_worker_id("1023"), 1023);
    }

    #[test]
    fn hostname_hash_stable_and_in_range() {
        // 纯函数不触碰进程全局 env，可并行；同输入跨进程/跨调用稳定
        for name in ["pod-a", "iot-access-7f9c2d", "localhost", "", "主机-01"] {
            assert_eq!(worker_id_for_hostname(name), worker_id_for_hostname(name));
            let w = worker_id_for_hostname(name);
            assert!((0..=1023).contains(&w), "{name} → {w} 越界");
        }
    }

    #[test]
    fn explicit_env_takes_precedence() {
        // SNOWFLAKE_WORKER_ID 显式时优先生效（来源 env），hostname 存在也不走哈希
        assert_eq!(resolve_from(Some("7"), Some("pod-x")), (7, "env"));
        // 越界/非数字 panic 路径不变（parse_worker_id 纯函数，上方 #should_panic 已覆盖）
        assert_eq!(resolve_from(Some("0"), None), (0, "env"));
    }

    #[test]
    fn hostname_fallback_used_without_explicit_env() {
        let (w, src) = resolve_from(None, Some("iot-device-abc"));
        assert_eq!(src, "hostname");
        assert_eq!(w, worker_id_for_hostname("iot-device-abc"));
    }

    #[test]
    fn no_env_no_hostname_defaults_to_zero() {
        assert_eq!(resolve_from(None, None), (0, "default"));
    }
}
