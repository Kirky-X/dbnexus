// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 跨分片查询引擎 Scatter-Gather 示例
//!
//! 演示 `ScatterGatherExecutor` 的使用：
//! - 创建分片路由器并注册多个 SQLite 分片
//! - 配置 `PartialFailurePolicy`（Fail / BestEffort）
//! - 执行 scatter-gather 并行查询
//! - 聚合函数（COUNT / SUM / AVG / MIN / MAX）
//! - 超时控制
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin scatter_gather
//! ```

use dbnexus::{AggregateValue, PartialFailurePolicy, ScatterGatherExecutor, ShardConfig, ShardRouter};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🌐 DBNexus Scatter-Gather 跨分片查询示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建分片路由器
    // ============================================
    println!("--- 1. 创建分片路由器 ---\n");

    let config = ShardConfig::new("hash", 4, "shard", "sqlite::memory:");
    let router = ShardRouter::with_config(&config).await?;
    let router = Arc::new(router);

    println!("  ✓ 路由器创建成功");
    println!("  - 分片数      : {}", router.total_shards());
    println!("  - 已初始化池  : {:?}", router.initialized_shards());
    println!();

    // ============================================
    // 2. 创建 Scatter-Gather 执行器
    // ============================================
    println!("--- 2. 创建 Scatter-Gather 执行器 ---\n");

    let timeout = Duration::from_secs(10);
    let policy = PartialFailurePolicy::BestEffort;

    let executor = ScatterGatherExecutor::new(Arc::clone(&router), timeout, policy);

    println!("  ✓ 执行器创建成功");
    println!("  - 超时       : {:?}", timeout);
    println!("  - 失败策略   : {:?}", policy);
    println!();

    // ============================================
    // 3. 执行 scatter 查询
    // ============================================
    println!("--- 3. 执行 Scatter 查询 ---\n");
    println!("  向所有分片并行发送 SQL，收集各分片返回的行数\n");

    let sql = "SELECT 1";
    match executor.scatter_query(sql, "admin").await {
        Ok(result) => {
            println!("  ✅ Scatter 查询完成！");
            println!("  - 分片返回行数:");
            for (shard_id, row_count) in &result.shard_row_counts {
                println!("    - shard {}: {} 行", shard_id, row_count);
            }
            if !result.failed_shards.is_empty() {
                println!("  - 失败分片:");
                for error in &result.failed_shards {
                    println!("    - shard {}: {}", error.shard_id, error.error);
                }
            }
        }
        Err(e) => {
            println!("  ❌ Scatter 查询失败: {}", e);
        }
    }
    println!();

    // ============================================
    // 4. 聚合函数演示
    // ============================================
    println!("--- 4. 聚合函数 ---\n");

    // 模拟各分片返回的行数
    let shard_counts = vec![(0, 150_u64), (1, 230), (2, 180), (3, 90)];
    let total_rows: u64 = shard_counts.iter().map(|(_, c)| c).sum();

    println!("  模拟数据（各分片行数）:");
    for (shard_id, count) in &shard_counts {
        println!("    - shard {}: {} 行", shard_id, count);
    }
    println!();

    // COUNT 聚合
    let count_result = AggregateValue::Count(total_rows as i64);
    println!("  COUNT 聚合: {:?}", count_result);

    // 模拟 SUM 聚合（各分片金额总和）
    let amounts = vec![15000.0, 23000.0, 18000.0, 9000.0];
    let sum_result = ScatterGatherExecutor::aggregate_sum(&amounts);
    println!("  SUM 聚合  : {:?}", sum_result);

    // AVG 聚合
    let avg_result = ScatterGatherExecutor::aggregate_avg(&amounts);
    println!("  AVG 聚合  : {:?}", avg_result);

    // MIN / MAX 聚合
    let min_result = ScatterGatherExecutor::aggregate_min(&amounts);
    let max_result = ScatterGatherExecutor::aggregate_max(&amounts);
    println!("  MIN 聚合  : {:?}", min_result);
    println!("  MAX 聚合  : {:?}", max_result);
    println!();

    // ============================================
    // 5. PartialFailurePolicy 对比
    // ============================================
    println!("--- 5. 部分失败策略对比 ---\n");

    println!("  ┌──────────────┬──────────────────────────────────────────────┐");
    println!("  │ 策略         │ 说明                                         │");
    println!("  ├──────────────┼──────────────────────────────────────────────┤");
    println!("  │ Fail         │ 任何分片失败则整体失败                       │");
    println!("  │ BestEffort   │ 返回已成功分片的结果 + 失败分片信息          │");
    println!("  └──────────────┴──────────────────────────────────────────────┘");
    println!();

    // ============================================
    // 6. AggregateFunction 类型
    // ============================================
    println!("--- 6. AggregateFunction 类型 ---\n");

    let agg_functions = [
        ("Count", "统计总行数"),
        ("Sum(column)", "对指定列求和"),
        ("Avg(column)", "对指定列求平均"),
        ("Min(column)", "对指定列求最小值"),
        ("Max(column)", "对指定列求最大值"),
    ];

    println!("  ┌─────────────────┬──────────────────────────┐");
    println!("  │ 聚合函数        │ 说明                     │");
    println!("  ├─────────────────┼──────────────────────────┤");
    for (name, desc) in &agg_functions {
        println!("  │ {:<15} │ {:<24} │", name, desc);
    }
    println!("  └─────────────────┴──────────────────────────┘");
    println!();

    // ============================================
    // 7. 架构说明
    // ============================================
    println!("--- 7. Scatter-Gather 架构 ---\n");
    println!("  ┌──────────┐     SQL      ┌─────────┐");
    println!("  │ Executor │ ────────────→ │ Shard 0 │");
    println!("  │          │ ────────────→ │ Shard 1 │");
    println!("  │          │ ────────────→ │ Shard 2 │");
    println!("  │          │ ────────────→ │ Shard 3 │");
    println!("  └────┬─────┘               └─────────┘");
    println!("       │ 收集结果 + 聚合");
    println!("       ▼");
    println!("  ┌──────────────┐");
    println!("  │ ScatterResult│");
    println!("  │ - row_counts │");
    println!("  │ - failed     │");
    println!("  │ - aggregated │");
    println!("  └──────────────┘");
    println!();

    println!("========================================");
    println!("✨ Scatter-Gather 跨分片查询示例完成！");
    println!("========================================");
    println!("\n📚 关键 API:");
    println!("  - ScatterGatherExecutor::new(router, timeout, policy)");
    println!("  - executor.scatter_query(sql, role) -> ScatterResult");
    println!("  - ScatterGatherExecutor::aggregate_sum/avg/min/max()");
    Ok(())
}
