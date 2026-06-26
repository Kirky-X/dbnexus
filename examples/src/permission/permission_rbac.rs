// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! RBAC 权限控制示例
//!
//! 演示 `MemoryPermissionProvider` 的使用，包括：
//! - 创建 admin / manager / guest 三种角色策略
//! - 定义表级权限（SELECT / INSERT / UPDATE / DELETE）
//! - 权限检查通过和拒绝场景
//! - 结合 `DbPool + Session` 与 `db_permission` 宏进行实体级权限校验
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example permission_rbac --features "sqlite,permission,macros"
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::access::permission::{
    MemoryPermissionProvider, PermissionAction, PermissionContext, PermissionProvider, RolePolicy, TablePermission,
};
use dbnexus::{DbEntity, db_permission};
use sea_orm::entity::prelude::*;

// ============================================
// 定义 User 实体（带 db_permission 注解）
// ============================================

/// 用户实体
///
/// `#[db_permission]` 在编译期生成 `ALLOWED_ROLES` / `ALLOWED_OPERATIONS` 常量，
/// 以及 `check_permission` / `check_operation` 方法，用于运行时权限校验。
#[derive(Clone, Debug, PartialEq, DbEntity, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
#[db_permission(roles = ["admin", "manager"], operations = ["SELECT", "INSERT", "UPDATE"])]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔐 DBNexus RBAC 权限控制示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建 MemoryPermissionProvider 并配置角色策略
    // ============================================
    let provider = MemoryPermissionProvider::new();

    // admin: 通配符表，拥有全部操作权限
    provider
        .add_role(
            "admin",
            RolePolicy {
                tables: vec![TablePermission {
                    name: "*".to_string(),
                    operations: vec![
                        PermissionAction::Select,
                        PermissionAction::Insert,
                        PermissionAction::Update,
                        PermissionAction::Delete,
                    ],
                }],
            },
        )
        .await;

    // manager: 仅 users 和 orders 表，无 DELETE 权限
    provider
        .add_role(
            "manager",
            RolePolicy {
                tables: vec![
                    TablePermission {
                        name: "users".to_string(),
                        operations: vec![
                            PermissionAction::Select,
                            PermissionAction::Insert,
                            PermissionAction::Update,
                        ],
                    },
                    TablePermission {
                        name: "orders".to_string(),
                        operations: vec![
                            PermissionAction::Select,
                            PermissionAction::Insert,
                            PermissionAction::Update,
                        ],
                    },
                ],
            },
        )
        .await;

    // guest: 仅 users 表 SELECT
    provider
        .add_role(
            "guest",
            RolePolicy {
                tables: vec![TablePermission {
                    name: "users".to_string(),
                    operations: vec![PermissionAction::Select],
                }],
            },
        )
        .await;

    println!("✓ 已配置 3 个角色: admin / manager / guest\n");

    // ============================================
    // 2. 演示权限检查通过和拒绝场景
    // ============================================
    println!("--- MemoryPermissionProvider 权限检查 ---\n");

    let test_cases = [
        ("admin", "users", PermissionAction::Select, true),
        ("admin", "users", PermissionAction::Delete, true),
        ("admin", "orders", PermissionAction::Insert, true),
        ("manager", "users", PermissionAction::Select, true),
        ("manager", "users", PermissionAction::Update, true),
        ("manager", "users", PermissionAction::Delete, false),
        ("manager", "logs", PermissionAction::Select, false),
        ("guest", "users", PermissionAction::Select, true),
        ("guest", "users", PermissionAction::Insert, false),
        ("guest", "orders", PermissionAction::Select, false),
        ("unknown", "users", PermissionAction::Select, false),
    ];

    for (role, table, action, expected) in test_cases {
        let result = provider.check_access(role, table, action.clone())?;
        let status = if result { "✓ 允许" } else { "✗ 拒绝" };
        let mark = if result == expected { "✔" } else { "✘" };
        println!(
            "  {} [{}] {}.{} 期望={} 实际={}",
            mark, role, table, action, expected, result
        );
        println!("     {}", status);
    }

    // ============================================
    // 3. 创建 DbPool + Session
    // ============================================
    println!("\n--- DbPool + Session 集成 ---\n");

    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("✓ 连接池和 Session 创建成功 (角色: admin)");

    // 建表
    common::ddl::create_users_table(&session).await?;
    println!("✓ users 表创建成功");

    // ============================================
    // 4. 使用 db_permission 宏生成的权限校验
    // ============================================
    println!("\n--- db_permission 宏权限校验 ---\n");

    // 宏生成的常量
    println!("  User 实体允许的角色: {:?}", Model::allowed_roles());
    println!("  User 实体允许的操作: {:?}", Model::allowed_operations());

    // 为 admin 角色创建 PermissionContext 并校验
    let admin_ctx = PermissionContext::new_default_with_rate_limit("admin".to_string())
        .await
        .expect("Failed to create admin context");
    match Model::check_permission(&admin_ctx) {
        Ok(()) => println!("  ✓ admin 角色通过 check_permission 校验"),
        Err(e) => println!("  ✗ admin 角色校验失败: {}", e),
    }

    // 校验 admin 的 SELECT 操作
    match Model::check_operation(&admin_ctx, &PermissionAction::Select) {
        Ok(()) => println!("  ✓ admin 角色通过 check_operation(Select) 校验"),
        Err(e) => println!("  ✗ admin 角色操作校验失败: {}", e),
    }

    // 校验 admin 的 DELETE 操作（不在 ALLOWED_OPERATIONS 中）
    match Model::check_operation(&admin_ctx, &PermissionAction::Delete) {
        Ok(()) => println!("  ✗ 意外: admin 的 Delete 操作被允许"),
        Err(e) => println!("  ✓ admin 的 Delete 操作被正确拒绝: {}", e),
    }

    // 为 guest 角色创建 PermissionContext（不在 ALLOWED_ROLES 中）
    let guest_ctx = PermissionContext::new_default_with_rate_limit("guest".to_string())
        .await
        .expect("Failed to create guest context");
    match Model::check_permission(&guest_ctx) {
        Ok(()) => println!("  ✗ 意外: guest 角色通过了 check_permission 校验"),
        Err(e) => println!("  ✓ guest 角色被正确拒绝: {}", e),
    }

    println!("\n========================================");
    println!("✨ RBAC 权限控制示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - MemoryPermissionProvider::new()   创建内存权限提供者");
    println!("  - provider.add_role(role, policy)  添加角色策略（async）");
    println!("  - provider.check_access(role, table, op)  检查表级权限");
    println!("  - #[db_permission(roles=[...], operations=[...])]  编译期生成权限校验方法");
    println!("  - Model::check_permission(&ctx)    校验角色是否允许访问实体");
    println!("  - Model::check_operation(&ctx, &op) 校验角色+操作是否允许");

    Ok(())
}
