// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限控制示例
//!
//! 展示如何使用 dbnexus 的权限系统：
//! - 使用 Session 执行权限检查
//! - 测试不同角色的访问权限
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example permissions --features sqlite,permission
//! ```

use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 DBNexus 权限控制示例\n");
    println!("========================================");

    // 初始化连接池（带权限配置）
    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        permissions_path: Some("src/permissions.yaml".to_string()),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功\n");

    // 创建表
    let session = pool.get_session("admin").await?;
    session
        .execute_raw_ddl(
            "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            role TEXT NOT NULL
        )",
        )
        .await?;
    session
        .execute_raw_ddl(
            "CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            amount REAL NOT NULL,
            status TEXT NOT NULL
        )",
        )
        .await?;
    println!("✓ 表创建成功\n");

    // 测试 admin 角色（所有权限）
    println!("👤 测试 admin 角色");
    println!("------------------------------------------");
    test_admin_role(&pool).await?;

    // 测试 manager 角色（可以访问 Users，不能访问 Orders）
    println!("\n👤 测试 manager 角色");
    println!("------------------------------------------");
    test_manager_role(&pool).await?;

    // 测试 orders_manager 角色（只能访问 Orders）
    println!("\n👤 测试 orders_manager 角色");
    println!("------------------------------------------");
    test_orders_manager_role(&pool).await?;

    println!("\n========================================");
    println!("✨ 所有权限测试完成！");

    Ok(())
}

async fn test_admin_role(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let session = pool.get_session("admin").await?;
    println!("  ✓ 获取 admin session");

    // admin 可以访问 Users
    let _ = session.execute_raw("SELECT * FROM users").await?;
    println!("  ✓ admin 可以查询 Users");

    let _ = session
        .execute_raw("INSERT INTO users (id, name, email, role) VALUES (1, 'Admin User', 'admin@example.com', 'admin')")
        .await?;
    println!("  ✓ admin 可以插入 Users");

    // admin 也可以访问 Orders
    let _ = session.execute_raw("SELECT * FROM orders").await?;
    println!("  ✓ admin 可以查询 Orders");

    let _ = session
        .execute_raw("INSERT INTO orders (id, user_id, amount, status) VALUES (1, 1, 99.99, 'pending')")
        .await?;
    println!("  ✓ admin 可以插入 Orders");

    Ok(())
}

async fn test_manager_role(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let session = pool.get_session("manager").await?;
    println!("  ✓ 获取 manager session");

    // manager 可以访问 Users
    let result = session.execute_raw("SELECT * FROM users").await?;
    println!("  ✓ manager 可以查询 Users (找到 {} 条记录)", result.rows_affected());

    // manager 可以插入 Users
    let _ = session
        .execute_raw(
            "INSERT INTO users (id, name, email, role) VALUES (2, 'Manager User', 'manager@example.com', 'manager')",
        )
        .await?;
    println!("  ✓ manager 可以插入 Users");

    // manager 尝试访问 Orders（应该被拒绝）
    let result = session.execute_raw("SELECT * FROM orders").await;
    match result {
        Ok(_) => println!("  ✗ manager 不应该能访问 Orders!"),
        Err(e) => println!("  ✓ manager 被拒绝访问 Orders: {}", e),
    }

    Ok(())
}

async fn test_orders_manager_role(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let session = pool.get_session("orders_manager").await?;
    println!("  ✓ 获取 orders_manager session");

    // orders_manager 尝试访问 Users（应该被拒绝）
    let result = session.execute_raw("SELECT * FROM users").await;
    match result {
        Ok(_) => println!("  ✗ orders_manager 不应该能访问 Users!"),
        Err(e) => println!("  ✓ orders_manager 被拒绝访问 Users: {}", e),
    }

    // orders_manager 可以访问 Orders
    let result = session.execute_raw("SELECT * FROM orders").await?;
    println!(
        "  ✓ orders_manager 可以查询 Orders (找到 {} 条记录)",
        result.rows_affected()
    );

    // orders_manager 可以插入 Orders
    let _ = session
        .execute_raw("INSERT INTO orders (id, user_id, amount, status) VALUES (2, 2, 149.99, 'processing')")
        .await?;
    println!("  ✓ orders_manager 可以插入 Orders");

    Ok(())
}
