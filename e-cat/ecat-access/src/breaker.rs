//! 厂商 API 熔断（B-2）：轻量状态机（closed → open → half-open → closed），
//! 按滑动窗口失败率触发熔断；open 期间直接拒绝（降级到缓存，见 api.rs），
//! 冷却期后 half-open 探测，成功即恢复。
//! 设计对齐 ecat-circuit-breaker 语义，但不强制 Tower 重构（适配器直连 reqwest）。
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Closed,
    Open,
    HalfOpen,
}

/// 熔断器配置：失败率阈值 / 窗口 / 冷却期 / half-open 探测数。
#[derive(Clone, Copy)]
pub struct BreakerConfig {
    /// 窗口内失败率达到该比例触发 open（默认 0.5）
    pub failure_ratio: f64,
    /// 统计窗口时长
    pub window: Duration,
    /// open 冷却期，过后进入 half-open
    pub open_duration: Duration,
    /// half-open 阶段允许的探测请求数
    pub half_open_probes: u32,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_ratio: 0.5,
            window: Duration::from_secs(10),
            open_duration: Duration::from_secs(30),
            half_open_probes: 3,
        }
    }
}

struct Inner {
    state: State,
    window_successes: u32,
    window_failures: u32,
    window_start: Instant,
    opened_at: Option<Instant>,
    half_open_count: u32,
}

/// 线程安全熔断器（Mutex 保护内部状态）。clone 共享同一底层状态。
#[derive(Clone)]
pub struct CircuitBreaker {
    inner: std::sync::Arc<Mutex<Inner>>,
    config: BreakerConfig,
}

impl CircuitBreaker {
    pub fn new(config: BreakerConfig) -> Self {
        Self {
            inner: std::sync::Arc::new(Mutex::new(Inner {
                state: State::Closed,
                window_successes: 0,
                window_failures: 0,
                window_start: Instant::now(),
                opened_at: None,
                half_open_count: 0,
            })),
            config,
        }
    }

    /// 判断请求是否应放行。返回 true 放行；false 熔断（调用方降级/报错）。
    pub fn allow(&self) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        self.rotate(&mut g);
        match g.state {
            State::Closed => true,
            State::Open => {
                // 冷却期结束 → half-open，放行首个探测请求
                if g.opened_at.map_or(false, |t| t.elapsed() >= self.config.open_duration) {
                    tracing::info!("circuit breaker: open → half-open");
                    g.state = State::HalfOpen;
                    g.half_open_count = 0;
                    true
                } else {
                    false
                }
            }
            State::HalfOpen => {
                if g.half_open_count >= self.config.half_open_probes {
                    false
                } else {
                    g.half_open_count += 1;
                    true
                }
            }
        }
    }

    /// 记录一次请求结果：success=true 成功，false 失败。
    pub fn record(&self, success: bool) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        self.rotate(&mut g);
        if success {
            g.window_successes += 1;
        } else {
            g.window_failures += 1;
        }
        match g.state {
            State::Closed => {
                // 窗口内样本达到 5 且失败率超阈值 → open
                if g.window_successes + g.window_failures >= 5
                    && self.failure_ratio(&g) >= self.config.failure_ratio
                {
                    tracing::warn!("circuit breaker: closed → open");
                    g.state = State::Open;
                    g.opened_at = Some(Instant::now());
                }
            }
            State::HalfOpen => {
                if success {
                    tracing::info!("circuit breaker: half-open → closed");
                    g.state = State::Closed;
                    g.opened_at = None;
                    g.window_successes = 0;
                    g.window_failures = 0;
                    g.window_start = Instant::now();
                } else {
                    tracing::warn!("circuit breaker: half-open → open (probe failed)");
                    g.state = State::Open;
                    g.opened_at = Some(Instant::now());
                }
            }
            State::Open => {}
        }
    }

    fn rotate(&self, g: &mut Inner) {
        if g.window_start.elapsed() >= self.config.window {
            g.window_successes = 0;
            g.window_failures = 0;
            g.window_start = Instant::now();
        }
    }

    fn failure_ratio(&self, g: &Inner) -> f64 {
        let total = g.window_successes + g.window_failures;
        if total == 0 {
            0.0
        } else {
            g.window_failures as f64 / total as f64
        }
    }

    /// 当前是否处于熔断状态（供观测）。
    pub fn is_open(&self) -> bool {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        matches!(g.state, State::Open)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_allows_all() {
        let b = CircuitBreaker::new(BreakerConfig::default());
        assert!(b.allow());
        b.record(true);
        assert!(!b.is_open());
    }

    #[test]
    fn failures_trip_open_then_reject() {
        // 短窗口 + 低阈值：5 次失败触发 open
        let cfg = BreakerConfig {
            failure_ratio: 0.5,
            window: Duration::from_millis(100),
            open_duration: Duration::from_secs(60),
            half_open_probes: 3,
        };
        let b = CircuitBreaker::new(cfg);
        for _ in 0..5 {
            assert!(b.allow(), "closed must allow");
            b.record(false);
        }
        assert!(b.is_open(), "5 failures must trip open");
        assert!(!b.allow(), "open must reject");
    }

    #[test]
    fn cooldown_enters_half_open_and_recovers() {
        let cfg = BreakerConfig {
            failure_ratio: 0.5,
            window: Duration::from_millis(50),
            open_duration: Duration::from_millis(50),
            half_open_probes: 3,
        };
        let b = CircuitBreaker::new(cfg);
        for _ in 0..5 {
            b.record(false);
        }
        assert!(b.is_open());
        // 冷却期后首个请求作为 half-open 探测放行
        std::thread::sleep(Duration::from_millis(80));
        assert!(b.allow(), "half-open must allow probe");
        b.record(true); // 探测成功 → 恢复 closed
        assert!(!b.is_open(), "successful probe must close the circuit");
        assert!(b.allow());
    }

    #[test]
    fn half_open_probe_failure_reopens() {
        let cfg = BreakerConfig {
            failure_ratio: 0.5,
            window: Duration::from_millis(50),
            open_duration: Duration::from_millis(50),
            half_open_probes: 3,
        };
        let b = CircuitBreaker::new(cfg);
        for _ in 0..5 {
            b.record(false);
        }
        std::thread::sleep(Duration::from_millis(80));
        assert!(b.allow());
        b.record(false); // 探测失败 → 重新 open
        assert!(b.is_open(), "failed probe must reopen circuit");
        assert!(!b.allow(), "reopened circuit must reject");
    }
}
