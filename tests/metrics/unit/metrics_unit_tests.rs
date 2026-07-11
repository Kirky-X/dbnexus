// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Metrics 指标系统单元测试
//!
//! 覆盖以下测试场景：
//! - 指标聚合统计测试
//! - 指标标签过滤测试
//! - 指标时间窗口统计测试
//! - Prometheus 格式导出测试
//! - 指标基数控制测试
//! - 高基数标签检测测试

use dbnexus::{LatencyHistogram, LatencyPercentiles, MetricsCollector, MetricsCollectorTrait, PoolMetrics};
use std::time::Duration;

// ============================================================================
// 指标聚合统计测试
// ============================================================================

/// TEST-MU-001: 基础指标聚合统计测试
#[test]
fn test_metrics_aggregation_basic() {
    let collector = MetricsCollector::new();

    // 记录多条不同类型的查询
    for _ in 0..10 {
        collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    }
    for _ in 0..5 {
        collector.record_query("INSERT", Duration::from_millis(20), true, Some(200));
    }
    for _ in 0..3 {
        collector.record_query("UPDATE", Duration::from_millis(15), false, None);
    }

    // 验证聚合统计
    let select_stats = collector.get_query_stats("SELECT").unwrap();
    let insert_stats = collector.get_query_stats("INSERT").unwrap();
    let update_stats = collector.get_query_stats("UPDATE").unwrap();

    assert_eq!(select_stats.count, 10);
    assert_eq!(insert_stats.count, 5);
    assert_eq!(update_stats.count, 3);
    assert_eq!(update_stats.error_count, 3);
}

/// TEST-MU-002: 多指标类型聚合测试
#[test]
fn test_metrics_aggregation_multiple_types() {
    let collector = MetricsCollector::new();

    // 记录多种查询类型
    collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    collector.record_query("INSERT", Duration::from_millis(20), true, Some(200));
    collector.record_query("UPDATE", Duration::from_millis(15), true, Some(150));
    collector.record_query("DELETE", Duration::from_millis(25), true, Some(250));

    // 获取所有统计
    let all_stats = collector.all_query_stats();

    assert_eq!(all_stats.len(), 4);
    assert!(all_stats.contains_key("SELECT"));
    assert!(all_stats.contains_key("INSERT"));
    assert!(all_stats.contains_key("UPDATE"));
    assert!(all_stats.contains_key("DELETE"));
}

/// TEST-MU-003: 总吞吐量聚合测试
#[test]
fn test_total_throughput_aggregation() {
    let collector = MetricsCollector::new();

    // 记录多个类型的查询
    for _ in 0..10 {
        collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    }
    for _ in 0..5 {
        collector.record_query("INSERT", Duration::from_millis(20), false, None);
    }

    // 获取总吞吐量
    let total = collector.total_throughput();

    assert_eq!(total.total_operations, 15);
    assert_eq!(total.success_count, 10);
    assert_eq!(total.failure_count, 5);
    assert!((total.error_rate - 0.333).abs() < 0.01);
}

/// TEST-MU-004: 空收集器聚合测试
#[test]
fn test_empty_collector_aggregation() {
    let collector = MetricsCollector::new();

    let all_stats = collector.all_query_stats();
    assert!(all_stats.is_empty());

    let total = collector.total_throughput();
    assert_eq!(total.total_operations, 0);
    assert_eq!(total.success_count, 0);
}

// ============================================================================
// 指标标签过滤测试
// ============================================================================

/// TEST-MU-005: 按查询类型过滤测试
#[test]
fn test_query_type_filtering() {
    let collector = MetricsCollector::new();

    // 记录不同类型的查询
    for _ in 0..20 {
        collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    }
    for _ in 0..10 {
        collector.record_query("INSERT", Duration::from_millis(20), true, Some(200));
    }

    // 过滤获取 SELECT 统计
    let select_stats = collector.get_query_stats("SELECT").unwrap();
    assert_eq!(select_stats.count, 20);

    // 过滤获取 INSERT 统计
    let insert_stats = collector.get_query_stats("INSERT").unwrap();
    assert_eq!(insert_stats.count, 10);

    // 获取不存在的类型应返回 None
    let not_exist = collector.get_query_stats("NOTEXIST");
    assert!(not_exist.is_none());
}

