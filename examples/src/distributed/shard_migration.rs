// Copyright (c) 2026 Kirky.X
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
    println!("🚀 DBNexus 分片迁移编排器示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建分片路由器
    // ============================================
    println!("--- 1. 创建分片路由器 ---\n");

    let config = ShardConfig::new("hash", 4, "shard", "sqlite::memory:");
    let router = ShardRouter::with_config(&config).await?;
    let router = Arc::new(router);

    println!("  ✓ 路由器创建成功");
    println!("  - 分片数     : {}", router.total_shards());
    println!("  - 已初始化池 : {:?}", router.initialized_shards());
    println!();

    // ============================================
    // 2. 分片迁移编排器说明
    // ============================================
    println!("--- 2. ShardMigrationOrchestrator ---\n");

    println!("  分片迁移编排器负责:");
    println!("  - 遍历所有已注册分片连接池");
    println!("  - 对每个分片独立执行相同的迁移文件");
    println!("  - 支持并行和串行两种执行模式");
    println!("  - 部分失败不阻断其他分片");
    println!();

    // ============================================
    // 3. 并行 vs 串行模式对比
    // ============================================
    println!("--- 3. 并行 vs 串行模式 ---\n");

    println!("  ┌──────────┬──────────────────────────────────────────────┐");
    println!("  │ 模式     │ 说明                                         │");
    println!("  ├──────────┼──────────────────────────────────────────────┤");
    println!("  │ 并行     │ 所有分片同时执行迁移，速度最快               │");
    println!("  │          │ 适合分片间无依赖的场景                       │");
    println!("  ├──────────┼──────────────────────────────────────────────┤");
    println!("  │ 串行     │ 逐个分片顺序执行迁移                         │");
    println!("  │          │ 适合需要严格控制迁移顺序的场景                 │");
    println!("  └──────────┴──────────────────────────────────────────────┘");
    println!();

    // ============================================
    // 4. 迁移结果结构
    // ============================================
    println!("--- 4. 迁移结果结构 ---\n");

    println!("  OrchestratedMigrationResult:");
    println!("  ┌────────────────────┬──────────────────────────────────┐");
    println!("  │ 字段               │ 说明                             │");
    println!("  ├────────────────────┼──────────────────────────────────┤");
    println!("  │ total_shards       │ 总分片数                         │");
    println!("  │ success_count      │ 成功分片数                       │");
    println!("  │ failed_shards      │ 失败分片列表                     │");
    println!("  │ results            │ 所有分片结果详情                 │");
    println!("  └────────────────────┴──────────────────────────────────┘");
    println!();

    println!("  ShardMigrationResult（单分片）:");
    println!("  ┌────────────────────┬──────────────────────────────────┐");
    println!("  │ 字段               │ 说明                             │");
    println!("  ├────────────────────┼──────────────────────────────────┤");
    println!("  │ shard_id           │ 分片 ID                          │");
    println!("  │ success            │ 是否成功                         │");
    println!("  │ applied_versions   │ 已应用的迁移版本列表             │");
    println!("  │ error              │ 错误信息（失败时）               │");
    println!("  └────────────────────┴──────────────────────────────────┘");
    println!();

    // ============================================
    // 5. 模拟迁移结果展示
    // ============================================
    println!("--- 5. 模拟迁移结果 ---\n");

    let total_shards = router.total_shards();
    println!("  模拟 {} 个分片的迁移结果:", total_shards);
    println!();

    // 模拟成功场景
    println!("  场景 A: 全部成功");
    println!("  ┌──────────┬──────────┬────────────────────┐");
    println!("  │ shard_id │ success  │ applied_versions   │");
    println!("  ├──────────┼──────────┼────────────────────┤");
    for i in 0..total_shards {
        println!("  │ {:>8} │ {:<8} │ [1, 2, 3]          │", i, "✅");
    }
    println!("  └──────────┴──────────┴────────────────────┘");
    println!("  结果: total={}, success={}, failed=0", total_shards, total_shards);
    println!();

    // 模拟部分失败场景
    println!("  场景 B: 部分失败");
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
    println!("  结果: total={}, success={}, failed=1", total_shards, total_shards - 1);
    println!();

    // ============================================
    // 6. 架构说明
    // ============================================
    println!("--- 6. 分片迁移编排架构 ---\n");

    println!("  ┌─────────────────────────┐");
    println!("  │ ShardMigrationOrchestrator│");
    println!("  └───────────┬─────────────┘");
    println!("              │ 遍历分片");
    println!("     ┌────────┼────────┬──────────┐");
    println!("     ▼        ▼        ▼          ▼");
    println!("  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐");
    println!("  │Shard0│ │Shard1│ │Shard2│ │Shard3│");
    println!("  │迁移  │ │迁移  │ │迁移  │ │迁移  │");
    println!("  └──┬───┘ └──┬───┘ └──┬───┘ └──┬───┘");
    println!("     │        │        │        │");
    println!("     └────────┴────────┴────────┘");
    println!("              │ 汇总结果");
    println!("              ▼");
    println!("  ┌───────────────────────────┐");
    println!("  │ OrchestratedMigrationResult│");
    println!("  └───────────────────────────┘");
    println!();

    // ============================================
    // 7. 与其他模块的关系
    // ============================================
    println!("--- 7. 与其他模块的关系 ---\n");
    println!("  ┌────────────────┬──────────────────────────────────────────┐");
    println!("  │ 依赖模块       │ 关系                                     │");
    println!("  ├────────────────┼──────────────────────────────────────────┤");
    println!("  │ sharding       │ 提供 ShardRouter 获取所有分片连接池      │");
    println!("  │ migration      │ 提供 MigrationExecutor 执行单分片迁移    │");
    println!("  └────────────────┴──────────────────────────────────────────┘");
    println!();

    println!("========================================");
    println!("✨ 分片迁移编排器示例完成！");
    println!("========================================");
    println!("\n📚 关键 API:");
    println!("  - ShardMigrationOrchestrator::new(router, parallel)");
    println!("  - orchestrator.orchestrate_migration(dir) -> OrchestratedMigrationResult");
    println!("  - result.total_shards / success_count / failed_shards");
    Ok(())
}
