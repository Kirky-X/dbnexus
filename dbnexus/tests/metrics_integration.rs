// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 指标收集集成测试
//!
//! 测试指标收集的完整功能，包括：
//! - 延迟指标收集
//! - 吞吐量统计
//! - 连接池指标
//! - 慢查询记录
//! - 错误计数

use dbnexus::metrics::{LatencyHistogram, MetricsCollector};
use std::time::Duration;
mod common;

/// TEST-METRICS-001: 创建指标收集器测试
#[tokio::test]
async fn test_metrics_collector_creation() {
    let collector = MetricsCollector::new();

    // 验证收集器创建成功
    assert!(collector.all_query_stats().is_empty());
}

/// TEST-METRICS-002: 记录查询指标测试
#[tokio::test]
async fn test_record_query_metrics() {
    let collector = MetricsCollector::new();

    // 记录成功的查询
    collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    collector.record_query("SELECT", Duration::from_millis(20), true, Some(200));
    collector.record_query("SELECT", Duration::from_millis(30), true, Some(300));

    // 记录失败的查询
    collector.record_query("SELECT", Duration::from_millis(5), false, None);

    // 获取查询统计
    let stats = collector.get_query_stats("SELECT");
    assert!(stats.is_some());

    let stats = stats.unwrap();
    assert_eq!(stats.count, 4);
    assert_eq!(stats.error_count, 1);
}

/// TEST-METRICS-003: 延迟直方图测试
#[tokio::test]
async fn test_latency_histogram() {
    let bucket_boundaries = vec![1, 5, 10, 50, 100, 500, 1000];
    let histogram = LatencyHistogram::new(bucket_boundaries);

    // 记录不同延迟
    histogram.record(Duration::from_millis(2));
    histogram.record(Duration::from_millis(7));
    histogram.record(Duration::from_millis(15));
    histogram.record(Duration::from_millis(75));
    histogram.record(Duration::from_millis(250));
    histogram.record(Duration::from_millis(750));
    histogram.record(Duration::from_millis(1500));

    // 获取直方图统计
    let stats = histogram.stats();
    assert!(stats.total_samples > 0);
    assert!(!stats.buckets.is_empty());

    // 验证桶分布
    let mut cumulative = 0u64;
    for bucket in &stats.buckets {
        cumulative += bucket.count;
        assert_eq!(bucket.cumulative_count, cumulative);
    }
}

/// TEST-METRICS-004: 吞吐量统计测试
#[tokio::test]
async fn test_throughput_stats() {
    let collector = MetricsCollector::new();

    // 记录成功的查询
    for _ in 0..100 {
        collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    }

    // 记录失败的查询
    for _ in 0..5 {
        collector.record_query("SELECT", Duration::from_millis(10), false, None);
    }

    // 获取查询统计
    let stats = collector.get_query_stats("SELECT");
    assert!(stats.is_some());

    let stats = stats.unwrap();
    assert_eq!(stats.count, 105);
    assert_eq!(stats.error_count, 5);

    // 验证错误率
    let expected_error_rate = 5.0 / 105.0;
    assert!((stats.error_rate() - expected_error_rate).abs() < 0.01);
}

/// TEST-METRICS-005: 连接池指标测试
#[tokio::test]
async fn test_connection_pool_metrics() {
    let collector = MetricsCollector::new();

    // 更新连接池状态
    collector.update_pool_status(10, 5, 5);

    // 获取连接池指标
    let pool_stats = collector.pool_status();
    assert_eq!(pool_stats.total, 10);
    assert_eq!(pool_stats.active, 5);
    assert_eq!(pool_stats.idle, 5);

    // 验证使用率
    let utilization = pool_stats.utilization_rate();
    assert_eq!(utilization, 0.5);
}

/// TEST-METRICS-006: 连接获取指标测试
#[tokio::test]
async fn test_connection_acquire_metrics() {
    let collector = MetricsCollector::new();

    // 记录连接获取成功
    collector.record_connection_acquire_success();
    collector.record_connection_acquire_success();
    collector.record_connection_acquire_success();

    // 记录连接获取超时
    collector.record_connection_acquire_timeout();

    // 记录连接获取失败
    collector.record_connection_acquire_failure();

    // 获取连接获取指标
    let stats = collector.connection_acquire_stats();
    assert_eq!(stats.total_attempts, 5);
    assert_eq!(stats.success_count, 3);
    assert_eq!(stats.timeout_count, 1);
    assert_eq!(stats.failure_count, 1);
}

/// TEST-METRICS-007: 事务指标测试
#[tokio::test]
async fn test_transaction_metrics() {
    let collector = MetricsCollector::new();

    // 记录事务提交
    collector.record_transaction_commit();
    collector.record_transaction_commit();

    // 记录事务回滚
    collector.record_transaction_rollback();

    // 获取事务统计
    let stats = collector.transaction_stats();

    // 验证事务记录
    assert_eq!(stats.total_transactions, 3);
    assert_eq!(stats.commit_count, 2);
    assert_eq!(stats.rollback_count, 1);
}