/// TEST-MU-006: 成功/失败查询过滤测试
#[test]
fn test_success_failure_filtering() {
    let collector = MetricsCollector::new();

    // 记录成功和失败的查询
    for _ in 0..10 {
        collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    }
    for _ in 0..5 {
        collector.record_query("SELECT", Duration::from_millis(10), false, None);
    }

    let stats = collector.get_query_stats("SELECT").unwrap();
    assert_eq!(stats.count, 15);
    assert_eq!(stats.error_count, 5);

    // 验证错误率计算
    let expected_error_rate = 5.0 / 15.0;
    assert!((stats.error_rate() - expected_error_rate).abs() < 0.01);
}

/// TEST-MU-007: 连接池指标过滤测试
#[test]
fn test_pool_metrics_filtering() {
    let collector = MetricsCollector::new();

    // 设置不同的连接池状态
    collector.update_pool_status(20, 15, 5);

    let pool_metrics = collector.pool_status();

    assert_eq!(pool_metrics.total, 20);
    assert_eq!(pool_metrics.active, 15);
    assert_eq!(pool_metrics.idle, 5);
}

// ============================================================================
// 指标时间窗口统计测试
// ============================================================================

/// TEST-MU-008: 延迟百分位时间窗口测试
#[test]
fn test_latency_percentiles_time_window() {
    let collector = MetricsCollector::new();

    // 记录延迟样本 [1, 2, 3, ..., 100] 毫秒
    for i in 1..=100 {
        collector.record_query("SELECT", Duration::from_millis(i), true, Some(100));
    }

    let stats = collector.get_query_stats("SELECT").unwrap();
    let percentiles = stats.latency_percentiles;

    // 验证 P50 约在 50ms
    assert!(percentiles.p50_ns >= 49_000_000 && percentiles.p50_ns <= 51_000_000);
    // 验证 P90 约在 90ms
    assert!(percentiles.p90_ns >= 89_000_000 && percentiles.p90_ns <= 91_000_000);
    // 验证 P99 约在 99ms
    assert!(percentiles.p99_ns >= 98_000_000 && percentiles.p99_ns <= 100_000_000);
}

/// TEST-MU-009: 延迟百分位边界值测试
#[test]
fn test_latency_percentiles_boundary_values() {
    let collector = MetricsCollector::new();

    // 记录固定延迟
    for _ in 0..100 {
        collector.record_query("SELECT", Duration::from_millis(100), true, None);
    }

    let stats = collector.get_query_stats("SELECT").unwrap();
    let percentiles = stats.latency_percentiles;

    // 所有百分位应该都等于 100ms
    assert_eq!(percentiles.p50_ns, 100_000_000);
    assert_eq!(percentiles.p90_ns, 100_000_000);
    assert_eq!(percentiles.p99_ns, 100_000_000);
    assert_eq!(percentiles.min_ns, 100_000_000);
    assert_eq!(percentiles.max_ns, 100_000_000);
}

/// TEST-MU-010: 延迟百分位空数据测试
#[test]
fn test_latency_percentiles_empty() {
    let collector = MetricsCollector::new();

    let stats = collector.get_query_stats("SELECT");

    // 未记录的查询类型应返回默认零值
    assert!(stats.is_none());
}

/// TEST-MU-011: 直方图时间窗口测试
#[test]
fn test_histogram_time_window() {
    let histogram = LatencyHistogram::new(vec![1, 5, 10, 50, 100, 500]);

    // 记录不同范围的延迟
    histogram.record(Duration::from_millis(2)); // 桶 1 (<=5)
    histogram.record(Duration::from_millis(7)); // 桶 2 (<=10)
    histogram.record(Duration::from_millis(15)); // 桶 3 (<=50)
    histogram.record(Duration::from_millis(75)); // 桶 4 (<=100)
    histogram.record(Duration::from_millis(600)); // 桶 5 (<=500)
    histogram.record(Duration::from_millis(1500)); // 桶 6 (>500)

    let stats = histogram.stats();

    assert_eq!(stats.total_samples, 6);
    assert_eq!(stats.buckets.len(), 7); // 6个定义桶 + 1个溢出桶
}

