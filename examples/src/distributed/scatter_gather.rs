// Copyright (c) 2025-2026 Kirky.X🌠
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

use dbnexus::{
    AggregateValue, PartialFailurePolicy, ScatterGatherExecutor, ShardConfig, ShardRouter,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🌐 DBNexus Scatter-Gather cross-shard query example");
    println!("========================================\n");

    // 创建分片路由器
    println!("--- 1. create the shard router ---\n");

    let config = ShardConfig::new("hash", 4, "shard", "sqlite::memory:");
    let router = ShardRouter::with_config(&config).await?;
    let router = Arc::new(router);

    println!("  ✓ Router created");
    println!("  - shard count      : {}", router.total_shards());
    println!("  - initialized pools  : {:?}", router.initialized_shards());
    println!();

    // 创建 Scatter-Gather 执行器
    println!("--- 2. create the Scatter-Gather executor ---\n");

    let timeout = Duration::from_secs(10);
    let policy = PartialFailurePolicy::BestEffort;

    let executor = ScatterGatherExecutor::new(Arc::clone(&router), timeout, policy);

    println!("  ✓ executor created");
    println!("  - timeout       : {:?}", timeout);
    println!("  - failure policy   : {:?}", policy);
    println!();

    // 执行 scatter 查询
    println!("--- 3. execute a Scatter query ---\n");
    println!("  send SQL to all shards in parallel and collect each shard's row count\n");

    let sql = "SELECT 1";
    match executor.scatter_query(sql, "admin").await {
        Ok(result) => {
            println!("  ✅ Scatter query complete!");
            println!("  - rows returned per shard:");
            for (shard_id, row_count) in &result.shard_row_counts {
                println!("    - shard {}: {} rows", shard_id, row_count);
            }
            if !result.failed_shards.is_empty() {
                println!("  - failed shards:");
                for error in &result.failed_shards {
                    println!("    - shard {}: {}", error.shard_id, error.error);
                }
            }
        }
        Err(e) => {
            println!("  ❌ Scatter query failed: {}", e);
        }
    }
    println!();

    // 聚合函数演示
    println!("--- 4. aggregate functions ---\n");

    let shard_counts = vec![(0, 150_u64), (1, 230), (2, 180), (3, 90)];
    let total_rows: u64 = shard_counts.iter().map(|(_, c)| c).sum();

    println!("  simulated data (row counts per shard):");
    for (shard_id, count) in &shard_counts {
        println!("    - shard {}: {} rows", shard_id, count);
    }
    println!();

    // COUNT 聚合
    let count_result = AggregateValue::Count(total_rows as i64);
    println!("  COUNT aggregation: {:?}", count_result);

    // 模拟 SUM 聚合（各分片金额总和）
    let amounts = vec![15000.0, 23000.0, 18000.0, 9000.0];
    let sum_result = ScatterGatherExecutor::aggregate_sum(&amounts);
    println!("  SUM aggregation  : {:?}", sum_result);

    // AVG 聚合
    let avg_result = ScatterGatherExecutor::aggregate_avg(&amounts);
    println!("  AVG aggregation : {:?}", avg_result);

    // MIN / MAX 聚合
    let min_result = ScatterGatherExecutor::aggregate_min(&amounts);
    let max_result = ScatterGatherExecutor::aggregate_max(&amounts);
    println!("  MIN aggregation  : {:?}", min_result);
    println!("  MAX aggregation  : {:?}", max_result);
    println!();

    // PartialFailurePolicy 对比
    println!("--- 5. partial failure policy comparison ---\n");

    println!("  ┌──────────────┬──────────────────────────────────────────────┐");
    println!("  │ policy          │ description                                    │");
    println!("  ├──────────────┼──────────────────────────────────────────────┤");
    println!("  │ Fail         │ the whole query fails if any shard fails       │");
    println!("  │ BestEffort   │ return the results of the succeeded shards + failure info │");
    println!("  └──────────────┴──────────────────────────────────────────────┘");
    println!();

    // AggregateFunction 类型
    println!("--- 6. AggregateFunction types ---\n");

    let agg_functions = [
        ("Count", "count the total rows"),
        ("Sum(column)", "sum the specified column"),
        ("Avg(column)", "average the specified column"),
        ("Min(column)", "min of the specified column"),
        ("Max(column)", "max of the specified column"),
    ];

    println!("  ┌─────────────────┬──────────────────────────┐");
    println!("  │ aggregate func      │ description                      │");
    println!("  ├─────────────────┼──────────────────────────┤");
    for (name, desc) in &agg_functions {
        println!("  │ {:<15} │ {:<24} │", name, desc);
    }
    println!("  └─────────────────┴──────────────────────────┘");
    println!();

    // 架构说明
    println!("--- 7. Scatter-Gather architecture ---\n");
    println!("  ┌──────────┐     SQL      ┌─────────┐");
    println!("  │ Executor │ ────────────→ │ Shard 0 │");
    println!("  │          │ ────────────→ │ Shard 1 │");
    println!("  │          │ ────────────→ │ Shard 2 │");
    println!("  │          │ ────────────→ │ Shard 3 │");
    println!("  └────┬─────┘               └─────────┘");
    println!("              │ aggregate results");
    println!("       ▼");
    println!("  ┌──────────────┐");
    println!("  │ ScatterResult│");
    println!("  │ - row_counts │");
    println!("  │ - failed     │");
    println!("  │ - aggregated │");
    println!("  └──────────────┘");
    println!();

    println!("========================================");
    println!("✨ Scatter-Gather cross-shard query example complete!");
    println!("========================================");
    println!("\n📚 Key APIs:");
    println!("  - ScatterGatherExecutor::new(router, timeout, policy)");
    println!("  - executor.scatter_query(sql, role) -> ScatterResult");
    println!("  - ScatterGatherExecutor::aggregate_sum/avg/min/max()");
    Ok(())
}
