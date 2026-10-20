// Copyright (c) 2025-2026 Kirky.X🌠
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
    println!("🔧 DBNexus pool lifecycle management example");
    println!("========================================\n");

    // pool-warmup：连接预热
    println!("─── 1. pool-warmup connection prewarming ───\n");
    println!("  set DbConfig.auto_migrate = true and specify migrations_dir,");
    println!(
        "  DbPool::with_config() prebuilds 5 connections in parallel to avoid cold-start latency.\n"
    );

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
    println!("  ✓ pool created; PoolStatus metrics:");
    println!("    - total connections: {}", status.total);
    println!("    - active connections: {}", status.active);
    println!("    - idle connections: {}", status.idle);
    println!("    - borrow count: {}", status.borrow_count);

    // pool-warmup feature 启用时，total 应 >= min_connections (5)
    if status.total >= 5 {
        println!(
            "  ✓ pool-warmup in effect: {} connections prebuilt",
            status.total
        );
    } else {
        println!(
            "  ⚠ total={} < 5, some warmup may have failed (check the logs)",
            status.total
        );
    }

    println!();

    // pool-health-check：后台健康检查
    println!("─── 2. pool-health-check background health checks ───\n");
    println!(
        "  with the pool-health-check feature enabled, DbPool starts a background task on construction,"
    );
    println!(
        "  which periodically (every 30s by default) validates idle connections, dropping dead ones and"
    );
    println!(
        "  the health check interval can be set with the DB_HEALTH_CHECK_INTERVAL environment variable (seconds, range 5-300).\n"
    );

    println!("  ✓ pool-health-check feature enabled");
    println!(
        "     the background task starts with warmup_pool and stops automatically on Drop (health_check_shutdown Notify)"
    );
    // 等待一小段时间让后台任务有机会执行（实际场景中会持续运行）
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let status_after = warmup_pool.status();
    println!(
        "     pool status after 100ms: total={}, active={}, idle={}",
        status_after.total, status_after.active, status_after.idle
    );

    println!();

    // auto-migrate：自动迁移
    println!("─── 3. auto-migrate automatic migration ───\n");
    println!("  rebuilding them to maintain min_connections.");
    println!(
        "  DbPool::with_config() runs the migration scripts in that directory after the pool is created.\n"
    );

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
    println!(
        "  ✓ auto-migrate enabled; migration scripts ran automatically when the pool was created"
    );

    // 验证 users 表已创建：尝试 INSERT，若表不存在则会报错
    let session = migrate_pool.get_session("admin").await?;
    match session
        .execute_raw("INSERT INTO users (id, name) VALUES (1, 'alice')")
        .await
    {
        Ok(_) => println!(
            "  ✓ verified: the users table was created automatically via auto-migrate and INSERT succeeded"
        ),
        Err(e) => println!("  ⚠ INSERT users failed after auto-migrate: {}", e),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);

    println!();
    println!("========================================");
    println!("✨ Pool lifecycle management example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!(
        "  - pool-warmup: prebuilds min_connections connections in parallel, with timeout and retry"
    );
    println!(
        "  - pool-health-check: a background task validates connections periodically; the interval is configurable via DB_HEALTH_CHECK_INTERVAL"
    );
    println!(
        "  - auto-migrate: executes the SQL migration scripts in migrations_dir when the pool is created"
    );
    println!("  - all three are opt-in features, enabled via DbConfig + Cargo features");

    Ok(())
}