/// TEST-MU-012: 直方图累积计数测试
#[test]
fn test_histogram_cumulative_count() {
    let histogram = LatencyHistogram::new(vec![10, 50, 100]);

    histogram.record(Duration::from_millis(5));
    histogram.record(Duration::from_millis(20));
    histogram.record(Duration::from_millis(60));
    histogram.record(Duration::from_millis(120));

    let stats = histogram.stats();

    // 验证累积计数（直方图累积计数必须严格单调非递减）
    for (i, bucket) in stats.buckets.iter().enumerate() {
        if i < stats.buckets.len() - 1 {
            assert!(
                bucket.cumulative_count <= stats.buckets[i + 1].cumulative_count,
                "bucket[{}] cumulative_count {} > bucket[{}] cumulative_count {}",
                i,
                bucket.cumulative_count,
                i + 1,
                stats.buckets[i + 1].cumulative_count
            );
        }
    }
}

// ============================================================================
// Prometheus 格式导出测试
// ============================================================================

/// TEST-MU-013: Prometheus 格式基础导出测试
#[test]
fn test_prometheus_export_basic() {
    let collector = MetricsCollector::new();

    collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    collector.update_pool_status(10, 5, 5);

    let prometheus = collector.export_prometheus();

    // 验证必需指标存在
    assert!(prometheus.contains("dbnexus_uptime_seconds"));
    assert!(prometheus.contains("dbnexus_pool_connections_total"));
    assert!(prometheus.contains("dbnexus_pool_connections_active"));
    assert!(prometheus.contains("dbnexus_pool_connections_idle"));
    assert!(prometheus.contains("dbnexus_queries_total"));
}

/// TEST-MU-014: Prometheus 导出包含百分位指标
#[test]
fn test_prometheus_export_percentiles() {
    let collector = MetricsCollector::new();

    // 记录足够的样本以生成百分位
    for i in 1..=100 {
        collector.record_query("SELECT", Duration::from_millis(i), true, Some(100));
    }

    let prometheus = collector.export_prometheus();

    // 验证百分位指标
    assert!(prometheus.contains("dbnexus_query_latency_p50_seconds"));
    assert!(prometheus.contains("dbnexus_query_latency_p90_seconds"));
    assert!(prometheus.contains("dbnexus_query_latency_p95_seconds"));
    assert!(prometheus.contains("dbnexus_query_latency_p99_seconds"));
}

/// TEST-MU-015: Prometheus 导出包含吞吐量指标
#[test]
fn test_prometheus_export_throughput() {
    let collector = MetricsCollector::new();

    for _ in 0..10 {
        collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    }
    for _ in 0..5 {
        collector.record_query("INSERT", Duration::from_millis(20), false, None);
    }

    let prometheus = collector.export_prometheus();

    assert!(prometheus.contains("dbnexus_total_qps"));
    assert!(prometheus.contains("dbnexus_total_operations"));
    assert!(prometheus.contains("dbnexus_error_rate"));
}

/// TEST-MU-016: Prometheus 导出包含事务指标
#[test]
fn test_prometheus_export_transaction_metrics() {
    let collector = MetricsCollector::new();

    collector.record_transaction_commit();
    collector.record_transaction_commit();
    collector.record_transaction_rollback();

    let prometheus = collector.export_prometheus();

    assert!(prometheus.contains("dbnexus_transactions_total"));
    assert!(prometheus.contains("dbnexus_transactions_commit_total"));
    assert!(prometheus.contains("dbnexus_transactions_rollback_total"));
}

/// TEST-MU-017: Prometheus 导出包含连接获取指标
#[test]
fn test_prometheus_export_connection_acquire_metrics() {
    let collector = MetricsCollector::new();

    collector.record_connection_acquire_success();
    collector.record_connection_acquire_success();
    collector.record_connection_acquire_timeout();
    collector.record_connection_acquire_failure();

    let prometheus = collector.export_prometheus();

    assert!(prometheus.contains("dbnexus_connection_acquire_total"));
    assert!(prometheus.contains("dbnexus_connection_acquire_timeout_total"));
    assert!(prometheus.contains("dbnexus_connection_acquire_failure_total"));
}

