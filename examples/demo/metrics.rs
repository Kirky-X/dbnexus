// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Prometheus 指标监控示例
//!
//! 展示如何使用 dbnexus 的指标监控功能：
//! - 配置 Prometheus 指标收集器
//! - 收集数据库操作指标
//! - 导出 Prometheus 格式指标
//! - 查询和监控指标数据
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example metrics --features "sqlite,metrics"
//! ```

use dbnexus::{DbConfig, DbPool, metrics::MetricsCollector};
use std::path::Path;
use std::time::Duration;

/// 定义 User 结构体（用于演示指标收集）
#[derive(Debug, Clone, PartialEq)]
struct User {
    id: i64,
    name: String,
    email: String,
}

/// 定义 Product 结构体（用于演示不同查询类型的指标）
#[derive(Debug, Clone, PartialEq)]
struct Product {
    id: i64,
    name: String,
    price: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 DBNexus Prometheus 指标监控示例\n");
    println!("========================================");

    // 1. 创建指标收集器
    println!("\n1️⃣ 创建指标收集器");
    println!("------------------------------------------");
    let metrics = MetricsCollector::new();
    println!("✓ 指标收集器创建成功");

    // 2. 初始化数据库连接池
    println!("\n2️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let current_dir = std::env::current_dir()?;
    let permissions_path = current_dir.join("src/permissions.yaml");
    println!("  权限配置路径: {}", permissions_path.display());

    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .permissions_path(permissions_path.to_string_lossy().to_string())
        .admin_role("admin")
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // 3. 创建测试数据
    println!("\n3️⃣ 创建测试数据");
    println!("------------------------------------------");
    let session = pool.get_session("admin").await?;

    // 创建表
    session
        .execute_raw_ddl(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL
            )",
        )
        .await?;

