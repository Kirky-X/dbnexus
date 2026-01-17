// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 快速开始示例
//!
//! 展示 dbnexus 的基本使用方法，包括：
//! - 定义 Entity 并自动生成 CRUD 方法
//! - 创建数据库连接池
//! - 获取 Session 执行数据库操作
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example quickstart --features sqlite
//! ```

use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化连接池（使用 SQLite 内存模式）
    // 在生产环境中，请使用实际的数据库连接字符串
    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        permissions_path: Some("src/permissions.yaml".to_string()),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // 获取管理员 Session
    // Session 自动从连接池获取连接，并在 drop 时自动归还
    let session = pool.get_session("admin").await?;
    println!("✓ Session 获取成功 (角色: admin)");

    // 检查权限配置
    println!("📋 权限配置:");
    println!("  - Admin role: {}", pool.config().admin_role);
    println!("  - Permissions path: {:?}", pool.config().permissions_path);

    // 创建表
    session
        .execute_raw_ddl(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL
            )",
        )
        .await?;
    println!("✓ 表创建成功");

    // 插入用户
    session
        .execute_raw("INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@example.com')")
        .await?;
    println!("✓ 用户插入成功: 1 <alice@example.com>");

    // 查询用户
    let result = session
        .execute_raw("SELECT id, name, email FROM users WHERE id = 1")
        .await?;
    println!("✓ 用户查询成功: {} 行受影响", result.rows_affected());

    // 更新用户
    session
        .execute_raw("UPDATE users SET email = 'alice_new@example.com' WHERE id = 1")
        .await?;
    println!("✓ 用户更新成功");

    // 删除用户
    session.execute_raw("DELETE FROM users WHERE id = 1").await?;
    println!("✓ 用户删除成功");

    // 删除表
    session.execute_raw_ddl("DROP TABLE users").await?;

    // 获取连接池状态
    let status = pool.status();
    println!(
        "\n📊 连接池状态: total={}, active={}, idle={}",
        status.total, status.active, status.idle
    );

    println!("\n✨ 示例运行完成！");

    Ok(())
}
