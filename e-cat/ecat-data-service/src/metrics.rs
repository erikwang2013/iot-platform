//! 业务指标（C-3）：在框架自动挂载的 /metrics 之外，登记本服务专有指标。
//! 注册于 ecat-metrics 全局 registry，Prometheus scrape /metrics 自动暴露。
use prometheus::{IntCounter, Opts};

/// 在 ecat-metrics 全局 registry 上注册计数器（幂等：AlreadyReg 无害）。
fn register_counter(name: &str, help: &str) -> IntCounter {
    let m = IntCounter::with_opts(Opts::new(name, help)).expect("valid counter opts");
    // AlreadyReg（重复注册）可忽略——进程内每个名字只应注册一次。
    let _ = ecat_metrics::registry().register(Box::new(m.clone()));
    m
}

/// 已入库事件数（TDengine 批量写成功累加）。
fn events_ingested() -> IntCounter {
    static M: std::sync::OnceLock<IntCounter> = std::sync::OnceLock::new();
    M.get_or_init(|| {
        register_counter("iot_events_ingested_total", "Total events ingested to TDengine")
    })
    .clone()
}

/// TDengine 批量写失败次数。
fn td_write_failures() -> IntCounter {
    static M: std::sync::OnceLock<IntCounter> = std::sync::OnceLock::new();
    M.get_or_init(|| {
        register_counter("iot_td_write_failures_total", "TDengine batch write failures")
    })
    .clone()
}

/// 记录一次成功入库（n 条事件）。
pub fn record_ingested(n: u64) {
    events_ingested().inc_by(n);
}

/// 记录一次 TDengine 批量写失败。
pub fn record_write_failure() {
    td_write_failures().inc();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_register_and_increment() {
        // 幂等：重复调用不 panic（OnceLock 保护）
        record_ingested(5);
        record_ingested(2);
        record_write_failure();
        // 全局 registry 文本应包含指标名
        let text = ecat_metrics::metrics_text();
        assert!(text.contains("iot_events_ingested_total"), "got: {text}");
        assert!(text.contains("iot_td_write_failures_total"), "got: {text}");
    }
}
