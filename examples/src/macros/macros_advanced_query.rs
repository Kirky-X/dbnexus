// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! db_entity 高级查询示例（schema + query + paginate + batch）
//!
//! 演示 `#[db_entity(...)]` 宏生成的高级查询和批量操作方法：
//! - `schema(backend)` — 从 Entity 自动生成 `migration::schema::Table`，可直接用于迁移
//! - `query(session)` — 返回 Sea-ORM 原生 `Select<E>`，支持链式 `filter/order_by/limit`
//! - `paginate(session, page_size)` — 分页查询，支持 `num_items/num_pages/fetch_page`
//! - `insert_many(session, models)` — 批量插入
//! - `update_many(session, filter, updates)` — 条件批量更新
//!
//! 所有方法复用 Sea-ORM 原生能力，0 重复实现。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run -p dbnexus-examples --bin macros_advanced_query
//! ```

#[path = "../common/mod.rs"]
mod common;

use dbnexus::db_entity;
use dbnexus::foundation::DatabaseType;
use dbnexus::{Migration, MigrationExecutor, TableChange};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbBackend, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};

// ============================================
// 实体定义
// ============================================

/// 用户实体 — 用于演示高级查询和批量操作
#[db_entity(table_name = "users", primary_key = "id")]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
    pub age: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔍 db_entity 高级查询: schema + query + paginate + batch");
    println!("========================================\n");

    // ============================================
    // 1. schema() — 从 Entity 自动生成表结构
    // ============================================
    println!("--- 1. schema() 自动生成表结构 ---\n");

    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("  ✓ 连接 SQLite 内存数据库成功");

    // schema() 调用 sea_orm::Schema::create_table_from_entity 并转换为 migration::schema::Table
    let table = Model::schema(DbBackend::Sqlite);
    let conn = session.connection()?;
    let mut executor = MigrationExecutor::new(conn.clone(), DatabaseType::Sqlite);
    let mut migration = Migration::new(1, "create_users_table".to_string());
    migration.add_table_change(TableChange::CreateTable(table));
    executor.apply_migration(&migration).await?;
    println!("  ✓ schema(DbBackend::Sqlite) 生成表并应用迁移成功\n");

    // ============================================
    // 2. insert_many() — 批量插入
    // ============================================
    println!("--- 2. insert_many() 批量插入 5 条记录 ---\n");

    let users = vec![
        Model {
            id: 1,
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            age: 25,
        },
        Model {
            id: 2,
            name: "Bob".to_string(),
            email: "bob@example.com".to_string(),
            age: 17,
        },
        Model {
            id: 3,
            name: "Charlie".to_string(),
            email: "charlie@example.com".to_string(),
            age: 30,
        },
        Model {
            id: 4,
            name: "Diana".to_string(),
            email: "diana@example.com".to_string(),
            age: 16,
        },
        Model {
            id: 5,
            name: "Eve".to_string(),
            email: "eve@example.com".to_string(),
            age: 22,
        },
    ];

    let result = Model::insert_many(&session, users).await?;
    println!("  ✓ insert_many 成功, last_insert_id = {:?}", result.last_insert_id);

    let total = Model::count(&session).await?;
    println!("  ✓ 当前总记录数: {}\n", total);

    // ============================================
    // 3. query() — 链式查询（filter + order_by + limit）
    // ============================================
    println!("--- 3. query() 链式查询: age >= 18, ORDER BY age DESC, LIMIT 2 ---\n");

    // query() 返回 Sea-ORM 原生 Select<E>，可链式调用所有 Sea-ORM 查询方法
    let select = Model::query(&session).await?;
    let conn = session.connection()?;
    let adults = select
        .filter(Column::Age.gte(18))    // WHERE age >= 18
        .order_by_desc(Column::Age)      // ORDER BY age DESC
        .limit(2)                        // LIMIT 2
        .all(conn)
        .await?;

    println!("  ✓ 查询结果 (age >= 18, 按 age 降序, 前 2 条):");
    for u in &adults {
        println!("    - id={}, name={}, age={}", u.id, u.name, u.age);
    }
    // 预期: Charlie(30), Alice(25)
    assert_eq!(adults.len(), 2);
    assert_eq!(adults[0].name, "Charlie");
    assert_eq!(adults[1].name, "Alice");
    println!("  ✓ 断言通过\n");

    // ============================================
    // 4. paginate() — 分页查询
    // ============================================
    println!("--- 4. paginate() 分页查询: page_size = 2 ---\n");

    let paginator = Model::paginate(&session, 2).await?;

    let total_items = paginator.num_items().await?;
    let total_pages = paginator.num_pages().await?;
    println!("  ✓ 总记录数: {}, 总页数: {}", total_items, total_pages);
    assert_eq!(total_items, 5);
    assert_eq!(total_pages, 3); // 5 条 / 每页 2 条 = 3 页

    for page_num in 0..total_pages {
        let page = paginator.fetch_page(page_num).await?;
        println!("    第 {} 页: {} 条记录", page_num, page.len());
        for u in &page {
            println!("      - id={}, name={}, age={}", u.id, u.name, u.age);
        }
    }
    println!("  ✓ 分页断言通过\n");

    // ============================================
    // 5. update_many() — 条件批量更新
    // ============================================
    println!("--- 5. update_many() 条件批量更新: age < 18 → age = 0 ---\n");

    // 将所有未成年人 (age < 18) 的 age 设为 0
    // 预期: Bob(17) 和 Diana(16) 被更新
    let filter: Condition = Column::Age.lt(18).into();
    let updates: Vec<(Column, sea_orm::Value)> = vec![(Column::Age, 0i64.into())];

    let affected = Model::update_many(&session, filter, updates).await?;
    println!("  ✓ update_many 完成, 受影响行数: {}", affected);
    assert_eq!(affected, 2, "应更新 2 条记录 (Bob 和 Diana)");

    // 验证更新结果
    let conn = session.connection()?;
    let bob = Entity::find_by_id(2).one(conn).await?.expect("Bob 应存在");
    let diana = Entity::find_by_id(4).one(conn).await?.expect("Diana 应存在");
    let alice = Entity::find_by_id(1).one(conn).await?.expect("Alice 应存在");

    println!("    Bob   (id=2): age {} → {}", 17, bob.age);
    println!("    Diana (id=4): age {} → {}", 16, diana.age);
    println!("    Alice (id=1): age {} (未变)", alice.age);

    assert_eq!(bob.age, 0, "Bob 的 age 应为 0");
    assert_eq!(diana.age, 0, "Diana 的 age 应为 0");
    assert_eq!(alice.age, 25, "Alice 的 age 应保持 25");
    println!("  ✓ 断言通过\n");

    // ============================================
    // 6. 复合条件 update_many()
    // ============================================
    println!("--- 6. update_many() 复合条件: age < 18 OR age > 28 → age = 100 ---\n");

    // 重置数据：先恢复 Bob 和 Diana 的 age 为 17（< 18，以满足后续复合条件）
    let restore_filter: Condition = Column::Age.eq(0).into();
    let restore_updates: Vec<(Column, sea_orm::Value)> = vec![(Column::Age, 17i64.into())];
    Model::update_many(&session, restore_filter, restore_updates).await?;

    // 复合条件: age < 18 OR age > 28
    // 预期: Bob(17), Diana(17), Charlie(30) → 3 条
    let cond = Condition::any().add(Column::Age.lt(18)).add(Column::Age.gt(28));
    let updates: Vec<(Column, sea_orm::Value)> = vec![(Column::Age, 100i64.into())];

    let affected = Model::update_many(&session, cond, updates).await?;
    println!("  ✓ update_many 复合条件完成, 受影响行数: {}", affected);
    assert_eq!(affected, 3, "应更新 3 条 (Bob, Diana, Charlie)");

    // 验证
    let conn = session.connection()?;
    let count_age_100 = Entity::find().filter(Column::Age.eq(100)).count(conn).await?;
    println!("  ✓ age=100 的记录数: {}", count_age_100);
    assert_eq!(count_age_100, 3);
    println!("  ✓ 断言通过\n");

    // ============================================
    // 总结
    // ============================================
    println!("========================================");
    println!("✨ 示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  schema(backend)         从 Entity 自动生成 migration::schema::Table");
    println!("    → 复用 sea_orm::Schema::create_table_from_entity");
    println!("    → 转换器将 TableCreateStatement 转为 migration::schema::Table");
    println!("  query(session)          返回 Sea-ORM 原生 Select<E>");
    println!("    → 支持 .filter().order_by().limit().all() 链式调用");
    println!("    → 0 重复实现，完全复用 Sea-ORM 查询能力");
    println!("  paginate(session, size) 分页查询");
    println!("    → .num_items() / .num_pages() / .fetch_page(n)");
    println!("  insert_many(session, v) 批量插入");
    println!("    → 返回 InsertResult 含 last_insert_id");
    println!("  update_many(session, c, u) 条件批量更新");
    println!("    → 接受 Condition + Vec<(Column, Value)>");
    println!("    → 返回受影响行数");

    Ok(())
}
