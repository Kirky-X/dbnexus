// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

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
    println!("🔌 DBNexus 基础连接示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建 SQLite 内存连接池
    // ============================================
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        admin_role: "admin".to_string(),
        max_connections: 5,
        min_connections: 1,
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");
    println!("  - 数据库 URL: {}", pool.config().url);
    println!("  - 最大连接数: {}", pool.config().max_connections);
    println!("  - 最小连接数: {}", pool.config().min_connections);
    println!("  - 管理员角色: {}", pool.config().admin_role);

    // ============================================
    // 2. 获取 admin Session
    // ============================================
    // Session 自动从连接池获取连接，并在 drop 时自动归还。
    // "admin" 角色在无权限配置文件时是允许的安全默认角色。
    let session = pool.get_session("admin").await?;
    println!("\n✓ Session 获取成功");
    println!("  - 当前角色: {}", session.role());
    println!("  - 是否在事务中: {}", session.is_in_transaction().await);

    // ============================================
    // 3. 打印连接成功信息
    // ============================================
    println!("\n✓ 数据库连接成功！");

    // ============================================
    // 4. 展示连接池状态
    // ============================================
    // session drop 后连接归还到池中，再查看状态
    drop(session);
    let status = pool.status();
    println!("\n📊 连接池状态:");
    println!("  - 总连接数: {}", status.total);
    println!("  - 活跃连接: {}", status.active);
    println!("  - 空闲连接: {}", status.idle);
    println!("  - 等待线程: {}", status.wait_count);
    println!("  - 历史最大等待: {}", status.max_waiters);
    println!("  - 借用次数: {}", status.borrow_count);
    println!("  - 历史最大活跃: {}", status.max_active);

    println!("\n========================================");
    println!("✨ 基础连接示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - DbPool::with_config() 使用 DbConfig 创建连接池");
    println!("  - pool.get_session(role) 获取带权限上下文的 Session");
    println!("  - pool.status() 返回 PoolStatus（total/active/idle 等）");
    println!("  - Session 在 drop 时自动归还连接到池中");

    Ok(())
}