    session
        .execute_raw_ddl(
            "CREATE TABLE products (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                price REAL NOT NULL
            )",
        )
        .await?;

    // 插入用户数据
    for i in 1..=100 {
        session
            .execute_raw(&format!(
                "INSERT INTO users (id, name, email) VALUES ({}, 'User {}', 'user{}@example.com')",
                i, i, i
            ))
            .await?;
    }
    println!("  ✓ 插入 100 个用户");

    // 插入产品数据
    println!("  插入产品数据...");
    let product_session = pool.get_session("admin").await?;
    for i in 1..=50 {
        product_session
            .execute_raw(&format!(
                "INSERT INTO products (id, name, price) VALUES ({}, 'Product {}', {})",
                i,
                i,
                i as f64 * 10.0
            ))
            .await?;
    }
    println!("  ✓ 插入 50 个产品");

    // 4. 模拟数据库操作并收集指标
    println!("\n4️⃣ 模拟数据库操作并收集指标");
    println!("------------------------------------------");

    // 记录 SELECT 查询（使用新 session）
    let select_session = pool.get_session("admin").await?;
    for _ in 0..10 {
        let start = std::time::Instant::now();
        let _result = select_session.execute_raw("SELECT * FROM users").await?;
        let duration = start.elapsed();
        metrics.record_query("SELECT", duration, true, Some(1024));
    }
    println!("  ✓ 记录 10 次 SELECT 查询");

    // 记录 INSERT 操作
    let insert_session = pool.get_session("admin").await?;
    for i in 101..=105 {
        let start = std::time::Instant::now();
        insert_session
            .execute_raw(&format!(
                "INSERT INTO users (id, name, email) VALUES ({}, 'User {}', 'user{}@example.com')",
                i, i, i
            ))
            .await?;
        let duration = start.elapsed();
        metrics.record_query("INSERT", duration, true, Some(512));
    }
    println!("  ✓ 记录 5 次 INSERT 操作");

    // 记录 UPDATE 操作
    let update_session = pool.get_session("admin").await?;
    for i in 1..=5 {
        let start = std::time::Instant::now();
        update_session
            .execute_raw(&format!(
                "UPDATE users SET email = 'updated{}@example.com' WHERE id = {}",
                i, i
            ))
            .await?;
        let duration = start.elapsed();
        metrics.record_query("UPDATE", duration, true, Some(256));
    }
    println!("  ✓ 记录 5 次 UPDATE 操作");

    // 记录 DELETE 操作
    let delete_session = pool.get_session("admin").await?;
    for i in 101..=105 {
        let start = std::time::Instant::now();
        delete_session
            .execute_raw(&format!("DELETE FROM users WHERE id = {}", i))
            .await?;
        let duration = start.elapsed();
        metrics.record_query("DELETE", duration, true, None);
    }
    println!("  ✓ 记录 5 次 DELETE 操作");

    // 模拟一些慢查询
    println!("\n  模拟慢查询...");
    for i in 0..3 {
        let start = std::time::Instant::now();
        tokio::time::sleep(Duration::from_millis(1100 + i * 100)).await;
        let duration = start.elapsed();
        metrics.record_query("SELECT", duration, true, Some(2048));
    }
    println!("  ✓ 记录 3 次慢查询");

    // 模拟一些失败的查询
    println!("\n  模拟失败的查询...");
    for _ in 0..2 {
        let start = std::time::Instant::now();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let duration = start.elapsed();
        metrics.record_query("SELECT", duration, false, None);
    }
    println!("  ✓ 记录 2 次失败的查询");

    // 5. 记录连接池指标
    println!("\n5️⃣ 记录连接池指标");
    println!("------------------------------------------");
    let status = pool.status();
    metrics.update_pool_status(status.total, status.active, status.idle);
    println!(
        "  ✓ 连接池状态: total={}, active={}, idle={}",
        status.total, status.active, status.idle
    );

    // 记录连接获取成功
    for _ in 0..20 {
        metrics.record_connection_acquire_success();
    }
    println!("  ✓ 记录 20 次连接获取成功");

    // 记录连接获取超时
    for _ in 0..2 {
        metrics.record_connection_acquire_timeout();
    }
    println!("  ✓ 记录 2 次连接获取超时");

    // 6. 记录事务指标
    println!("\n6️⃣ 记录事务指标");
    println!("------------------------------------------");

    // 记录事务提交
    for _ in 0..15 {
        metrics.record_transaction_commit();
    }
    println!("  ✓ 记录 15 次事务提交");

    // 记录事务回滚
    for _ in 0..3 {
        metrics.record_transaction_rollback();
    }
    println!("  ✓ 记录 3 次事务回滚");

    // 记录事务失败
    for _ in 0..1 {
        metrics.record_transaction_failure();
    }
    println!("  ✓ 记录 1 次事务失败");

    // 7. 查询特定类型的指标
    println!("\n7️⃣ 查询 SELECT 查询的指标");
    println!("------------------------------------------");

    if let Some(select_stats) = metrics.get_query_stats("SELECT") {
        println!("  📊 SELECT 查询统计:");
        println!("    - 总查询数: {}", select_stats.count);
        println!("    - 错误数: {}", select_stats.error_count);
        println!("    - 错误率: {:.2}%", select_stats.error_rate() * 100.0);
        println!("    - 平均 QPS: {:.2}", select_stats.throughput.avg_qps);
        println!("\n  📈 延迟百分位:");
        println!("    - P50: {:?}", select_stats.latency_percentiles.p50());
        println!("    - P90: {:?}", select_stats.latency_percentiles.p90());
        println!("    - P95: {:?}", select_stats.latency_percentiles.p95());
        println!("    - P99: {:?}", select_stats.latency_percentiles.p99());
        println!("\n  📈 延迟范围:");
        println!("    - 最小: {:?}", select_stats.latency_percentiles.min());
        println!("    - 最大: {:?}", select_stats.latency_percentiles.max());
    }

    // 8. 查询所有查询类型的指标
    println!("\n8️⃣ 查询所有查询类型的指标");
    println!("------------------------------------------");

    let all_stats = metrics.all_query_stats();
    println!("  📊 所有查询类型统计:");
    for (query_type, stats) in &all_stats {
        println!(
            "    - {}: {} 次查询, {:.2}% 错误率",
            query_type,
            stats.count,
            stats.error_rate() * 100.0
        );
    }

    // 9. 查询总吞吐量
    println!("\n9️⃣ 查询总吞吐量");
    println!("------------------------------------------");

    let total_throughput = metrics.total_throughput();
    println!("  📊 总吞吐量统计:");
    println!("    - 总操作数: {}", total_throughput.total_operations);
    println!("    - 成功操作数: {}", total_throughput.success_count);
    println!("    - 失败操作数: {}", total_throughput.failure_count);
    println!("    - 错误率: {:.2}%", total_throughput.error_rate * 100.0);
    println!("    - 平均 QPS: {:.2}", total_throughput.avg_qps);

    // 10. 查询慢查询记录
    println!("\n🔟 查询慢查询记录");
    println!("------------------------------------------");

    let slow_queries = metrics.slow_queries();
    println!("  📊 慢查询记录 (阈值: 1000ms):");
    for (i, query) in slow_queries.iter().enumerate() {
        println!(
            "    {}. {} - {}ms (时间: {})",
            i + 1,
            query.query_type,
            query.duration_ms,
            query.timestamp
        );
    }

    // 11. 查询连接获取统计
    println!("\n1️⃣1️⃣ 查询连接获取统计");
    println!("------------------------------------------");

    let acquire_stats = metrics.connection_acquire_stats();
    println!("  📊 连接获取统计:");
    println!("    - 总尝试次数: {}", acquire_stats.total_attempts);
    println!("    - 成功次数: {}", acquire_stats.success_count);
    println!("    - 超时次数: {}", acquire_stats.timeout_count);
    println!("    - 失败次数: {}", acquire_stats.failure_count);
    println!("    - 超时率: {:.2}%", acquire_stats.timeout_rate * 100.0);

    // 12. 查询事务统计
    println!("\n1️⃣2️⃣ 查询事务统计");
    println!("------------------------------------------");

    let txn_stats = metrics.transaction_stats();
    println!("  📊 事务统计:");
    println!("    - 总事务数: {}", txn_stats.total_transactions);
    println!("    - 提交次数: {}", txn_stats.commit_count);
    println!("    - 回滚次数: {}", txn_stats.rollback_count);
    println!("    - 失败次数: {}", txn_stats.failure_count);
    println!("    - 成功率: {:.2}%", txn_stats.success_rate);

    // 13. 查询连接池状态
    println!("\n1️⃣3️⃣ 查询连接池状态");
    println!("------------------------------------------");

    let pool_metrics = metrics.pool_status();
    println!("  📊 连接池状态:");
    println!("    - 总连接数: {}", pool_metrics.total);
    println!("    - 活跃连接数: {}", pool_metrics.active);
    println!("    - 空闲连接数: {}", pool_metrics.idle);
    println!("    - 使用率: {:.2}%", pool_metrics.utilization_rate() * 100.0);

    // 14. 导出 Prometheus 格式指标
    println!("\n1️⃣4️⃣ 导出 Prometheus 格式指标");
    println!("------------------------------------------");

    let prometheus_output = metrics.export_prometheus();
    println!("  📄 Prometheus 指标导出:");
    println!("  {}", "=".repeat(60));

    // 打印前 50 行（避免输出过长）
    for (i, line) in prometheus_output.lines().take(50).enumerate() {
        println!("  {}", line);
    }

    if prometheus_output.lines().count() > 50 {
        println!("  ... (省略 {} 行)", prometheus_output.lines().count() - 50);
    }

    println!("  {}", "=".repeat(60));
    println!("  ✓ 指标已导出为 Prometheus 格式");
    println!("  📄 总行数: {}", prometheus_output.lines().count());

    // 15. 自定义慢查询阈值
    println!("\n1️⃣5️⃣ 自定义慢查询阈值");
    println!("------------------------------------------");

    metrics.set_slow_query_threshold(500); // 设置阈值为 500ms
    println!("  ✓ 慢查询阈值已更新为 500ms");

    // 记录一个新的慢查询
    let start = std::time::Instant::now();
    tokio::time::sleep(Duration::from_millis(600)).await;
    let duration = start.elapsed();
    metrics.record_query("SELECT", duration, true, Some(1024));
    println!("  ✓ 记录新的慢查询 (600ms)");

    let slow_queries = metrics.slow_queries();
    println!("  📊 更新后的慢查询记录数: {}", slow_queries.len());

    // 16. 获取运行时长
    println!("\n1️⃣6️⃣ 获取运行时长");
    println!("------------------------------------------");

    let uptime = metrics.uptime();
    println!("  ⏱️  运行时长: {:?}", uptime);

    println!("\n========================================");
    println!("✨ Prometheus 指标监控示例运行完成！");

    println!("\n💡 提示:");
    println!("  - 在生产环境中，可以将指标导出到 Prometheus 服务器");
    println!("  - 使用 Grafana 创建仪表板可视化指标");
    println!("  - 设置告警规则监控关键指标");
    println!("  - 定期检查慢查询并优化性能");

    Ok(())
}
