// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 实体操作基础示例
//!
//! 展示如何使用 DbEntity 和 db_crud 宏定义实体并执行基本 CRUD 操作：
//! - 定义 User 实体（DbEntity + db_crud）
//! - 执行 insert / find_by_id / update / delete / find_all
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example entity_basic --features sqlite,macros
//! ```

use dbnexus::{DbConfig, DbEntity, DbPool, db_crud};
use sea_orm::entity::prelude::*;

// ============================================
// 定义用户实体
// ============================================

#[derive(Clone, Debug, PartialEq, DbEntity, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
#[db_crud(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub username: String,
    pub email: String,
    pub active: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// 运行实体 CRUD 示例
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run().await
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("📦 实体操作基础示例");
    println!("========================================\n");

    // 初始化连接池
    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    let session = pool.get_session("admin").await?;

    // 创建表
    session
        .execute_raw_ddl(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                email TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1
            )",
        )
        .await?;
    println!("✓ users 表创建成功\n");

    // ============================================
    // 1. INSERT - 插入记录
    // ============================================
    println!("--- INSERT ---");
    let user = Model {
        id: 0, // 自增主键
        username: "alice".to_string(),
        email: "alice@example.com".to_string(),
        active: true,
    };
    let inserted = Model::insert(&session, user).await?;
    println!("  ✓ 插入用户: id={}, username=alice", inserted.id);

    let user2 = Model {
        id: 0,
        username: "bob".to_string(),
        email: "bob@example.com".to_string(),
        active: true,
    };
    let inserted2 = Model::insert(&session, user2).await?;
    println!("  ✓ 插入用户: id={}, username=bob\n", inserted2.id);

    // ============================================
    // 2. FIND_BY_ID - 按 ID 查询
    // ============================================
    println!("--- FIND_BY_ID ---");
    let found = Model::find_by_id(&session, inserted.id).await?.expect("用户不存在");
    println!(
        "  ✓ 查询用户 id={}: username={}, email={}, active={}\n",
        found.id, found.username, found.email, found.active
    );

    // ============================================
    // 3. FIND_ALL - 查询所有
    // ============================================
    println!("--- FIND_ALL ---");
    let all = Model::find_all(&session).await?;
    println!("  ✓ 查询所有用户: 共 {} 条记录\n", all.len());
    for u in &all {
        println!("    - id={}, username={}", u.id, u.username);
    }

    // ============================================
    // 4. UPDATE - 更新记录
    // ============================================
    println!("\n--- UPDATE ---");
    let updated = Model::update(
        &session,
        Model {
            email: "alice_updated@example.com".to_string(),
            active: false,
            ..found
        },
    )
    .await?;
    println!("  ✓ 更新用户 id={}: 新邮箱={}\n", updated.id, updated.email);

    // ============================================
    // 5. DELETE - 删除记录
    // ============================================
    println!("--- DELETE ---");
    let affected = Model::delete(&session, inserted2.id).await?;
    println!("  ✓ 删除用户 id={}: 影响 {} 行\n", inserted2.id, affected);

    // ============================================
    // 验证最终状态
    // ============================================
    println!("--- 最终状态 ---");
    let remaining = Model::find_all(&session).await?;
    println!("  ✓ 剩余用户数: {}", remaining.len());

    // 清理
    session.execute_raw_ddl("DROP TABLE users").await?;

    println!("\n========================================");
    println!("✨ 实体 CRUD 示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - #[derive(DbEntity, DeriveEntityModel)] 定义实体模型");
    println!("  - #[db_crud(table_name = \"...\")] 自动生成 CRUD 方法");
    println!("  - Model::insert()    - 插入记录");
    println!("  - Model::find_by_id() - 按 ID 查询");
    println!("  - Model::find_all()  - 查询所有");
    println!("  - Model::update()    - 更新记录");
    println!("  - Model::delete()    - 按 ID 删除");

    Ok(())
}
