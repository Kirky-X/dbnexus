// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Prometheus 指标导出示例
//!
//! 演示 `MetricsCollector` 的完整使用流程：
//! - 创建连接池并将连接池状态同步到指标收集器
//! - 记录查询延迟、事务、连接获取等指标
//! - 展示 `PoolMetrics`（active/idle/total connections）和 `QueryStats`
//! - 导出 Prometheus 格式指标字符串
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example metrics_prometheus --features "sqlite,metrics"
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::{MetricsCollector, MetricsCollectorTrait, PoolMetrics, QueryStats};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("📊 DBNexus Prometheus 指标导出示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建连接池（用于真实场景下的指标采集上下文）
    // ============================================
    let (_pool, _session) = common::db::setup_shared_sqlite_session().await?;
    println!("✓ 连接池创建成功 (max_connections=5)\n");

    // ============================================
    // 2. 创建 MetricsCollector 并同步连接池状态
    // ============================================
    let collector = MetricsCollector::new();

    // 将连接池状态同步到指标收集器（total/active/idle）
    collector.update_pool_status(5, 2, 3);
    println!("--- 连接池指标 (PoolMetrics) ---");
    let pool_metrics: PoolMetrics = collector.pool_status();
    println!(
        "  total={}, active={}, idle={}",
        pool_metrics.total, pool_metrics.active, pool_metrics.idle
    );
    println!("  utilization_rate={:.2}%", pool_metrics.utilization_rate() * 100.0);

    // ============================================
    // 3. 记录查询指标（模拟业务流量）
    // ============================================
    println!("\n--- 记录查询指标 ---");
    for i in 1..=20 {
        let latency_ms = if i % 5 == 0 { 120 } else { 5 + (i % 10) };
        let success = i != 15; // 第 15 次模拟失败
        let bytes = if success { Some(100 * i) } else { None };
        collector.record_query("SELECT", Duration::from_millis(latency_ms), success, bytes);
    }
    // 记录少量 INSERT/UPDATE 用于多类型指标展示
    collector.record_query("INSERT", Duration::from_millis(25), true, None);
    collector.record_query("UPDATE", Duration::from_millis(40), true, None);

    println!("  ✓ 记录 20 条 SELECT + 1 条 INSERT + 1 条 UPDATE");

    // ============================================
    // 4. 展示 QueryStats（查询统计）
    // ============================================
    println!("\n--- 查询统计 (QueryStats) ---");
    if let Some(stats) = collector.get_query_stats("SELECT") {
        print_query_stats("SELECT", &stats);
    }
    if let Some(stats) = collector.get_query_stats("INSERT") {
        print_query_stats("INSERT", &stats);
    }

    // ============================================
    // 5. 记录事务与连接获取指标
    // ============================================
    println!("\n--- 事务与连接获取指标 ---");
    collector.record_transaction_commit();
    collector.record_transaction_commit();
    collector.record_transaction_rollback();
    collector.record_connection_acquire_success();
    collector.record_connection_acquire_success();
    collector.record_connection_acquire_timeout();

    let txn_stats = collector.transaction_stats();
    println!(
        "  事务: total={}, commit={}, rollback={}, success_rate={:.2}%",
        txn_stats.total_transactions, txn_stats.commit_count, txn_stats.rollback_count, txn_stats.success_rate
    );

    let conn_stats = collector.connection_acquire_stats();
    println!(
        "  连接获取: total={}, success={}, timeout={}",
        conn_stats.total_attempts, conn_stats.success_count, conn_stats.timeout_count
    );

    // ============================================
    // 6. 慢查询记录
    // ============================================
    println!("\n--- 慢查询记录 ---");
    collector.set_slow_query_threshold(100);
    let slow_queries = collector.slow_queries();
    println!("  慢查询数量 (threshold=100ms): {}", slow_queries.len());
    for sq in &slow_queries {
        println!("  - type={}, duration_ms={}", sq.query_type, sq.duration_ms);
    }

    // ============================================
    // 7. 导出 Prometheus 格式指标
    // ============================================
    println!("\n--- Prometheus 格式导出 ---");
    let prometheus_output = collector.export_prometheus();
    // 只打印前 30 行，避免刷屏
    for (idx, line) in prometheus_output.lines().enumerate() {
        if idx >= 30 {
            println!("  ... (共 {} 行，已省略后续输出)", prometheus_output.lines().count());
            break;
        }
        println!("  {}", line);
    }

    // 通过 trait 对象验证 MetricsCollectorTrait 实现
    let trait_collector: std::sync::Arc<dyn MetricsCollectorTrait> = std::sync::Arc::new(MetricsCollector::new());
    let trait_metrics = trait_collector.pool_metrics();
    println!(
        "\n✓ 通过 MetricsCollectorTrait 获取 PoolMetrics: total={}",
        trait_metrics.total
    );

    println!("\n========================================");
    println!("✨ Prometheus 指标导出示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - MetricsCollector::new()                      - 创建指标收集器");
    println!("  - collector.update_pool_status(t, a, i)        - 同步连接池状态");
    println!("  - collector.record_query(type, dur, ok, bytes) - 记录查询指标");
    println!("  - collector.pool_status() -> PoolMetrics       - 获取连接池指标");
    println!("  - collector.get_query_stats(type) -> QueryStats - 获取查询统计");
    println!("  - collector.export_prometheus() -> String      - 导出 Prometheus 格式");
    println!("  - PoolMetrics::utilization_rate()              - 连接使用率");
    println!("  - MetricsCollectorTrait                        - 通用指标收集 trait");

    Ok(())
}

/// 打印 QueryStats 详情
fn print_query_stats(query_type: &str, stats: &QueryStats) {
    let p50 = stats.latency_percentiles.p50();
    let p90 = stats.latency_percentiles.p90();
    let p99 = stats.latency_percentiles.p99();
    println!("  [{}] count={}, errors={}", query_type, stats.count, stats.error_count);
    println!("      latency: p50={:?}, p90={:?}, p99={:?}", p50, p90, p99);
    println!(
        "      throughput: qps={:.2}, error_rate={:.4}",
        stats.throughput.avg_qps,
        stats.error_rate()
    );
    println!("      histogram samples={}", stats.histogram.total_samples);
}