/// TEST-METRICS-008: 慢查询配置测试
#[tokio::test]
async fn test_slow_query_config() {
    let collector = MetricsCollector::new();

    // 配置慢查询阈值
    collector.set_slow_query_threshold(100);
    collector.set_slow_query_enabled(true);

    // 记录慢查询
    collector.record_query("SELECT", Duration::from_millis(150), true, Some(100));
    collector.record_query("SELECT", Duration::from_millis(200), true, Some(200));

    // 获取慢查询记录
    let slow_queries = collector.slow_queries();
    assert!(slow_queries.len() >= 2);
}

/// TEST-METRICS-009: 错误计数测试
#[tokio::test]
async fn test_error_counting() {
    let collector = MetricsCollector::new();

    // 记录连接错误
    collector.record_connection_error();
    collector.record_connection_error();

    // 验证错误已记录
    let error_count = collector.connection_errors.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(error_count, 2, "连接错误计数应该为 2");
}

/// TEST-METRICS-010: 多查询类型指标测试
#[tokio::test]
async fn test_multiple_query_types() {
    let collector = MetricsCollector::new();

    // 记录不同类型的查询
    for _ in 0..50 {
        collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
        collector.record_query("INSERT", Duration::from_millis(20), true, Some(200));
        collector.record_query("UPDATE", Duration::from_millis(15), true, Some(150));
        collector.record_query("DELETE", Duration::from_millis(25), true, Some(250));
    }

    // 获取各查询类型的指标
    let select_stats = collector.get_query_stats("SELECT");
    let insert_stats = collector.get_query_stats("INSERT");
    let update_stats = collector.get_query_stats("UPDATE");
    let delete_stats = collector.get_query_stats("DELETE");

    assert!(select_stats.is_some());
    assert!(insert_stats.is_some());
    assert!(update_stats.is_some());
    assert!(delete_stats.is_some());

    assert_eq!(select_stats.unwrap().count, 50);
    assert_eq!(insert_stats.unwrap().count, 50);
    assert_eq!(update_stats.unwrap().count, 50);
    assert_eq!(delete_stats.unwrap().count, 50);
}

/// TEST-METRICS-011: 总吞吐量统计测试
#[tokio::test]
async fn test_total_throughput() {
    let collector = MetricsCollector::new();

    // 记录各种查询
    for _ in 0..100 {
        collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
        collector.record_query("INSERT", Duration::from_millis(20), true, Some(200));
    }

    // 记录一些失败的查询
    for _ in 0..10 {
        collector.record_query("SELECT", Duration::from_millis(10), false, None);
        collector.record_query("INSERT", Duration::from_millis(20), false, None);
    }

    // 获取总吞吐量
    let total_stats = collector.total_throughput();
    assert_eq!(total_stats.total_operations, 220);
    assert_eq!(total_stats.success_count, 200);
    assert_eq!(total_stats.failure_count, 20);
}

/// TEST-METRICS-012: 所有查询统计测试
#[tokio::test]
async fn test_all_query_stats() {
    let collector = MetricsCollector::new();

    // 记录多种查询类型
    collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
    collector.record_query("INSERT", Duration::from_millis(20), true, Some(200));
    collector.record_query("UPDATE", Duration::from_millis(15), true, Some(150));

    // 获取所有查询统计
    let all_stats = collector.all_query_stats();
    assert_eq!(all_stats.len(), 3);
    assert!(all_stats.contains_key("SELECT"));
    assert!(all_stats.contains_key("INSERT"));
    assert!(all_stats.contains_key("UPDATE"));
}

/// TEST-METRICS-013: 慢查询禁用测试
#[tokio::test]
async fn test_slow_query_disabled() {
    let collector = MetricsCollector::new();

    // 禁用慢查询记录
    collector.set_slow_query_enabled(false);
    collector.set_slow_query_threshold(100);

    // 记录慢查询
    collector.record_query("SELECT", Duration::from_millis(150), true, Some(100));

    // 获取慢查询记录（应该为空）
    let slow_queries = collector.slow_queries();
    assert_eq!(slow_queries.len(), 0);
}

/// TEST-METRICS-014: 慢查询阈值调整测试
#[tokio::test]
async fn test_slow_query_threshold_adjustment() {
    let collector = MetricsCollector::new();

    // 设置较高的阈值
    collector.set_slow_query_threshold(1000);
    collector.set_slow_query_enabled(true);

    // 记录低于阈值的查询
    collector.record_query("SELECT", Duration::from_millis(500), true, Some(100));

    // 获取慢查询记录（应该为空）
    let slow_queries = collector.slow_queries();
    assert_eq!(slow_queries.len(), 0);

    // 降低阈值
    collector.set_slow_query_threshold(100);

    // 记录高于阈值的查询
    collector.record_query("SELECT", Duration::from_millis(150), true, Some(100));

    // 获取慢查询记录（应该不为空）
    let slow_queries = collector.slow_queries();
    assert!(!slow_queries.is_empty());
}

/// TEST-METRICS-015: 延迟分布测试
#[tokio::test]
async fn test_latency_distribution() {
    let collector = MetricsCollector::new();

    // 记录不同范围的延迟
    for i in 0..10 {
        collector.record_query("SELECT", Duration::from_millis(i * 10), true, Some(100));
    }

    // 获取查询统计
    let stats = collector.get_query_stats("SELECT");
    assert!(stats.is_some());

    let stats = stats.unwrap();
    // 验证有延迟数据
    assert!(stats.count > 0);
}
