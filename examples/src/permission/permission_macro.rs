// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! db_entity 宏 permissions 子参数示例
//!
//! 演示 `#[db_entity(..., permissions(roles = [...], operations = [...]))]` 的使用，包括：
//! - 定义带 permissions 注解的实体
//! - 展示宏如何自动生成 `ALLOWED_ROLES` / `ALLOWED_OPERATIONS` 常量
//! - 使用 `check_permission` / `check_operation` 方法进行运行时权限校验
//! - 演示不同角色的访问控制（通过 / 拒绝场景）
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example permission_macro --features "sqlite,permission,macros"
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::access::{PermissionAction, PermissionContext};
use dbnexus::db_entity;
use dbnexus::sea_orm::entity::prelude::*;

// ============================================
// 定义带 permissions 注解的实体
// ============================================

/// 用户实体 — 仅 admin 和 manager 角色可访问，允许 SELECT/INSERT/UPDATE
#[db_entity(
    table_name = "users",
    primary_key = "id",
    permissions(roles = ["admin", "manager"], operations = ["SELECT", "INSERT", "UPDATE"])
)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🏷️  DBNexus db_entity permissions sub-parameter example");
    println!("========================================\n");

    // ============================================
    // 展示宏生成的常量
    // ============================================
    println!("--- macro-generated constants ---\n");
    println!("  User::ALLOWED_ROLES      = {:?}", Model::ALLOWED_ROLES);
    println!(
        "  User::ALLOWED_OPERATIONS = {:?}",
        Model::ALLOWED_OPERATIONS
    );
    println!("  Model::allowed_roles()   = {:?}", Model::allowed_roles());
    println!(
        "  Model::allowed_operations() = {:?}",
        Model::allowed_operations()
    );

    // ============================================
    // 创建 DbPool + Session
    // ============================================
    println!("\n--- DbPool + Session ---\n");

    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("✓ Session created (role: {})", session.role());

    // 建表
    common::ddl::create_users_table(&session).await?;
    println!("✓ users table created");

    // ============================================
    // 演示不同角色的权限校验
    // ============================================
    println!("\n--- permission check scenarios ---\n");

    // admin 角色 — 在 ALLOWED_ROLES 中
    let admin_ctx = PermissionContext::new_default_with_rate_limit("admin".to_string())
        .await
        .expect("Failed to create admin context");

    println!("Scenario 1: admin role accesses the users table");
    match Model::check_permission(&admin_ctx) {
        Ok(()) => println!("  ✓ check_permission passed — admin is in the allowed role list"),
        Err(e) => println!("  ✗ check_permission failed: {}", e),
    }

    println!("\nScenario 2: admin role executes SELECT");
    match Model::check_operation(&admin_ctx, &PermissionAction::Select) {
        Ok(()) => {
            println!("  ✓ check_operation(Select) passed — SELECT is in the allowed operation list")
        }
        Err(e) => println!("  ✗ check_operation(Select) failed: {}", e),
    }

    println!("\nScenario 3: admin role executes INSERT");
    match Model::check_operation(&admin_ctx, &PermissionAction::Insert) {
        Ok(()) => {
            println!("  ✓ check_operation(Insert) passed — INSERT is in the allowed operation list")
        }
        Err(e) => println!("  ✗ check_operation(Insert) failed: {}", e),
    }

    println!("\nScenario 4: admin role executes DELETE (not in ALLOWED_OPERATIONS)");
    match Model::check_operation(&admin_ctx, &PermissionAction::Delete) {
        Ok(()) => println!(
            "  ✗ unexpected: DELETE was allowed (it should be rejected outside the operation list)"
        ),
        Err(e) => println!("  ✓ check_operation(Delete) correctly rejected: {}", e),
    }

    // manager 角色 — 也在 ALLOWED_ROLES 中
    let manager_ctx = PermissionContext::new_default_with_rate_limit("manager".to_string())
        .await
        .expect("Failed to create manager context");

    println!("\nScenario 5: manager role accesses the users table");
    match Model::check_permission(&manager_ctx) {
        Ok(()) => println!("  ✓ check_permission passed — manager is in the allowed role list"),
        Err(e) => println!("  ✗ check_permission failed: {}", e),
    }

    // guest 角色 — 不在 ALLOWED_ROLES 中
    let guest_ctx = PermissionContext::new_default_with_rate_limit("guest".to_string())
        .await
        .expect("Failed to create guest context");

    println!("\nScenario 6: guest role accesses the users table (not in ALLOWED_ROLES)");
    match Model::check_permission(&guest_ctx) {
        Ok(()) => println!("  ✗ unexpected: guest passed check_permission (it should be rejected)"),
        Err(e) => println!("  ✓ check_permission correctly rejected: {}", e),
    }

    println!("\nScenario 7: guest role executes SELECT");
    match Model::check_operation(&guest_ctx, &PermissionAction::Select) {
        Ok(()) => println!("  ✗ unexpected: guest's SELECT operation was allowed"),
        Err(e) => println!("  ✓ check_operation(Select) correctly rejected: {}", e),
    }

    // ============================================
    // 总结
    // ============================================
    println!("\n========================================");
    println!("✨ db_entity permissions sub-parameter example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!(
        "  - #[db_entity(..., permissions(roles=[...], operations=[...]))]  declare entity access control"
    );
    println!("  - Model::ALLOWED_ROLES       the compile-time generated role allowlist constant");
    println!(
        "  - Model::ALLOWED_OPERATIONS  the compile-time generated operation allowlist constant"
    );
    println!(
        "  - Model::check_permission(&ctx)         verify the role is allowed to access the entity"
    );
    println!("  - Model::check_operation(&ctx, &op)     verify the role + operation are allowed");
    println!(
        "  - PermissionContext::new_default_with_rate_limit(role)  create a permission context"
    );
    println!(
        "\n⚠️  Note: the permissions sub-parameter validates role name formats at compile time,"
    );
    println!(
        "   an invalid role name (e.g. starting with a digit or containing a hyphen) fails compilation."
    );

    Ok(())
}