/// TEST-MU-018: Prometheus 空导出测试
#[test]
fn test_prometheus_export_empty() {
    let collector = MetricsCollector::new();

    let prometheus = collector.export_prometheus();

    // 空收集器也应导出有效格式
    assert!(prometheus.contains("dbnexus_uptime_seconds"));
    assert!(prometheus.contains("dbnexus_pool_connections_total"));
}

// ============================================================================
// 指标基数控制测试
// ============================================================================

/// TEST-MU-019: 延迟存储滑动窗口测试（基数控制）
#[test]
fn test_latency_storage_sliding_window() {
    let collector = MetricsCollector::new();

    // 记录超过最大样本数的延迟
    let max_samples = 10000;
    for i in 0..(max_samples + 100) {
        collector.record_query("SELECT", Duration::from_millis(i as u64), true, None);
    }

    let stats = collector.get_query_stats("SELECT").unwrap();

    // 滑动窗口应该限制样本数
    assert!(stats.latency_percentiles.sample_count <= max_samples as u64);
}

/// TEST-MU-020: 直方图桶计数边界测试
#[test]
fn test_histogram_bucket_count_boundary() {
    let histogram = LatencyHistogram::new(vec![10, 50, 100]);

    // 记录边界值
    histogram.record(Duration::from_millis(10)); // 边界
    histogram.record(Duration::from_millis(11)); // 超过边界
    histogram.record(Duration::from_millis(50)); // 边界
    histogram.record(Duration::from_millis(51)); // 超过边界

    let stats = histogram.stats();

    // 验证总样本数
    assert_eq!(stats.total_samples, 4);
}

/// TEST-MU-021: 连接池利用率计算测试
#[test]
fn test_pool_utilization_calculation() {
    let metrics = PoolMetrics {
        total: 10,
        active: 5,
        idle: 5,
    };

    assert_eq!(metrics.utilization_rate(), 0.5);

    let metrics_full = PoolMetrics {
        total: 10,
        active: 10,
        idle: 0,
    };

    assert_eq!(metrics_full.utilization_rate(), 1.0);

    let metrics_empty = PoolMetrics {
        total: 0,
        active: 0,
        idle: 0,
    };

    assert_eq!(metrics_empty.utilization_rate(), 0.0);
}

/// TEST-MU-022: 错误率边界测试
#[test]
fn test_error_rate_boundaries() {
    let collector = MetricsCollector::new();

    // 无错误
    collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));

    let stats = collector.get_query_stats("SELECT").unwrap();
    assert_eq!(stats.error_rate(), 0.0);

    // 全部失败
    let collector2 = MetricsCollector::new();
    collector2.record_query("SELECT", Duration::from_millis(10), false, None);

    let stats2 = collector2.get_query_stats("SELECT").unwrap();
    assert_eq!(stats2.error_rate(), 1.0);
}

/// TEST-MU-023: 连接获取超时率计算测试
#[test]
fn test_connection_acquire_timeout_rate() {
    let collector = MetricsCollector::new();

    for _ in 0..8 {
        collector.record_connection_acquire_success();
    }
    for _ in 0..2 {
        collector.record_connection_acquire_timeout();
    }

    let stats = collector.connection_acquire_stats();

    assert_eq!(stats.total_attempts, 10);
    assert_eq!(stats.timeout_rate, 0.2);
}

/// TEST-MU-024: 事务成功率计算测试
#[test]
fn test_transaction_success_rate() {
    let collector = MetricsCollector::new();

    for _ in 0..7 {
        collector.record_transaction_commit();
    }
    for _ in 0..3 {
        collector.record_transaction_rollback();
    }

    let stats = collector.transaction_stats();

    assert_eq!(stats.total_transactions, 10);
    assert_eq!(stats.success_rate, 70.0);
}

// ============================================================================
// 高基数标签检测测试
// ============================================================================

