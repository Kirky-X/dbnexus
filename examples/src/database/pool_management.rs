// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 连接池生命周期管理示例
//!
//! 演示 dbnexus 连接池的三大生命周期增强特性：
//! - **pool-warmup**：连接池创建时预建 `min_connections` 个连接，避免冷启动延迟
//! - **pool-health-check**：后台任务定期校验空闲连接，自动剔除失效连接并重建
//! - **auto-migrate**：连接池创建时自动执行 `migrations_dir` 下的迁移脚本
//!
//! 这三个特性均由 `DbPool::with_config()` 在构造时根据 feature 与配置自动触发，
//! 无需用户显式调用——本示例通过 `PoolStatus` 指标观察其效果。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin pool_management
//! ```

use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔧 DBNexus 连接池生命周期管理示例");
    println!("========================================\n");

    // ============================================
    // 1. pool-warmup：连接预热
    // ============================================
    println!("─── 1. pool-warmup 连接预热 ───\n");
    println!("  配置 min_connections = 5，启用 pool-warmup feature 后，");
    println!("  DbPool::with_config() 会并行预建 5 个连接，避免冷启动延迟。\n");

    let warmup_config = DbConfig {
        url: "sqlite::memory:".to_string(),
        admin_role: "admin".to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 20,
            min_connections: 5,
            ..Default::default()
        },
        ..Default::default()
    };

    let warmup_pool = DbPool::with_config(warmup_config).await?;
    let status = warmup_pool.status();
    println!("  ✓ 连接池创建完成，PoolStatus 指标：");
    println!("    - total connections: {}", status.total);
    println!("    - active connections: {}", status.active);
    println!("    - idle connections: {}", status.idle);
    println!("    - borrow count: {}", status.borrow_count);

    // pool-warmup feature 启用时，total 应 >= min_connections (5)
    if status.total >= 5 {
        println!("  ✓ pool-warmup 生效：预建了 {} 个连接", status.total);
    } else {
        println!("  ⚠ total={} < 5，可能部分预热失败（检查日志）", status.total);
    }

    println!();

    // ============================================
    // 2. pool-health-check：后台健康检查
    // ============================================
    println!("─── 2. pool-health-check 后台健康检查 ───\n");
    println!("  启用 pool-health-check feature 后，DbPool 构造时自动启动后台任务，");
    println!("  定期（默认 30s）校验空闲连接，自动剔除失效连接并重建以维持 min_connections。");
    println!("  健康检查间隔可通过环境变量 DB_HEALTH_CHECK_INTERVAL（秒，范围 5-300）配置。\n");

    println!("  ✓ pool-health-check feature 已启用");
    println!("    后台任务已随 warmup_pool 启动，Drop 时自动停止（health_check_shutdown Notify）");
    // 等待一小段时间让后台任务有机会执行（实际场景中会持续运行）
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let status_after = warmup_pool.status();
    println!(
        "    100ms 后连接池状态：total={}, active={}, idle={}",
        status_after.total, status_after.active, status_after.idle
    );

    println!();

    // ============================================
    // 3. auto-migrate：自动迁移
    // ============================================
    println!("─── 3. auto-migrate 自动迁移 ───\n");
    println!("  设置 DbConfig.auto_migrate = true 并指定 migrations_dir 后，");
    println!("  DbPool::with_config() 会在连接池创建完成后自动执行该目录下的迁移脚本。\n");

    // 准备临时迁移目录与 SQL 文件
    let temp_dir = std::env::temp_dir().join("dbnexus_pool_management_migrations");
    std::fs::create_dir_all(&temp_dir)?;
    let migration_sql = temp_dir.join("001_create_users.sql");
    std::fs::write(
        &migration_sql,
        "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
    )?;

    let migrate_config = DbConfig {
        url: "sqlite::memory:".to_string(),
        admin_role: "admin".to_string(),
        migrations_dir: Some(temp_dir.clone()),
        auto_migrate: true,
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };

    let migrate_pool = DbPool::with_config(migrate_config).await?;
    println!("  ✓ auto-migrate 已启用，连接池创建时自动执行了迁移脚本");

    // 验证 users 表已创建：尝试 INSERT，若表不存在则会报错
    let session = migrate_pool.get_session("admin").await?;
    match session
        .execute_raw("INSERT INTO users (id, name) VALUES (1, 'alice')")
        .await
    {
        Ok(_) => println!("  ✓ 验证成功：users 表已通过 auto-migrate 自动创建，INSERT 成功"),
        Err(e) => println!("  ⚠ auto-migrate 后 INSERT users 失败：{}", e),
    }

    // 清理临时迁移文件
    let _ = std::fs::remove_dir_all(&temp_dir);

    println!();
    println!("========================================");
    println!("✨ 连接池生命周期管理示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - pool-warmup: 并行预建 min_connections 个连接，带超时与重试");
    println!("  - pool-health-check: 后台任务定期校验连接，间隔可由 DB_HEALTH_CHECK_INTERVAL 配置");
    println!("  - auto-migrate: 连接池构造时自动执行 migrations_dir 下的 SQL 迁移脚本");
    println!("  - 三者均为 opt-in feature，通过 DbConfig + Cargo features 组合启用");

    Ok(())
}
