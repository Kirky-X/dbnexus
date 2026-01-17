// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限控制示例
//!
//! 展示如何使用 dbnexus 的权限系统：
//! - 定义带权限控制的 Entity
//! - 使用 Session 执行权限检查
//! - 测试不同角色的访问权限
//!
//! # 编译时角色验证
//!
//! 通过 `config` 属性启用编译时角色验证：
//! ```rust,ignore
//! #[db_permission(
//!     roles = ["admin", "manager"],
//!     operations = ["SELECT", "INSERT", "UPDATE"],
//!     config = "permissions.yaml"  // 编译时验证角色是否在配置中定义
//! )]
//! ```
//!
//! 如果配置文件 `permissions.yaml` 中没有定义声明的角色，编译将失败。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example permissions --features sqlite
//! ```

use dbnexus::{DbPool, DbEntity, db_crud, db_permission};

// 定义带权限控制的 User Entity
//
// #[db_permission] 声明允许访问此实体的角色和操作
// - roles: 允许访问的角色列表
// - operations: 允许的操作列表（可选，不指定则允许所有操作）
// - config: 可选，指定权限配置文件路径，启用编译时角色验证
//
// 注意：编译时验证需要配置文件存在于项目根目录
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]
#[db_permission(roles = ["admin", "manager"], operations = ["SELECT", "INSERT", "UPDATE"])]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
    role: String,
}

// 定义 Orders Entity，只有 admin 和 orders_manager 角色可以访问
#[derive(DbEntity)]
#[db_entity]
#[table_name = "orders")]
#[db_crud]
#[db_permission(roles = ["admin", "orders_manager"], operations = ["SELECT", "INSERT", "UPDATE", "DELETE"])]
struct Order {
    #[primary_key]
    id: i64,
    user_id: i64,
    amount: f64,
    status: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 DBNexus 权限控制示例\n");
    println!("========================================");

    // 初始化连接池
    let pool = DbPool::new("sqlite::memory:").await?;
    println!("✓ 连接池创建成功\n");

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
    let _ = User::find_all(&session).await?;
    println!("  ✓ admin 可以查询 Users");

    let _ = User::insert(&session, User {
        id: 1,
        name: "Admin User".to_string(),
        email: "admin@example.com".to_string(),
        role: "admin".to_string(),
    }).await?;
    println!("  ✓ admin 可以插入 Users");

    // admin 也可以访问 Orders
    let _ = Order::find_all(&session).await?;
    println!("  ✓ admin 可以查询 Orders");

    let _ = Order::insert(&session, Order {
        id: 1,
        user_id: 1,
        amount: 99.99,
        status: "pending".to_string(),
    }).await?;
    println!("  ✓ admin 可以插入 Orders");

    Ok(())
}

async fn test_manager_role(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let session = pool.get_session("manager").await?;
    println!("  ✓ 获取 manager session");

    // manager 可以访问 Users
    let users = User::find_all(&session).await?;
    println!("  ✓ manager 可以查询 Users (找到 {} 条记录)", users.len());

    // manager 可以插入 Users
    let _ = User::insert(&session, User {
        id: 2,
        name: "Manager User".to_string(),
        email: "manager@example.com".to_string(),
        role: "manager".to_string(),
    }).await?;
    println!("  ✓ manager 可以插入 Users");

    // manager 尝试访问 Orders（应该被拒绝）
    let result = Order::find_all(&session).await;
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
    let result = User::find_all(&session).await;
    match result {
        Ok(_) => println!("  ✗ orders_manager 不应该能访问 Users!"),
        Err(e) => println!("  ✓ orders_manager 被拒绝访问 Users: {}", e),
    }

    // orders_manager 可以访问 Orders
    let orders = Order::find_all(&session).await?;
    println!("  ✓ orders_manager 可以查询 Orders (找到 {} 条记录)", orders.len());

    // orders_manager 可以插入 Orders
    let _ = Order::insert(&session, Order {
        id: 2,
        user_id: 2,
        amount: 149.99,
        status: "processing".to_string(),
    }).await?;
    println!("  ✓ orders_manager 可以插入 Orders");

    Ok(())
}