/// TEST-MU-025: 高基数标签检测 - 大量唯一查询类型
#[test]
fn test_high_cardinality_detection_many_types() {
    let collector = MetricsCollector::new();

    // 创建大量不同的查询类型（模拟高基数标签）
    for i in 0..1000 {
        collector.record_query(&format!("SELECT_{}", i), Duration::from_millis(10), true, Some(100));
    }

    let all_stats = collector.all_query_stats();

    // 应该能够处理大量唯一标签
    assert_eq!(all_stats.len(), 1000);
}

/// TEST-MU-026: 高基数场景下内存控制测试
#[test]
fn test_memory_control_high_cardinality() {
    let collector = MetricsCollector::new();

    // 记录大量不同的查询类型
    for i in 0..500 {
        collector.record_query(&format!("type_{}", i), Duration::from_millis(i % 100), true, None);
    }

    // 验证仍然可以获取统计
    let stats = collector.get_query_stats("type_0");
    assert!(stats.is_some());
    assert_eq!(stats.unwrap().count, 1);
}

/// TEST-MU-027: 高基数下百分位准确性测试
#[test]
fn test_percentiles_accuracy_high_cardinality() {
    let collector = MetricsCollector::new();

    // 记录多种类型，每种有足够样本
    for query_type in &["A", "B", "C", "D", "E"] {
        for i in 1..=1000 {
            collector.record_query(query_type, Duration::from_millis(i), true, None);
        }
    }

    // 验证每种类型的百分位都正确
    for query_type in &["A", "B", "C", "D", "E"] {
        let stats = collector.get_query_stats(query_type).unwrap();
        assert_eq!(stats.count, 1000);

        // P50 应该约等于 500ms
        let p50_ms = stats.latency_percentiles.p50_ns / 1_000_000;
        assert!((490..=510).contains(&p50_ms));
    }
}

/// TEST-MU-028: 高基数下直方图准确性测试
#[test]
fn test_histogram_accuracy_high_cardinality() {
    let collector = MetricsCollector::new();

    // 记录多种类型
    for _ in 0..10 {
        collector.record_query("SELECT", Duration::from_millis(5), true, None);
        collector.record_query("SELECT", Duration::from_millis(15), true, None);
        collector.record_query("SELECT", Duration::from_millis(55), true, None);
        collector.record_query("INSERT", Duration::from_millis(5), true, None);
    }

    let stats = collector.get_query_stats("SELECT").unwrap();
    assert_eq!(stats.histogram.total_samples, 30);
}

/// TEST-MU-029: 高基数下 Prometheus 导出测试
#[test]
fn test_prometheus_export_high_cardinality() {
    let collector = MetricsCollector::new();

    // 记录多种查询类型
    for i in 0..50 {
        collector.record_query(&format!("query_type_{}", i), Duration::from_millis(10), true, Some(100));
    }

    let prometheus = collector.export_prometheus();

    // 验证导出的指标数量与查询类型数匹配
    let query_count = prometheus.matches("dbnexus_queries_total").count();
    assert!(query_count >= 50);
}

/// TEST-MU-030: 高基数标签下时间窗口测试
#[test]
fn test_time_window_high_cardinality() {
    let collector = MetricsCollector::new();

    // 记录多种类型，每种记录多次
    for _ in 0..10 {
        for i in 0..100 {
            collector.record_query(&format!("type_{}", i), Duration::from_millis(i % 50), true, None);
        }
    }

    // 验证所有类型都能正确聚合
    let all_stats = collector.all_query_stats();
    assert_eq!(all_stats.len(), 100);

    for i in 0..100 {
        let stats = all_stats.get(&format!("type_{}", i)).unwrap();
        assert_eq!(stats.count, 10);
    }
}

// ============================================================================
// Trait 实现测试
// ============================================================================

/// TEST-MU-031: MetricsCollectorTrait 默认实现测试
#[test]
fn test_metrics_collector_trait_default() {
    let collector = MetricsCollector::new();

    // 使用 trait 方法记录查询
    MetricsCollectorTrait::record_query(&collector, Duration::from_millis(10));

    let stats = collector.query_stats();
    assert_eq!(stats.count, 1);
}

