// Copyright (c) 2026 Kirky.X
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
    println!("🏷️  DBNexus db_entity permissions 子参数示例");
    println!("========================================\n");

    // ============================================
    // 1. 展示宏生成的常量
    // ============================================
    println!("--- 宏生成的常量 ---\n");
    println!("  User::ALLOWED_ROLES      = {:?}", Model::ALLOWED_ROLES);
    println!("  User::ALLOWED_OPERATIONS = {:?}", Model::ALLOWED_OPERATIONS);
    println!("  Model::allowed_roles()   = {:?}", Model::allowed_roles());
    println!("  Model::allowed_operations() = {:?}", Model::allowed_operations());

    // ============================================
    // 2. 创建 DbPool + Session
    // ============================================
    println!("\n--- DbPool + Session ---\n");

    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("✓ Session 创建成功 (角色: {})", session.role());

    // 建表
    common::ddl::create_users_table(&session).await?;
    println!("✓ users 表创建成功");

    // ============================================
    // 3. 演示不同角色的权限校验
    // ============================================
    println!("\n--- 权限校验场景 ---\n");

    // admin 角色 — 在 ALLOWED_ROLES 中
    let admin_ctx = PermissionContext::new_default_with_rate_limit("admin".to_string())
        .await
        .expect("Failed to create admin context");

    println!("场景 1: admin 角色访问 users 表");
    match Model::check_permission(&admin_ctx) {
        Ok(()) => println!("  ✓ check_permission 通过 — admin 在允许角色列表中"),
        Err(e) => println!("  ✗ check_permission 失败: {}", e),
    }

    println!("\n场景 2: admin 角色执行 SELECT 操作");
    match Model::check_operation(&admin_ctx, &PermissionAction::Select) {
        Ok(()) => println!("  ✓ check_operation(Select) 通过 — SELECT 在允许操作列表中"),
        Err(e) => println!("  ✗ check_operation(Select) 失败: {}", e),
    }

    println!("\n场景 3: admin 角色执行 INSERT 操作");
    match Model::check_operation(&admin_ctx, &PermissionAction::Insert) {
        Ok(()) => println!("  ✓ check_operation(Insert) 通过 — INSERT 在允许操作列表中"),
        Err(e) => println!("  ✗ check_operation(Insert) 失败: {}", e),
    }

    println!("\n场景 4: admin 角色执行 DELETE 操作（不在 ALLOWED_OPERATIONS 中）");
    match Model::check_operation(&admin_ctx, &PermissionAction::Delete) {
        Ok(()) => println!("  ✗ 意外: DELETE 被允许（应该在操作列表之外被拒绝）"),
        Err(e) => println!("  ✓ check_operation(Delete) 正确拒绝: {}", e),
    }

    // manager 角色 — 也在 ALLOWED_ROLES 中
    let manager_ctx = PermissionContext::new_default_with_rate_limit("manager".to_string())
        .await
        .expect("Failed to create manager context");

    println!("\n场景 5: manager 角色访问 users 表");
    match Model::check_permission(&manager_ctx) {
        Ok(()) => println!("  ✓ check_permission 通过 — manager 在允许角色列表中"),
        Err(e) => println!("  ✗ check_permission 失败: {}", e),
    }

    // guest 角色 — 不在 ALLOWED_ROLES 中
    let guest_ctx = PermissionContext::new_default_with_rate_limit("guest".to_string())
        .await
        .expect("Failed to create guest context");

    println!("\n场景 6: guest 角色访问 users 表（不在 ALLOWED_ROLES 中）");
    match Model::check_permission(&guest_ctx) {
        Ok(()) => println!("  ✗ 意外: guest 通过了 check_permission（应该被拒绝）"),
        Err(e) => println!("  ✓ check_permission 正确拒绝: {}", e),
    }

    println!("\n场景 7: guest 角色执行 SELECT 操作");
    match Model::check_operation(&guest_ctx, &PermissionAction::Select) {
        Ok(()) => println!("  ✗ 意外: guest 的 SELECT 操作被允许"),
        Err(e) => println!("  ✓ check_operation(Select) 正确拒绝: {}", e),
    }

    // ============================================
    // 4. 总结
    // ============================================
    println!("\n========================================");
    println!("✨ db_entity permissions 子参数示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - #[db_entity(..., permissions(roles=[...], operations=[...]))]  声明实体访问控制");
    println!("  - Model::ALLOWED_ROLES       编译期生成的角色白名单常量");
    println!("  - Model::ALLOWED_OPERATIONS  编译期生成的操作白名单常量");
    println!("  - Model::check_permission(&ctx)         校验角色是否允许访问实体");
    println!("  - Model::check_operation(&ctx, &op)     校验角色+操作是否允许");
    println!("  - PermissionContext::new_default_with_rate_limit(role)  创建权限上下文");
    println!("\n⚠️  注意: permissions 子参数在编译期验证角色名格式，");
    println!("   无效角色名（如以数字开头、含连字符）会导致编译失败。");

    Ok(())
}
