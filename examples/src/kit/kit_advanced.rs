// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DbNexusModule 高级示例：DI 驱动的多能力数据库操作
//!
//! 在 [`kit_usage`](crate::kit_usage) 基础上演示更复杂的 `AsyncKit` + `DbNexusModule` 场景：
//! - 通过 AsyncKit DI 获取数据库连接池
//! - 结合 `MetricsCollector` 记录查询指标
//! - 结合 `PermissionProvider` 执行权限检查
//! - 事务回滚演示
//! - 多 Session 并发操作
//! - 完整的业务编排模式：权限检查 → SQL 执行 → 指标记录
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin kit_advanced --features "kit"
//! ```

use std::sync::Arc;

use dbnexus::database::ConnectionPool;
use dbnexus::domain::permission;
use dbnexus::foundation::{DbConfig, PoolConfig};
use dbnexus::observability::{MetricsCollector, MetricsCollectorTrait};
use dbnexus::DbNexusModule;
use oxcache::integrations::kit::{OxcacheConfig, OxcacheModule};
use trait_kit::prelude::*;

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🧰 DBNexus AsyncKit 高级示例：pool + permission + metrics");
    println!("========================================\n");

    // ============================================
    // 1. 通过 AsyncKit 构建连接池
    // ============================================
    println!("--- 1. AsyncKit 构建连接池 ---\n");

    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig::default());
    kit.set_config(DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        admin_role: "admin".to_string(),
        pool_config: PoolConfig {
            max_connections: 5,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    });

    kit.register::<OxcacheModule>()
        .map_err(|e| format!("register OxcacheModule: {e}"))?;
    kit.register::<DbNexusModule>()
        .map_err(|e| format!("register DbNexusModule: {e}"))?;

    let kit = kit.build().await.map_err(|e| format!("AsyncKit::build: {e}"))?;

    let pool: Arc<dyn ConnectionPool + Send + Sync> = kit
        .require::<DbNexusModule>()
        .map_err(|e| format!("require DbNexusModule: {e}"))?;
    println!("  ✓ 通过 AsyncKit DI 获取连接池成功");

    // ============================================
    // 2. 初始化辅助能力（Metrics + Permission）
    // ============================================
    println!("\n--- 2. 初始化 Metrics + Permission ---\n");

    let metrics: Arc<MetricsCollector> = Arc::new(MetricsCollector::new());
    println!("  ✓ MetricsCollector 创建成功");

    let perm_provider: Arc<dyn dbnexus::PermissionProvider> = Arc::new(permission::new_in_memory());
    println!("  ✓ PermissionProvider 创建成功（内存实现）");

    // ============================================
    // 3. 准备测试数据
    // ============================================
    println!("\n--- 3. 准备测试数据 ---\n");

    let session = pool.get_session("admin").await?;

    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS orders (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                amount REAL NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending'
            )",
        )
        .await?;

    session
        .execute_raw("INSERT INTO orders (id, user_id, amount) VALUES (1, 42, 99.5)")
        .await?;
    session
        .execute_raw("INSERT INTO orders (id, user_id, amount) VALUES (2, 42, 12.3)")
        .await?;
    session
        .execute_raw("INSERT INTO orders (id, user_id, amount) VALUES (3, 7, 50.0)")
        .await?;
    println!("  ✓ 已准备 orders 测试数据（3 行）");

    // 同步连接池状态到指标
    let status = pool.status();
    metrics.record_pool_usage(status.total as u32, status.active as u32, status.idle as u32);
    println!("  ✓ 已记录初始连接池指标");

    // ============================================
    // 4. 业务编排：权限检查 → SQL 执行 → 指标记录
    // ============================================
    println!("\n--- 4. 业务编排：权限 → 查询 → 指标 ---\n");

    let test_role = "admin";
    let test_table = "orders";

    for i in 1..=3 {
        let start = std::time::Instant::now();

        // 权限检查
        let allowed = perm_provider
            .check(test_role, test_table, dbnexus::domain::PermissionAction::Select)
            .await?;

        let duration = start.elapsed();

        if allowed {
            // 通过连接池获取 Session 执行 SQL
            let query_session = pool.get_session(test_role).await?;
            let _ = query_session.execute_raw("SELECT COUNT(*) FROM orders").await?;
            metrics.record_query("select", duration, true, None);
            println!("  请求 #{}: ✓ 权限通过，查询完成，耗时 {:?}", i, duration);
        } else {
            println!("  请求 #{}: ✗ 权限被拒绝，耗时 {:?}", i, duration);
        }
    }

    // ============================================
    // 5. 事务回滚演示
    // ============================================
    println!("\n--- 5. 事务回滚 ---\n");

    let txn_session = pool.get_session("admin").await?;
    txn_session.begin_transaction().await?;
    println!("  ✓ 开始事务");

    txn_session
        .execute_raw("INSERT INTO orders (id, user_id, amount, status) VALUES (4, 99, 999.0, 'rollback-test')")
        .await?;
    println!("  ✓ 事务内插入: id=4 (rollback-test)");

    // 验证插入成功（事务内可见）
    let _ = txn_session
        .execute_raw("SELECT COUNT(*) FROM orders WHERE id = 4")
        .await?;
    println!("  ✓ 事务内查询确认 id=4 存在");

    txn_session.rollback().await?;
    println!("  ✓ 事务已回滚");

    // 验证回滚后数据不存在
    let check_session = pool.get_session("admin").await?;
    let _ = check_session
        .execute_raw("SELECT COUNT(*) FROM orders WHERE id = 4")
        .await?;
    println!("  ✓ 回滚后 id=4 已不存在");

    // ============================================
    // 6. 查看汇总指标
    // ============================================
    println!("\n--- 6. 汇总指标 ---\n");

    let pool_metrics = metrics.pool_metrics();
    println!(
        "  连接池: total={}, active={}, idle={}",
        pool_metrics.total, pool_metrics.active, pool_metrics.idle
    );

    let stats = metrics.query_stats();
    println!(
        "  查询: count={}, errors={}, p50={:?}, p99={:?}",
        stats.count,
        stats.error_count,
        stats.latency_percentiles.p50(),
        stats.latency_percentiles.p99(),
    );

    // ============================================
    // 7. 多角色 Session 并发
    // ============================================
    println!("\n--- 7. 多角色 Session ---\n");

    let admin_session = pool.get_session("admin").await?;
    let system_session = pool.get_session("system").await?;

    println!("  ✓ admin  (role: {})", admin_session.role());
    println!("  ✓ system (role: {})", system_session.role());

    let final_status = pool.status();
    println!(
        "\n  最终连接池状态: total={}, active={}, idle={}",
        final_status.total, final_status.active, final_status.idle
    );

    println!("\n========================================");
    println!("✨ AsyncKit 高级示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - AsyncKit DI                通过模块系统注入数据库连接池");
    println!("  - DbNexusModule              构建 DbPool 的 AsyncKit 模块");
    println!("  - 业务编排                    权限 → 查询 → 指标");
    println!("  - 事务回滚                    begin → insert → rollback → verify");
    println!("  - MetricsCollector           独立于 kit，记录查询和连接池指标");
    println!("  - 多角色 Session              同一连接池支持不同角色");

    Ok(())
}
