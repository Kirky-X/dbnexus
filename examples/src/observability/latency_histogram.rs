// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 延迟直方图与慢查询示例
//!
//! 演示 `LatencyHistogram`、`HistogramStats`、`LatencyPercentiles` 以及
//! `SlowQueryConfig` / `SlowQueryRecord` 的完整使用流程：
//! - 独立使用 `LatencyHistogram` 记录查询延迟
//! - 展示 `HistogramStats`（桶分布、累计样本数）
//! - 展示 `LatencyPercentiles`（P50/P75/P90/P95/P99/P99.9）
//! - 通过 `MetricsCollector` 配置 `SlowQueryConfig` 并采集 `SlowQueryRecord`
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example latency_histogram --features "sqlite,metrics"
//! ```

use dbnexus::{
    HistogramStats, LatencyHistogram, LatencyPercentiles, MetricsCollector, SlowQueryConfig,
    SlowQueryRecord,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("📈 DBNexus latency histogram and slow query example");
    println!("========================================\n");

    // ============================================
    // 1. 独立使用 LatencyHistogram
    // ============================================
    println!("--- 1. LatencyHistogram standalone ---");
    // 定义桶边界（毫秒）：≤1ms, ≤5ms, ≤10ms, ≤50ms, ≤100ms, ≤500ms, ≤1s, >1s
    let bucket_boundaries: Vec<u64> = vec![1, 5, 10, 50, 100, 500, 1000];
    let histogram = LatencyHistogram::new(bucket_boundaries.clone());

    // 模拟记录一组查询延迟
    let samples = [
        Duration::from_micros(500),  // 0.5ms → 落入 ≤1ms 桶
        Duration::from_millis(2),    // 2ms → 落入 ≤5ms 桶
        Duration::from_millis(7),    // 7ms → 落入 ≤10ms 桶
        Duration::from_millis(15),   // 15ms → 落入 ≤50ms 桶
        Duration::from_millis(75),   // 75ms → 落入 ≤100ms 桶
        Duration::from_millis(250),  // 250ms → 落入 ≤500ms 桶
        Duration::from_millis(750),  // 750ms → 落入 ≤1s 桶
        Duration::from_millis(1500), // 1500ms → 落入 >1s 溢出桶
    ];
    for s in &samples {
        histogram.record(*s);
    }
    println!("  ✓ recorded {} samples", samples.len());

    // ============================================
    // 2. 展示 HistogramStats（桶分布）
    // ============================================
    println!("\n--- 2. HistogramStats bucket distribution ---");
    let stats: HistogramStats = histogram.stats();
    println!("  total_samples = {}", stats.total_samples);
    println!("  bucket distribution:");
    for bucket in &stats.buckets {
        let boundary_str = if bucket.boundary_ms == u64::MAX {
            "+Inf".to_string()
        } else {
            format!("≤{}ms", bucket.boundary_ms)
        };
        println!(
            "    {:>8}: count={}, cumulative={}, percentile={:.2}%",
            boundary_str, bucket.count, bucket.cumulative_count, bucket.percentile
        );
    }

    // ============================================
    // 3. 通过 MetricsCollector 展示 LatencyPercentiles
    // ============================================
    println!("\n--- 3. LatencyPercentiles latency percentiles ---");
    let collector = MetricsCollector::new();

    // 记录 100 条延迟数据（1ms ~ 100ms 线性分布）
    for i in 1..=100u64 {
        collector.record_query("SELECT", Duration::from_millis(i), true, Some(i * 100));
    }
    // 补充几条长尾延迟
    collector.record_query("SELECT", Duration::from_millis(200), true, None);
    collector.record_query("SELECT", Duration::from_millis(500), true, None);

    let query_stats = collector
        .get_query_stats("SELECT")
        .expect("the SELECT statistics should exist");
    let percentiles: &LatencyPercentiles = &query_stats.latency_percentiles;
    println!("  sample count: {}", percentiles.sample_count);
    println!("  P50  = {:?}", percentiles.p50());
    println!("  P75  = {:?}", percentiles.p75());
    println!("  P90  = {:?}", percentiles.p90());
    println!("  P95  = {:?}", percentiles.p95());
    println!("  P99  = {:?}", percentiles.p99());
    println!("  P99.9= {:?}", percentiles.p999());
    println!("  min  = {:?}", percentiles.min());
    println!("  max  = {:?}", percentiles.max());

    // ============================================
    // 4. SlowQueryConfig 与 SlowQueryRecord
    // ============================================
    println!("\n--- 4. SlowQueryConfig slow query configuration ---");
    let slow_config = SlowQueryConfig {
        threshold_ms: 100,
        enabled: true,
    };
    println!(
        "  configuration: enabled={}, threshold_ms={}",
        slow_config.enabled, slow_config.threshold_ms
    );

    // 通过 MetricsCollector 应用配置并触发慢查询记录
    // （ collector 默认 threshold=1000ms，这里调整为 50ms 以便捕获更多慢查询）
    collector.set_slow_query_threshold(50);
    collector.set_slow_query_enabled(true);
    println!("  ✓ threshold adjusted to 50ms (default 1000ms)");

    // 上面已记录的 200ms 和 500ms 查询会被识别为慢查询
    // 注意：set_slow_query_threshold 只影响后续 record_query 的判定
    // 我们再记录几条明确的慢查询
    collector.record_query("SELECT", Duration::from_millis(80), true, None);
    collector.record_query("INSERT", Duration::from_millis(150), true, None);
    collector.record_query("UPDATE", Duration::from_millis(300), false, None);

    let slow_queries: Vec<SlowQueryRecord> = collector.slow_queries();
    println!("\n  slow query records (threshold=50ms):");
    println!("  {} slow queries in total", slow_queries.len());
    for (idx, sq) in slow_queries.iter().enumerate() {
        println!(
            "    #{}: type={}, duration_ms={}, timestamp={}",
            idx + 1,
            sq.query_type,
            sq.duration_ms,
            sq.timestamp
        );
    }

    // ============================================
    // 5. 禁用慢查询记录的对比
    // ============================================
    println!("\n--- 5. disable slow query recording ---");
    let disabled_collector = MetricsCollector::new();
    disabled_collector.set_slow_query_enabled(false);
    disabled_collector.set_slow_query_threshold(10);
    disabled_collector.record_query("SELECT", Duration::from_millis(500), true, None);
    let disabled_slow = disabled_collector.slow_queries();
    println!(
        "  with enabled=false, the slow query count after recording a 500ms query = {} (should be 0)",
        disabled_slow.len()
    );

    println!("\n========================================");
    println!("✨ Latency histogram and slow query example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - LatencyHistogram::new(bucket_boundaries)   - create a histogram");
    println!("  - histogram.record(duration)                 - record a latency");
    println!("  - histogram.stats() -> HistogramStats        - get the bucket statistics");
    println!("  - HistogramStats.buckets                     - bucket distribution details");
    println!("  - MetricsCollector::get_query_stats          - get percentiles");
    println!("  - LatencyPercentiles: p50/p75/p90/p95/p99    - latency percentiles");
    println!("  - SlowQueryConfig {{ threshold_ms, enabled }}  - slow query configuration");
    println!("  - collector.set_slow_query_threshold(ms)     - adjust the threshold");
    println!("  - collector.slow_queries() -> Vec<SlowQueryRecord> - the slow query list");

    Ok(())
}
