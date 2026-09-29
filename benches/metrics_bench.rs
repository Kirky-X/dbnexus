// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 指标系统性能基准测试
//!
//! 衡量 `MetricsCollector` 与 `LatencyHistogram` 的核心操作开销：
//! - P50/P90/P99 计算（`record_query` + `get_query_stats` + 百分位读取）
//! - Prometheus 导出（`export_prometheus`）
//! - 直方图记录（`LatencyHistogram::record` + `stats`）
//!
//! 运行: cargo bench --bench metrics_bench --features "metrics"

#![cfg(feature = "metrics")]

use criterion::{Criterion, criterion_group, criterion_main};
use dbnexus::{LatencyHistogram, MetricsCollector};
use std::hint::black_box;
use std::time::Duration;

// ============================================================================
// 基准测试
// ============================================================================

/// P50/P90/P99 计算：记录 100 条查询后读取百分位
///
/// 测量 `record_query` + `get_query_stats` + `latency_percentiles.p50/p90/p99` 的
/// 端到端开销。每次迭代记录 100 条不同延迟的查询，然后读取百分位。
fn bench_percentile_calculation(c: &mut Criterion) {
    c.bench_function("percentile_calculation", |b| {
        b.iter_with_setup(MetricsCollector::new, |collector| {
            for i in 1..=100 {
                let latency = Duration::from_millis(i as u64);
                collector.record_query("SELECT", latency, true, Some(100));
            }
            // 读取百分位
            if let Some(stats) = collector.get_query_stats("SELECT") {
                let _ = black_box(stats.latency_percentiles.p50());
                let _ = black_box(stats.latency_percentiles.p90());
                let _ = black_box(stats.latency_percentiles.p99());
            }
            collector
        })
    });
}

/// Prometheus 导出：预填充指标后导出为 Prometheus 格式字符串
///
/// 预填充 5 种查询类型 × 50 条记录 + 连接池状态，然后导出。
fn bench_prometheus_export(c: &mut Criterion) {
    let collector = MetricsCollector::new();
    // 预填充指标
    for query_type in &["SELECT", "INSERT", "UPDATE", "DELETE", "MERGE"] {
        for i in 1..=50 {
            let latency = Duration::from_millis(i);
            let success = i != 50;
            collector.record_query(query_type, latency, success, Some(100));
        }
    }
    collector.update_pool_status(20, 10, 10);

    c.bench_function("prometheus_export", |b| {
        b.iter(|| {
            let output = collector.export_prometheus();
            black_box(output);
        })
    });
}

/// 直方图记录：`LatencyHistogram::record` + `stats` 的吞吐量
///
/// 使用标准桶边界 [1, 5, 10, 50, 100, 500, 1000]ms，
/// 每次迭代记录 100 条不同延迟的样本，然后读取统计。
fn bench_histogram_record(c: &mut Criterion) {
    let bucket_boundaries = vec![1, 5, 10, 50, 100, 500, 1000];

    c.bench_function("histogram_record", |b| {
        b.iter_with_setup(
            || LatencyHistogram::new(bucket_boundaries.clone()),
            |histogram| {
                for i in 1..=100 {
                    let latency = Duration::from_millis(i);
                    histogram.record(latency);
                }
                let _ = black_box(histogram.stats());
                histogram
            },
        )
    });
}

/// Prometheus 导出规模曲线：1/50/200 标签（另附慢查询环满态）
///
/// `/metrics` 抓取路径（http-health 端点直通本导出）的成本随标签数线性
/// 增长，规模点作为回归基线暴露增长斜率。慢查询填充用单一 query_type
/// （"slow"，计入总标签数——op 填充 labels-1 个）——环满与标签数解耦：
/// export_prometheus 不遍历慢查询环（仅读查询统计），慢_{i} 各自成标签
/// 会伪造 x 轴。
fn bench_prometheus_export_scales(c: &mut Criterion) {
    let mut group = c.benchmark_group("prometheus_export_scale");
    for &labels in &[1usize, 50, 200] {
        let collector = MetricsCollector::new();
        collector.set_slow_query_threshold(0);
        for i in 0..labels.saturating_sub(1) {
            collector.record_query(&format!("op_{i}"), Duration::from_millis(1), true, None);
        }
        // 慢查询环满（上限 100）：环内容当前不入导出，仅固化"环满"前提，
        // 未来导出接入慢查询统计时本基准即覆盖该形态
        for _ in 0..105 {
            collector.record_query("slow", Duration::from_millis(1), true, None);
        }
        group.bench_function(format!("labels_{labels}"), |b| {
            b.iter(|| black_box(collector.export_prometheus()))
        });
    }
    group.finish();
}

/// 健康快照：`/readyz` 探测的数据源成本（池状态 + 慢查询计数 + 副本段）
///
/// 亚微秒级基准（本机采样 ≈0.4-0.8µs）：WSL2 上亚微秒基准跨进程漂移
/// 可达 2×，绝对差 <1µs 视为噪声而非回归——回归判定看同进程多轮中位数
/// 趋势或与 docs/PERFORMANCE.md 记录量级对比，不用窄百分比阈值。
#[cfg(all(
    feature = "sqlite",
    feature = "health-check",
    feature = "sql-parser",
    feature = "runtime-tokio-rustls"
))]
fn bench_health_snapshot(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = rt.block_on(async {
        // sqlite::memory: 快照基于池状态统计而非连接探活，预热一次保证 healthy
        let pool = dbnexus::DbPool::new("sqlite::memory:").await.expect("pool");
        let session = pool.get_session("admin").await.expect("session");
        session.execute_raw("SELECT 1").await.expect("warm");
        pool
    });
    c.bench_function("health_snapshot", |b| {
        b.iter(|| black_box(rt.block_on(pool.health_snapshot())))
    });
}

// health_snapshot 依赖 sqlite+health-check 组合，cfg 在宏外拆分声明
#[cfg(all(
    feature = "sqlite",
    feature = "health-check",
    feature = "sql-parser",
    feature = "runtime-tokio-rustls"
))]
criterion_group!(
    benches,
    bench_percentile_calculation,
    bench_prometheus_export,
    bench_prometheus_export_scales,
    bench_histogram_record,
    bench_health_snapshot
);
#[cfg(not(all(
    feature = "sqlite",
    feature = "health-check",
    feature = "sql-parser",
    feature = "runtime-tokio-rustls"
)))]
criterion_group!(
    benches,
    bench_percentile_calculation,
    bench_prometheus_export,
    bench_prometheus_export_scales,
    bench_histogram_record
);
criterion_main!(benches);