/// TEST-MU-032: MetricsCollectorTrait 连接记录测试
#[test]
fn test_metrics_collector_trait_connection() {
    let collector = MetricsCollector::new();

    // 快速连接（<100ms）- 成功
    MetricsCollectorTrait::record_connection(&collector, Duration::from_millis(50));

    // 中等连接（100-1000ms）- 超时
    MetricsCollectorTrait::record_connection(&collector, Duration::from_millis(500));

    // 慢连接（>1000ms）- 失败
    MetricsCollectorTrait::record_connection(&collector, Duration::from_millis(1500));

    let stats = collector.connection_stats();
    assert_eq!(stats.success_count, 1);
    assert_eq!(stats.timeout_count, 1);
    assert_eq!(stats.failure_count, 1);
}

/// TEST-MU-033: MetricsCollectorTrait 事务记录测试
#[test]
fn test_metrics_collector_trait_transaction() {
    let collector = MetricsCollector::new();

    MetricsCollectorTrait::record_transaction(&collector, Duration::from_millis(100), true);
    MetricsCollectorTrait::record_transaction(&collector, Duration::from_millis(50), false);

    let stats = collector.transaction_stats();
    assert_eq!(stats.commit_count, 1);
    assert_eq!(stats.failure_count, 1);
}

/// TEST-MU-034: MetricsCollectorTrait 连接池使用记录测试
#[test]
fn test_metrics_collector_trait_pool_usage() {
    let collector = MetricsCollector::new();

    MetricsCollectorTrait::record_pool_usage(&collector, 10, 7, 3);

    let metrics = collector.pool_metrics();
    assert_eq!(metrics.total, 10);
    assert_eq!(metrics.active, 7);
    assert_eq!(metrics.idle, 3);
    assert_eq!(metrics.utilization_rate(), 0.7);
}

/// TEST-MU-035: MetricsCollectorTrait Prometheus 导出测试
#[test]
fn test_metrics_collector_trait_prometheus_export() {
    let collector = MetricsCollector::new();

    collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));

    let prometheus = MetricsCollectorTrait::export_prometheus(&collector);

    assert!(prometheus.contains("dbnexus_uptime_seconds"));
    assert!(prometheus.contains("dbnexus_queries_total"));
}

/// TEST-MU-036: MetricsCollectorTrait 清空测试
#[test]
fn test_metrics_collector_trait_clear() {
    let collector = MetricsCollector::new();

    collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    collector.update_pool_status(10, 5, 5);

    // 验证有数据
    assert!(!collector.all_query_stats().is_empty());

    // 清空
    MetricsCollectorTrait::clear(&collector);

    // 验证已清空
    assert!(collector.all_query_stats().is_empty());
}

// ============================================================================
// 边界条件和错误处理测试
// ============================================================================

/// TEST-MU-037: 零延迟记录测试
#[test]
fn test_zero_latency_recording() {
    let collector = MetricsCollector::new();

    collector.record_query("SELECT", Duration::from_nanos(0), true, Some(0));

    let stats = collector.get_query_stats("SELECT").unwrap();
    assert_eq!(stats.count, 1);
    assert_eq!(stats.latency_percentiles.min_ns, 0);
}

/// TEST-MU-038: 极大延迟记录测试
#[test]
fn test_max_latency_recording() {
    let collector = MetricsCollector::new();

    // 记录极大延迟（1小时）
    collector.record_query("SELECT", Duration::from_secs(3600), true, None);

    let stats = collector.get_query_stats("SELECT").unwrap();
    assert_eq!(stats.latency_percentiles.max_ns, 3600 * 1_000_000_000);
}

/// TEST-MU-039: 延迟百分位 Duration 转换测试
#[test]
fn test_latency_percentiles_duration_conversion() {
    let percentiles = LatencyPercentiles {
        p50_ns: 10_000_000,   // 10ms
        p75_ns: 25_000_000,   // 25ms
        p90_ns: 50_000_000,   // 50ms
        p95_ns: 75_000_000,   // 75ms
        p99_ns: 100_000_000,  // 100ms
        p999_ns: 150_000_000, // 150ms
        min_ns: 1_000_000,    // 1ms
        max_ns: 200_000_000,  // 200ms
        sample_count: 1000,
    };

    assert_eq!(percentiles.p50(), Duration::from_millis(10));
    assert_eq!(percentiles.p99(), Duration::from_millis(100));
    assert_eq!(percentiles.min(), Duration::from_millis(1));
    assert_eq!(percentiles.max(), Duration::from_millis(200));
}

