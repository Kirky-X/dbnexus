// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 基础连接示例
//!
//! 展示 dbnexus 的基本连接池与 Session 用法：
//! - 创建 SQLite 内存连接池
//! - 获取 admin Session
//! - 打印连接成功信息
//! - 展示连接池状态
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example basic_connection --features "sqlite,permission,macros"
//! ```

use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔌 DBNexus basic connection example");
    println!("========================================\n");

    // 创建 SQLite 内存连接池
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        admin_role: "admin".to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    println!("✓ Pool created");
    println!("  - database URL: {}", pool.config().url);
    println!(
        "  - max connections: {}",
        pool.config().pool_config.max_connections
    );
    println!(
        "  - min connections: {}",
        pool.config().pool_config.min_connections
    );
    println!("  - admin role: {}", pool.config().admin_role);

    // 获取 admin Session
    // Session 自动从连接池获取连接，并在 drop 时自动归还。
    // "admin" 角色在无权限配置文件时是允许的安全默认角色。
    let session = pool.get_session("admin").await?;
    println!("\n✓ Session acquired");
    println!("  - current role: {}", session.role());
    println!("  - in transaction: {}", session.is_in_transaction().await);

    // 打印连接成功信息
    println!("\n✓ Database connected!");

    // 展示连接池状态
    // session drop 后连接归还到池中，再查看状态
    drop(session);
    let status = pool.status();
    println!("\n📊 Pool status:");
    println!("  - total connections: {}", status.total);
    println!("  - active connections: {}", status.active);
    println!("  - idle connections: {}", status.idle);
    println!("  - waiters: {}", status.wait_count);
    println!("  - historical max waiters: {}", status.max_waiters);
    println!("  - borrow count: {}", status.borrow_count);
    println!("  - historical max active: {}", status.max_active);

    println!("\n========================================");
    println!("✨ Basic connection example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - DbPool::with_config() create a pool from DbConfig");
    println!("  - pool.get_session(role) get a Session with permission context");
    println!("  - pool.status() returns PoolStatus (total/active/idle etc.)");
    println!("  - Session returns the connection to the pool automatically on drop");

    Ok(())
}
