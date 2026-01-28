// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 快速开始示例
//!
//! 展示 dbnexus 的基本使用方法，包括：
//! - 定义 Entity 并自动生成 CRUD 方法
//! - 创建数据库连接池
//! - 通过 Entity API 执行数据库操作（不暴露 connection）
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example quickstart --features sqlite
//!

use dbnexus::{DbConfig, DbPool};
use sea_orm::entity::prelude::*;

// ============================================
// 定义用户实体（使用 Sea-ORM 宏）
// ============================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, DeriveModel, DeriveActiveModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

impl Entity for Entity {}

// 为 Entity 添加 CRUD 方法（自动生成 insert, find_by_id, update, delete 等）
db_crud!(Entity);

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化连接池（使用 SQLite 内存模式）
    // 在生产环境中，请使用实际的数据库连接字符串
    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .permissions_path("src/permissions.yaml")
        .admin_role("admin")
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // 获取管理员 Session
    // Session 自动从连接池获取连接，并在 drop 时自动归还
    let session = pool.get_session("admin").await?;
    println!("✓ Session 获取成功 (角色: admin)");

    // 检查权限配置
    println!("📋 权限配置:");
    println!("  - Admin role: {}", pool.config().admin_role());
    println!("  - Permissions path: {:?}", pool.config().permissions_path());

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

    // ============================================
    // 使用 Entity API 进行 CRUD 操作
    // ============================================
    println!("\n📋 使用 Entity API 进行 CRUD 操作:");

    // ----------------------------------------------------
    // 插入用户 - 使用 Entity::insert()
    // ----------------------------------------------------
    let user = Model {
        id: 0, // 主键由数据库自动生成
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    let inserted_user = Entity::insert(&session, user).await?;
    println!("  ✓ INSERT: 插入用户 'Alice' (id={})", inserted_user.id);

    // ----------------------------------------------------
    // 查询用户 - 使用 Entity::find_by_id()
    // ----------------------------------------------------
    let user_found = Entity::find_by_id(&session, inserted_user.id).await?;
    println!(
        "  ✓ SELECT: 查询用户 'Alice' (id={}, email={})",
        user_found.id, user_found.email
    );

    // ----------------------------------------------------
    // 更新用户 - 使用 ActiveModel::save()
    // ----------------------------------------------------
    let mut active_model: sea_orm::ActiveModel = user_found.into();
    active_model.email = sea_orm::Set("alice_new@example.com".to_string());
    let updated_user = active_model.save(&session).await?;
    println!("  ✓ UPDATE: 更新用户邮箱 (id={})", updated_user.id.unwrap());

    // ----------------------------------------------------
    // 删除用户 - 使用 ActiveModel::delete()
    // ----------------------------------------------------
    let user_to_delete = Entity::find_by_id(&session, inserted_user.id).await?.unwrap();
    let _: sea_orm::ActiveModel = user_to_delete.delete(&session).await?;
    println!("  ✓ DELETE: 删除用户 'Alice' (id={})", user_to_delete.id);

    // ============================================
    // 使用 execute_raw_ddl 进行 DDL 操作（这是合理的使用场景）
    // ============================================
    session.execute_raw_ddl("DROP TABLE users").await?;
    println!("\n✓ DDL 操作完成（删除表）");

    // 获取连接池状态
    let status = pool.status();
    println!(
        "\n📊 连接池状态: total={}, active={}, idle={}",
        status.total, status.active, status.idle
    );

    println!("\n✨ 示例运行完成！");
    println!("\n📚 关键点:");
    println!("  1. Session 不暴露 connection，用户无法直接访问底层连接");
    println!("  2. 所有 CRUD 操作通过 Entity API 进行");
    println!("  3. execute_raw_ddl 用于 DDL 操作（合理的使用场景）");
    println!("  4. 权限控制和指标收集由宏自动处理");

    Ok(())
}

// 配置构建器辅助函数
fn db_config_builder() -> dbnexus::config::DbConfigBuilder {
    dbnexus::config::DbConfigBuilder::new()
}
