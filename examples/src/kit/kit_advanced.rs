// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! DbNexusModule 高级示例：DI 驱动的多能力数据库操作
//!
//! 在 `kit_usage` 基础上演示更复杂的 `AsyncKit` + `DbNexusModule` 场景：
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

use dbnexus::DbNexusModule;
use dbnexus::database::ConnectionPool;
use dbnexus::domain::permission;
use dbnexus::foundation::{DbConfig, PoolConfig};
use dbnexus::observability::{MetricsCollector, MetricsCollectorTrait};
use oxcache::integrations::kit::{OxcacheConfig, OxcacheModule};
use trait_kit::prelude::*;

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🧰 DBNexus AsyncKit advanced example: pool + permission + metrics");
    println!("========================================\n");

    // ============================================
    // 1. 通过 AsyncKit 构建连接池
    // ============================================
    println!("--- 1. AsyncKit builds the connection pool ---\n");

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

    let kit = kit
        .build()
        .await
        .map_err(|e| format!("AsyncKit::build: {e}"))?;

    let pool: Arc<dyn ConnectionPool + Send + Sync> = kit
        .require::<DbNexusModule>()
        .map_err(|e| format!("require DbNexusModule: {e}"))?;
    println!("  ✓ pool fetched via AsyncKit DI successfully");

    // ============================================
    // 2. 初始化辅助能力（Metrics + Permission）
    // ============================================
    println!("\n--- 2. initialize Metrics + Permission ---\n");

    let metrics: Arc<MetricsCollector> = Arc::new(MetricsCollector::new());
    println!("  ✓ MetricsCollector created");

    let perm_provider: Arc<dyn dbnexus::PermissionProvider> = Arc::new(permission::new_in_memory());
    println!("  ✓ PermissionProvider created (in-memory implementation)");

    // ============================================
    // 3. 准备测试数据
    // ============================================
    println!("\n--- 3. prepare test data ---\n");

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
    println!("  ✓ orders test data prepared (3 rows)");

    // 同步连接池状态到指标
    let status = pool.status();
    metrics.record_pool_usage(
        status.total as u32,
        status.active as u32,
        status.idle as u32,
    );
    println!("  ✓ initial pool metrics recorded");

    // ============================================
    // 4. 业务编排：权限检查 → SQL 执行 → 指标记录
    // ============================================
    println!("\n--- 4. business orchestration: permissions → query → metrics ---\n");

    let test_role = "admin";
    let test_table = "orders";

    for i in 1..=3 {
        let start = std::time::Instant::now();

        // 权限检查
        let allowed = perm_provider
            .check(
                test_role,
                test_table,
                dbnexus::domain::PermissionAction::Select,
            )
            .await?;

        let duration = start.elapsed();

        if allowed {
            // 通过连接池获取 Session 执行 SQL
            let query_session = pool.get_session(test_role).await?;
            let _ = query_session
                .execute_raw("SELECT COUNT(*) FROM orders")
                .await?;
            metrics.record_query("select", duration, true, None);
            println!(
                "  request #{}: ✓ permission granted, query completed in {:?}",
                i, duration
            );
        } else {
            println!("  request #{}: ✗ permission denied, took {:?}", i, duration);
        }
    }

    // ============================================
    // 5. 事务回滚演示
    // ============================================
    println!("\n--- 5. transaction rollback ---\n");

    let txn_session = pool.get_session("admin").await?;
    txn_session.begin_transaction().await?;
    println!("  ✓ transaction started");

    txn_session
        .execute_raw("INSERT INTO orders (id, user_id, amount, status) VALUES (4, 99, 999.0, 'rollback-test')")
        .await?;
    println!("  ✓ inserted inside the transaction: id=4 (rollback-test)");

    // 验证插入成功（事务内可见）
    let _ = txn_session
        .execute_raw("SELECT COUNT(*) FROM orders WHERE id = 4")
        .await?;
    println!("  ✓ in-transaction query confirms id=4 exists");

    txn_session.rollback().await?;
    println!("  ✓ transaction rolled back");

    // 验证回滚后数据不存在
    let check_session = pool.get_session("admin").await?;
    let _ = check_session
        .execute_raw("SELECT COUNT(*) FROM orders WHERE id = 4")
        .await?;
    println!("  ✓ id=4 no longer exists after rollback");

    // ============================================
    // 6. 查看汇总指标
    // ============================================
    println!("\n--- 6. summary metrics ---\n");

    let pool_metrics = metrics.pool_metrics();
    println!(
        "  pool: total={}, active={}, idle={}",
        pool_metrics.total, pool_metrics.active, pool_metrics.idle
    );

    let stats = metrics.query_stats();
    println!(
        "  query: count={}, errors={}, p50={:?}, p99={:?}",
        stats.count,
        stats.error_count,
        stats.latency_percentiles.p50(),
        stats.latency_percentiles.p99(),
    );

    // ============================================
    // 7. 多角色 Session 并发
    // ============================================
    println!("\n--- 7. multi-role Session ---\n");

    let admin_session = pool.get_session("admin").await?;
    let system_session = pool.get_session("system").await?;

    println!("  ✓ admin  (role: {})", admin_session.role());
    println!("  ✓ system (role: {})", system_session.role());

    let final_status = pool.status();
    println!(
        "\n  final pool status: total={}, active={}, idle={}",
        final_status.total, final_status.active, final_status.idle
    );

    println!("\n========================================");
    println!("✨ AsyncKit advanced example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - AsyncKit DI                inject the database pool via the module system");
    println!("  - DbNexusModule              the AsyncKit module that builds the DbPool");
    println!("  - business orchestration      permissions → query → metrics");
    println!("  - transaction rollback       begin → insert → rollback → verify");
    println!(
        "  - MetricsCollector           independent of the kit; records query and pool metrics"
    );
    println!("  - multi-role Session         one pool supports different roles");

    Ok(())
}
