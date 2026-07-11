// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 基础 CRUD 示例
//!
//! 展示如何使用 `#[db_entity(...)]` 统一属性宏定义实体并执行 CRUD 操作：
//! - 定义 User 实体（id, name, email）
//! - 创建连接池和 Session
//! - 建表（CREATE TABLE）
//! - 插入数据（Model::insert）
//! - 查询所有（Model::find_all）
//! - 更新数据（Model::update）
//! - 删除数据（Model::delete）
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example basic_crud --features "sqlite,permission,macros"
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

// ============================================
// 定义 User 实体
// ============================================

/// 用户实体模型
///
/// 使用 `#[db_entity(table_name = "users", primary_key = "id")]` 统一属性宏获得：
/// - sea-orm 的 EntityModel 实现（Entity/ActiveModel/Column 等，由 DeriveEntityModel 生成）
/// - dbnexus 的 `table_name()` / `primary_key_column()` 辅助方法
/// - 8 个带权限检查的 CRUD 方法（insert/find_by_id/update/delete/find_all/...）
/// - `impl ActiveModelBehavior for ActiveModel`（宏自动生成，用户无需手写）
#[db_entity(table_name = "users", primary_key = "id")]
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
    println!("📝 DBNexus 基础 CRUD 示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建连接池和 Session
    // ============================================
    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("✓ 连接池和 Session 创建成功 (角色: admin)\n");

    // ============================================
    // 2. 建表（CREATE TABLE）
    // ============================================
    // execute_raw_ddl 仅限 admin 角色调用，用于 DDL 操作。
    common::ddl::create_users_table(&session).await?;
    println!("✓ users 表创建成功\n");

    // ============================================
    // 3. 插入数据（Model::insert）
    // ============================================
    println!("--- INSERT ---");
    let user1 = Model {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    let inserted1 = Model::insert(&session, user1).await?;
    println!("  ✓ 插入用户: id={}, name={}", inserted1.id, inserted1.name);

    let user2 = Model {
        id: 2,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };
    let inserted2 = Model::insert(&session, user2).await?;
    println!("  ✓ 插入用户: id={}, name={}", inserted2.id, inserted2.name);

    let user3 = Model {
        id: 3,
        name: "Charlie".to_string(),
        email: "charlie@example.com".to_string(),
    };
    let inserted3 = Model::insert(&session, user3).await?;
    println!("  ✓ 插入用户: id={}, name={}", inserted3.id, inserted3.name);

    // ============================================
    // 4. 查询所有（Model::find_all）
    // ============================================
    println!("\n--- FIND_ALL ---");
    let all_users = Model::find_all(&session).await?;
    println!("  ✓ 查询所有用户: 共 {} 条记录", all_users.len());
    for u in &all_users {
        println!("    - id={}, name={}, email={}", u.id, u.name, u.email);
    }

    // ============================================
    // 5. 更新数据（Model::update）
    // ============================================
    println!("\n--- UPDATE ---");
    let updated = Model::update(
        &session,
        Model {
            email: "alice_new@example.com".to_string(),
            ..inserted1
        },
    )
    .await?;
    println!("  ✓ 更新用户 id={}: 新邮箱={}", updated.id, updated.email);

    // ============================================
    // 6. 删除数据（Model::delete）
    // ============================================
    println!("\n--- DELETE ---");
    let affected = Model::delete(&session, inserted3.id).await?;
    println!("  ✓ 删除用户 id={}: 影响 {} 行", inserted3.id, affected);

    // ============================================
    // 验证最终状态
    // ============================================
    println!("\n--- 最终状态 ---");
    let remaining = Model::find_all(&session).await?;
    println!("  ✓ 剩余用户数: {}", remaining.len());
    for u in &remaining {
        println!("    - id={}, name={}, email={}", u.id, u.name, u.email);
    }

    println!("\n========================================");
    println!("✨ 基础 CRUD 示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - #[db_entity(table_name=\"...\", primary_key=\"...\")]  统一属性宏定义实体");
    println!("  - Model::insert(&session, model)   - 插入记录");
    println!("  - Model::find_all(&session)        - 查询所有");
    println!("  - Model::update(&session, model)   - 更新记录");
    println!("  - Model::delete(&session, id)      - 按 ID 删除");
    println!("  - session.execute_raw_ddl(sql)     - 执行 DDL（仅 admin）");

    Ok(())
}
