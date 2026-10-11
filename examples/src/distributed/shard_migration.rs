// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 分片迁移编排器示例
//!
//! 演示 `ShardMigrationOrchestrator` 的使用：
//! - 创建分片路由器
//! - 配置迁移编排器（并行/串行模式）
//! - 执行跨分片迁移
//! - 查看 `OrchestratedMigrationResult` 结果
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin shard_migration
//! ```

use dbnexus::{ShardConfig, ShardRouter};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🚀 DBNexus shard migration orchestrator example");
    println!("========================================\n");

    // ============================================
    // 创建分片路由器
    // ============================================
    println!("--- 1. create the shard router ---\n");

    let config = ShardConfig::new("hash", 4, "shard", "sqlite::memory:");
    let router = ShardRouter::with_config(&config).await?;
    let router = Arc::new(router);

    println!("  ✓ Router created");
    println!("  - shard count     : {}", router.total_shards());
    println!("  - initialized pools : {:?}", router.initialized_shards());
    println!();

    // ============================================
    // 分片迁移编排器说明
    // ============================================
    println!("--- 2. ShardMigrationOrchestrator ---\n");

    println!("  the shard migration orchestrator:");
    println!("  - iterate over all registered shard pools");
    println!("  - execute the same migration file on each shard independently");
    println!("  - supports both parallel and serial execution modes");
    println!("  - a partial failure does not block the other shards");
    println!();

    // ============================================
    // 并行 vs 串行模式对比
    // ============================================
    println!("--- 3. parallel vs serial modes ---\n");

    println!("  ┌──────────┬──────────────────────────────────────────────┐");
    println!("  │ mode       │ description                                    │");
    println!("  ├──────────┼──────────────────────────────────────────────┤");
    println!("  │ parallel   │ execute migrations on all shards at once; fastest │");
    println!("  │          │ good for scenarios with no cross-shard dependencies │");
    println!("  ├──────────┼──────────────────────────────────────────────┤");
    println!("  │ serial     │ execute migrations shard by shard in order      │");
    println!("  │          │ good for scenarios needing strict migration ordering  │");
    println!("  └──────────┴──────────────────────────────────────────────┘");
    println!();

    // ============================================
    // 迁移结果结构
    // ============================================
    println!("--- 4. migration result structures ---\n");

    println!("  OrchestratedMigrationResult:");
    println!("  ┌────────────────────┬──────────────────────────────────┐");
    println!("  │ field               │ description                             │");
    println!("  ├────────────────────┼──────────────────────────────────┤");
    println!("  │ total_shards       │ the total number of shards                │");
    println!("  │ success_count      │ the number of succeeded shards             │");
    println!("  │ failed_shards      │ the list of failed shards                 │");
    println!("  │ results            │ details of all shard results              │");
    println!("  └────────────────────┴──────────────────────────────────┘");
    println!();

    println!("  ShardMigrationResult (single shard):");
    println!("  ┌────────────────────┬──────────────────────────────────┐");
    println!("  │ field               │ description                             │");
    println!("  ├────────────────────┼──────────────────────────────────┤");
    println!("  │ shard_id           │ shard ID                                  │");
    println!("  │ success            │ whether it succeeded                      │");
    println!("  │ applied_versions   │ the list of applied migration versions    │");
    println!("  │ error              │ the error message (on failure)            │");
    println!("  └────────────────────┴──────────────────────────────────┘");
    println!();

    // ============================================
    // 模拟迁移结果展示
    // ============================================
    println!("--- 5. simulated migration results ---\n");

    let total_shards = router.total_shards();
    println!("  simulated migration results for {} shards:", total_shards);
    println!();

    println!("  Scenario A: all succeed");
    println!("  ┌──────────┬──────────┬────────────────────┐");
    println!("  │ shard_id │ success  │ applied_versions   │");
    println!("  ├──────────┼──────────┼────────────────────┤");
    for i in 0..total_shards {
        println!("  │ {:>8} │ {:<8} │ [1, 2, 3]          │", i, "✅");
    }
    println!("  └──────────┴──────────┴────────────────────┘");
    println!(
        "  result: total={}, success={}, failed=0",
        total_shards, total_shards
    );
    println!();

    println!("  Scenario B: partial failure");
    println!("  ┌──────────┬──────────┬────────────────────────────┐");
    println!("  │ shard_id │ success  │ error                      │");
    println!("  ├──────────┼──────────┼────────────────────────────┤");
    for i in 0..total_shards {
        if i == 2 {
            println!("  │ {:>8} │ {:<8} │ connection timeout         │", i, "❌");
        } else {
            println!("  │ {:>8} │ {:<8} │                            │", i, "✅");
        }
    }
    println!("  └──────────┴──────────┴────────────────────────────┘");
    println!(
        "  result: total={}, success={}, failed=1",
        total_shards,
        total_shards - 1
    );
    println!();

    // ============================================
    // 架构说明
    // ============================================
    println!("--- 6. shard migration orchestration architecture ---\n");

    println!("  ┌─────────────────────────┐");
    println!("  │ ShardMigrationOrchestrator│");
    println!("  └───────────┬─────────────┘");
    println!("              │ iterate over shards");
    println!("     ┌────────┼────────┬──────────┐");
    println!("     ▼        ▼        ▼          ▼");
    println!("  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐");
    println!("  │Shard0│ │Shard1│ │Shard2│ │Shard3│");
    println!("  │migration  │ │migration  │ │migration  │ │migration  │");
    println!("  └──┬───┘ └──┬───┘ └──┬───┘ └──┬───┘");
    println!("     │        │        │        │");
    println!("     └────────┴────────┴────────┘");
    println!("              │ aggregate results");
    println!("              ▼");
    println!("  ┌───────────────────────────┐");
    println!("  │ OrchestratedMigrationResult│");
    println!("  └───────────────────────────┘");
    println!();

    // ============================================
    // 与其他模块的关系
    // ============================================
    println!("--- 7. relations to other modules ---\n");
    println!("  ┌────────────────┬──────────────────────────────────────────┐");
    println!("  │ dependencies       │ relationship                               │");
    println!("  ├────────────────┼──────────────────────────────────────────┤");
    println!("  │ sharding       │ provides ShardRouter to get all shard pools  │");
    println!("  │ migration      │ provides MigrationExecutor for single-shard migrations │");
    println!("  └────────────────┴──────────────────────────────────────────┘");
    println!();

    println!("========================================");
    println!("✨ Shard migration orchestrator example complete!");
    println!("========================================");
    println!("\n📚 Key APIs:");
    println!("  - ShardMigrationOrchestrator::new(router, parallel)");
    println!("  - orchestrator.orchestrate_migration(dir) -> OrchestratedMigrationResult");
    println!("  - result.total_shards / success_count / failed_shards");
    Ok(())
}