/// TEST-MU-040: 直方图空数据统计测试
#[test]
fn test_histogram_empty_stats() {
    let histogram = LatencyHistogram::new(vec![10, 50, 100]);

    let stats = histogram.stats();

    assert_eq!(stats.total_samples, 0);
    assert!(stats.buckets.iter().all(|b| b.count == 0));
}

/// TEST-MU-041: 重置清空所有指标测试
#[test]
fn test_reset_clears_all_metrics() {
    let collector = MetricsCollector::new();

    // 记录初始数据
    collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    assert_eq!(collector.get_query_stats("SELECT").unwrap().count, 1);

    // 重置
    collector.reset();

    // 验证已清空
    assert!(collector.get_query_stats("SELECT").is_none());
}

/// TEST-MU-041: 重置后重新记录测试
#[test]
fn test_re_record_after_reset_succeeds() {
    let collector = MetricsCollector::new();

    // 记录初始数据
    collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));

    // 重置
    collector.reset();

    // 重新记录
    collector.record_query("SELECT", Duration::from_millis(20), true, Some(200));

    // 验证新数据
    let stats = collector.get_query_stats("SELECT").unwrap();
    assert_eq!(stats.count, 1);
    assert_eq!(stats.latency_percentiles.p50_ns, 20_000_000);
}

/// TEST-MU-042: 并发场景下数据一致性测试
#[test]
fn test_concurrent_data_consistency() {
    use std::sync::Arc;
    use std::thread;

    let collector = Arc::new(MetricsCollector::new());
    let mut handles = vec![];

    // 多个线程同时记录
    for _ in 0..10 {
        let collector_clone = Arc::clone(&collector);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                collector_clone.record_query("SELECT", Duration::from_millis(i % 50), true, Some(100));
            }
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    // 验证总数正确
    let stats = collector.get_query_stats("SELECT").unwrap();
    assert_eq!(stats.count, 1000);
}

/// TEST-MU-043: 慢查询记录边界测试
#[test]
fn test_slow_query_boundary() {
    let collector = MetricsCollector::new();

    // 设置阈值为 100ms
    collector.set_slow_query_threshold(100);

    // 记录 99ms（不触发）
    collector.record_query("SELECT", Duration::from_millis(99), true, None);
    assert!(collector.slow_queries().is_empty());

    // 记录 100ms（触发）
    collector.record_query("SELECT", Duration::from_millis(100), true, None);
    assert_eq!(collector.slow_queries().len(), 1);

    // 记录 101ms（触发）
    collector.record_query("SELECT", Duration::from_millis(101), true, None);
    assert_eq!(collector.slow_queries().len(), 2);
}

/// TEST-MU-044: 慢查询记录限制测试
#[test]
fn test_slow_query_limit() {
    let collector = MetricsCollector::new();

    collector.set_slow_query_threshold(10);

    // 记录超过限制的慢查询
    for i in 0..150 {
        collector.record_query("SELECT", Duration::from_millis(100 + i), true, None);
    }

    // 应该限制为 100 条（默认最大值）
    let slow_queries = collector.slow_queries();
    assert!(slow_queries.len() <= 100);
}

/// TEST-MU-045: 错误计数累加测试
#[test]
fn test_error_count_accumulation() {
    let collector = MetricsCollector::new();

    // 记录多个错误
    for _ in 0..5 {
        collector.record_connection_error();
    }

    assert_eq!(collector.connection_error_count(), 5);

    // 记录更多错误
    for _ in 0..3 {
        collector.record_query("SELECT", Duration::from_millis(10), false, None);
    }

    // 验证累积
    let stats = collector.get_query_stats("SELECT").unwrap();
    assert_eq!(stats.error_count, 3);
}
