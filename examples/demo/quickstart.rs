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

use dbnexus::{DbConfigBuilder, DbPool, db_crud};
use sea_orm::entity::prelude::*;

// ============================================
// 定义用户实体（使用正确的宏组合）
// ============================================

// ✅ 正确：使用 DeriveEntityModel, DeriveModel, DeriveActiveModel
// ✅ 正确：添加 #[db_crud] 属性宏
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, DeriveModel, DeriveActiveModel)]
#[sea_orm(table_name = "users")]
#[db_crud] // ← 属性宏，自动生成 CRUD 方法
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
}

// ✅ 正确：impl Entity 是由 DeriveEntityModel 自动生成的
// Entity::insert, Entity::find_by_id 等方法已经可用
// 如果需要额外的自定义方法，可以在这里添加

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
    // 使用 Entity API 进行 CRUD 操作（通过 #[db_crud] 宏生成）
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
    // ✅ 正确：使用宏生成的 insert 方法
    let inserted_user = Entity::insert(&session, user).await?;
    println!("  ✓ INSERT: 插入用户 'Alice' (id={})", inserted_user.id);

    // ----------------------------------------------------
    // 查询用户 - 使用 Entity::find_by_id()
    // ----------------------------------------------------
    // ✅ 正确：使用宏生成的 find_by_id 方法
    let user_found = Entity::find_by_id(&session, inserted_user.id).await?;
    println!(
        "  ✓ SELECT: 查询用户 'Alice' (id={}, email={})",
        user_found.id, user_found.email
    );

    // ----------------------------------------------------
    // 查询所有用户 - 使用 Entity::find_all()
    // ----------------------------------------------------
    // ✅ 正确：使用宏生成的 find_all 方法
    let all_users = Entity::find_all(&session).await?;
    println!("  ✓ SELECT: 查询所有用户 (共 {} 个)", all_users.len());

    // ----------------------------------------------------
    // 更新用户 - 使用 ActiveModel::save()
    // ----------------------------------------------------
    let mut active_model: sea_orm::ActiveModel = user_found.into();
    active_model.email = sea_orm::Set("alice_new@example.com".to_string());
    let updated_user = active_model.save(&session).await?;
    println!("  ✓ UPDATE: 更新用户邮箱 (id={})", updated_user.id.unwrap());

    // ----------------------------------------------------
    // 条件查询 - 使用 Entity::find_by_condition()
    // ----------------------------------------------------
    let condition = Condition::all().add(Column::Name.eq("Alice"));
    let alice_users = Entity::find_by_condition(&session, condition).await?;
    println!("  ✓ SELECT: 条件查询用户 (name='Alice', 共 {} 个)", alice_users.len());

    // ----------------------------------------------------
    // 删除用户 - 使用 Entity::delete()
    // ----------------------------------------------------
    // ✅ 正确：使用宏生成的 delete 方法
    let deleted_count = Entity::delete(&session, inserted_user.id).await?;
    println!(
        "  ✓ DELETE: 删除用户 'Alice' (id={}, 影响行数={})",
        inserted_user.id, deleted_count
    );

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
    println!("  1. 使用 #[db_crud] 属性宏自动生成 CRUD 方法");
    println!("  2. Session 不暴露 connection，用户无法直接访问底层连接");
    println!("  3. 所有 CRUD 操作通过 Entity API 进行");
    println!("  4. 宏生成的 CRUD 方法自动包含：");
    println!("     - 权限检查 (check_table_permission)");
    println!("     - 指标收集 (record_metric) - 需要启用 metrics 特性");
    println!("     - 审计日志 (audit) - 需要启用 audit 特性");
    println!("  5. 可用的宏生成方法:");
    println!("     - Entity::insert() - 插入记录");
    println!("     - Entity::find_by_id() - 按 ID 查询");
    println!("     - Entity::find_all() - 查询所有");
    println!("     - Entity::find_by_condition() - 条件查询");
    println!("     - Entity::update() - 更新记录");
    println!("     - Entity::delete() - 按 ID 删除");
    println!("     - Entity::delete_many() - 批量删除");
    println!("     - Entity::count() - 统计数量");

    Ok(())
}

// 配置构建器辅助函数
fn db_config_builder() -> dbnexus::config::DbConfigBuilder {
    dbnexus::config::DbConfigBuilder::new()
}
