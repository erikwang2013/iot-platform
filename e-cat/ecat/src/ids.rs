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

fn init() -> &'static FastIdGenerator {
    GEN.get_or_init(|| {
        let raw = std::env::var("SNOWFLAKE_WORKER_ID").unwrap_or_else(|_| "0".into());
        let worker_id = parse_worker_id(&raw);
        tracing::info!(worker_id, "snowflake id generator initialized");
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
}
