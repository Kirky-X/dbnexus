// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 指标系统性能基准测试（T093）
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

criterion_group!(
    benches,
    bench_percentile_calculation,
    bench_prometheus_export,
    bench_histogram_record
);
criterion_main!(benches);
