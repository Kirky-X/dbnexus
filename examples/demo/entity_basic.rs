// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 实体操作完整示例
//!
//! 展示 dbnexus 中实体操作的正确用法，包括：
//! - 使用 Sea-ORM 宏定义 Entity
//! - 使用 Condition 构建查询条件
//! - 使用 Entity 的 CRUD 方法进行数据库操作
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example entity_basic --features sqlite
//!

use dbnexus::entity::{ActiveModelTrait, Condition, EntityTrait, Set};
use dbnexus::{DbConfig, DbPool};
use sea_orm::ActiveValue;
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
    pub age: Option<i32>,
    pub status: String,
}

impl Entity for Entity {}

// 为 Entity 添加 CRUD 方法
db_crud!(Entity);

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化连接池
    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .admin_role("admin")
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // 获取 Session
    let session = pool.get_session("admin").await?;
    println!("✓ Session 获取成功");

    // 创建测试表
    session
        .execute_raw_ddl(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                email TEXT NOT NULL,
                age INTEGER,
                status TEXT DEFAULT 'active'
            )",
        )
        .await?;
    println!("✓ 测试表创建成功");

    // ============================================
    // 1. Condition 查询条件构建
    // ============================================
    println!("\n📋 Condition 查询条件示例:");

    // Condition::all() - 匹配所有记录
    let all_condition = Condition::all();
    println!("  - Condition::all() 用于匹配所有记录");

    // Condition::any() - 匹配任何条件
    let any_condition = Condition::any();
    println!("  - Condition::any() 用于匹配任何条件");

    // Condition::not() - 条件取反
    let not_condition = Condition::not(Condition::all());
    println!("  - Condition::not() 用于条件取反");

    // Condition::contains() - 包含查询
    let contains_condition = Condition::contains("name", "John");
    println!("  - Condition::contains() 用于模糊查询");

    // Condition 组合
    let combined = Condition::all()
        .add(Condition::contains("email", "@example.com"))
        .add(Condition::not(Condition::any()));
    println!("  - 条件组合使用 .add() 方法");

    // ============================================
    // 2. Set 更新数据构建
    // ============================================
    println!("\n📋 Set 更新数据示例:");

    // 字符串类型更新
    let name_set = Set("Alice".to_string());
    println!("  - Set 用于字符串: Set(String)");

    // 数字类型更新
    let age_set = Set(Some(25));
    println!("  - Set 用于数字: Set(Option<i32>)");

    // ============================================
    // 3. 实际 CRUD 操作（使用 Entity API）
    // ============================================
    println!("\n📋 实际 CRUD 操作示例（使用 Entity API）:");

    // ----------------------------------------------------
    // 3.1 插入数据 - 使用 Entity::insert()
    // ----------------------------------------------------
    let user = Model {
        id: 0, // 主键由数据库自动生成
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        age: Some(25),
        status: "active".to_string(),
    };
    let inserted_user = Entity::insert(&session, user).await?;
    println!("  ✓ INSERT: 插入用户 'Alice' (id={})", inserted_user.id);

    // ----------------------------------------------------
    // 3.2 查询数据 - 使用 Entity::find_by_id()
    // ----------------------------------------------------
    let found_user = Entity::find_by_id(&session, inserted_user.id).await?;
    println!("  ✓ SELECT: 查询用户 'Alice' (id={})", found_user.id);

    // ----------------------------------------------------
    // 3.3 更新数据 - 使用 ActiveModel 和 save()
    // ----------------------------------------------------
    let mut active_model: ActiveModel = inserted_user.into();
    active_model.age = Set(Some(26));
    let updated_user = active_model.save(&session).await?;
    println!("  ✓ UPDATE: 更新用户年龄为 26 (id={})", updated_user.id.unwrap());

    // ----------------------------------------------------
    // 3.4 删除数据 - 使用 ActiveModel::delete()
    // ----------------------------------------------------
    let deleted_user = Entity::find_by_id(&session, inserted_user.id).await?.unwrap();
    let _: ActiveModel = deleted_user.clone().delete(&session).await?;
    println!("  ✓ DELETE: 删除用户 'Alice' (id={})", deleted_user.id);

    // ============================================
    // 4. 批量操作示例
    // ============================================
    println!("\n📋 批量操作示例:");

    // 插入多个用户
    let users = vec![
        Model {
            id: 0,
            name: "Bob".to_string(),
            email: "bob@example.com".to_string(),
            age: Some(30),
            status: "active".to_string(),
        },
        Model {
            id: 0,
            name: "Charlie".to_string(),
            email: "charlie@example.com".to_string(),
            age: Some(35),
            status: "inactive".to_string(),
        },
    ];

    for user in users {
        Entity::insert(&session, user).await?;
        println!("  ✓ 插入用户成功");
    }

    // 使用 Condition 查询
    println!("\n📋 使用 Condition 进行复杂查询:");
    let active_users = Entity::find()
        .filter(Condition::all().add(Condition::eq("status", "active")))
        .all(&session)
        .await?;
    println!("  - 活跃用户数量: {}", active_users.len());

    // ============================================
    // 5. 事务中的实体操作
    // ============================================
    println!("\n📋 事务中的实体操作示例:");

    session.begin_transaction().await?;
    println!("  ✓ BEGIN: 开始事务");

    // 在事务中执行多个操作
    let user_in_txn = Model {
        id: 0,
        name: "Dave".to_string(),
        email: "dave@example.com".to_string(),
        age: Some(40),
        status: "pending".to_string(),
    };
    let inserted_dave = Entity::insert(&session, user_in_txn).await?;
    println!("  ✓ INSERT: 在事务中插入 'Dave' (id={})", inserted_dave.id);

    let mut dave_model = Entity::find_by_id(&session, inserted_dave.id).await?.unwrap();
    dave_model.status = "active".to_string();
    let dave_active: ActiveModel = dave_model.into();
    dave_active.save(&session).await?;
    println!("  ✓ UPDATE: 在事务中更新 'Dave'");

    session.commit().await?;
    println!("  ✓ COMMIT: 提交事务");

    // 验证数据
    let dave_verified = Entity::find_by_id(&session, inserted_dave.id).await?;
    if let Some(dave) = dave_verified {
        println!("  ✓ 验证: Dave 的状态已更新为 {}", dave.status);
    }

    // ============================================
    // 6. 清理
    // ============================================
    session.execute_raw_ddl("DROP TABLE users").await?;
    println!("\n✓ 清理完成");

    println!("\n🎉 实体操作示例完成！");
    println!("\n📚 进一步学习:");
    println!("  - 查看 quickstart.rs 了解完整的 CRUD 流程");
    println!("  - 查看 transactions.rs 了解事务管理");
    println!("  - 查看 permissions.rs 了解权限控制");

    Ok(())
}

// 配置构建器辅助函数
fn db_config_builder() -> dbnexus::config::DbConfigBuilder {
    dbnexus::config::DbConfigBuilder::new()
}
