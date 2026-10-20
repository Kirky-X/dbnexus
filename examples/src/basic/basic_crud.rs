// Copyright (c) 2025-2026 Kirky.X🌠
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

use dbnexus::sea_orm::entity::prelude::*;
use dbnexus::{db_entity, t_simple};

// 定义 User 实体

/// 用户实体模型
///
/// 使用 `#[db_entity(table_name = "users", primary_key = "id")]` 统一属性宏获得：
/// - sea-orm 的 EntityModel 实现（Entity/ActiveModel/Column 等，由 DeriveEntityModel 生成）
/// - dbnexus 的 `table_name()` / `primary_key_column()` 辅助方法
/// - 一组带权限检查的 CRUD 方法（insert/find/update/delete 及批量、分页、存在性等 14+ 个）
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

// 主函数

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("{}", t_simple("example-basic-crud-title"));
    println!("========================================\n");

    // 创建连接池和 Session
    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("✓ pool and Session created (role: admin)\n");

    // 建表（CREATE TABLE）
    // execute_raw_ddl 仅限 admin 角色调用，用于 DDL 操作。
    common::ddl::create_users_table(&session).await?;
    println!("✓ users table created\n");

    // 插入数据（Model::insert）
    println!("--- INSERT ---");
    let user1 = Model {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    let inserted1 = Model::insert(&session, user1).await?;
    println!(
        "  ✓ inserted user: id={}, name={}",
        inserted1.id, inserted1.name
    );

    let user2 = Model {
        id: 2,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };
    let inserted2 = Model::insert(&session, user2).await?;
    println!(
        "  ✓ inserted user: id={}, name={}",
        inserted2.id, inserted2.name
    );

    let user3 = Model {
        id: 3,
        name: "Charlie".to_string(),
        email: "charlie@example.com".to_string(),
    };
    let inserted3 = Model::insert(&session, user3).await?;
    println!(
        "  ✓ inserted user: id={}, name={}",
        inserted3.id, inserted3.name
    );

    // 查询所有（Model::find_all）
    println!("\n--- FIND_ALL ---");
    let all_users = Model::find_all(&session).await?;
    println!(
        "  ✓ queried all users: {} records in total",
        all_users.len()
    );
    for u in &all_users {
        println!("    - id={}, name={}, email={}", u.id, u.name, u.email);
    }

    // 更新数据（Model::update）
    println!("\n--- UPDATE ---");
    let updated = Model::update(
        &session,
        Model {
            email: "alice_new@example.com".to_string(),
            ..inserted1
        },
    )
    .await?;
    println!(
        "  ✓ updated user id={}: new email={}",
        updated.id, updated.email
    );

    // 删除数据（Model::delete）
    println!("\n--- DELETE ---");
    let affected = Model::delete(&session, inserted3.id).await?;
    println!(
        "  ✓ deleted user id={}: {} rows affected",
        inserted3.id, affected
    );

    // 验证最终状态
    println!("\n--- final state ---");
    let remaining = Model::find_all(&session).await?;
    println!("  ✓ remaining users: {}", remaining.len());
    for u in &remaining {
        println!("    - id={}, name={}, email={}", u.id, u.name, u.email);
    }

    println!("\n========================================");
    println!("✨ Basic CRUD example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - #[db_entity(table_name=\"...\", primary_key=\"...\")]  unified attribute macro");
    println!("  - Model::insert(&session, model)   - insert a record");
    println!("  - Model::find_all(&session)        - query all");
    println!("  - Model::update(&session, model)   - update a record");
    println!("  - Model::delete(&session, id)      - delete by ID");
    println!("  - session.execute_raw_ddl(sql)     - execute DDL (admin only)");

    Ok(())
}
