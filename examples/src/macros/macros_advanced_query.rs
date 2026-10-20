// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
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
use dbnexus::sea_orm::entity::prelude::*;
use dbnexus::sea_orm::{
    ColumnTrait, Condition, DbBackend, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use dbnexus::{Migration, MigrationExecutor, TableChange};

// 实体定义

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

// 主函数

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔍 db_entity advanced queries: schema + query + paginate + batch");
    println!("========================================\n");

    // schema() — 从 Entity 自动生成表结构
    println!("--- 1. schema() auto-generates the table structure ---\n");

    let (_pool, session) = common::db::setup_shared_sqlite_session().await?;
    println!("  ✓ connected to the SQLite in-memory database");

    // schema() 调用 sea_orm::Schema::create_table_from_entity 并转换为 migration::schema::Table
    let table = Model::schema(DbBackend::Sqlite);
    let conn = session.connection()?;
    let mut executor = MigrationExecutor::new(conn.clone(), DatabaseType::Sqlite);
    let mut migration = Migration::new(1, "create_users_table".to_string());
    migration.add_table_change(TableChange::CreateTable(table));
    executor.apply_migration(&migration).await?;
    println!("  ✓ schema(DbBackend::Sqlite) generated the table and applied the migration\n");

    // insert_many() — 批量插入
    println!("--- 2. insert_many() batch inserts 5 records ---\n");

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
    println!(
        "  ✓ insert_many succeeded, last_insert_id = {:?}",
        result.last_insert_id
    );

    let total = Model::count(&session).await?;
    println!("  ✓ current total records: {}\n", total);

    // query() — 链式查询（filter + order_by + limit）
    println!("--- 3. query() chained queries: age >= 18, ORDER BY age DESC, LIMIT 2 ---\n");

    // query() 返回 Sea-ORM 原生 Select<E>，可链式调用所有 Sea-ORM 查询方法
    let select = Model::query(&session).await?;
    let conn = session.connection()?;
    let adults = select
        .filter(Column::Age.gte(18)) // WHERE age >= 18
        .order_by_desc(Column::Age) // ORDER BY age DESC
        .limit(2) // LIMIT 2
        .all(conn)
        .await?;

    println!("  ✓ query result (age >= 18, ordered by age desc, first 2):");
    for u in &adults {
        println!("    - id={}, name={}, age={}", u.id, u.name, u.age);
    }
    // 预期: Charlie(30), Alice(25)
    assert_eq!(adults.len(), 2);
    assert_eq!(adults[0].name, "Charlie");
    assert_eq!(adults[1].name, "Alice");
    println!("  ✓ assertions passed\n");

    // paginate() — 分页查询
    println!("--- 4. paginate() paginated query: page_size = 2 ---\n");

    let paginator = Model::paginate(&session, 2).await?;

    let total_items = paginator.num_items().await?;
    let total_pages = paginator.num_pages().await?;
    println!(
        "  ✓ total records: {}, total pages: {}",
        total_items, total_pages
    );
    assert_eq!(total_items, 5);
    assert_eq!(total_pages, 3); // 5 条 / 每页 2 条 = 3 页

    for page_num in 0..total_pages {
        let page = paginator.fetch_page(page_num).await?;
        println!("    page {}: {} records", page_num, page.len());
        for u in &page {
            println!("      - id={}, name={}, age={}", u.id, u.name, u.age);
        }
    }
    println!("  ✓ pagination assertions passed\n");

    // update_many() — 条件批量更新
    println!("--- 5. update_many() conditional batch update: age < 18 → age = 0 ---\n");

    // 将所有未成年人 (age < 18) 的 age 设为 0
    // 预期: Bob(17) 和 Diana(16) 被更新
    let filter: Condition = Column::Age.lt(18).into();
    let updates: Vec<(Column, sea_orm::Value)> = vec![(Column::Age, 0i64.into())];

    let affected = Model::update_many(&session, filter, updates).await?;
    println!("  ✓ update_many done, rows affected: {}", affected);
    assert_eq!(affected, 2, "should update 2 records (Bob and Diana)");

    let conn = session.connection()?;
    let bob = Entity::find_by_id(2)
        .one(conn)
        .await?
        .expect("Bob should exist");
    let diana = Entity::find_by_id(4)
        .one(conn)
        .await?
        .expect("Diana should exist");
    let alice = Entity::find_by_id(1)
        .one(conn)
        .await?
        .expect("Alice should exist");

    println!("    Bob   (id=2): age {} → {}", 17, bob.age);
    println!("    Diana (id=4): age {} → {}", 16, diana.age);
    println!("    Alice (id=1): age {} (unchanged)", alice.age);

    assert_eq!(bob.age, 0, "Bob's age should be 0");
    assert_eq!(diana.age, 0, "Diana's age should be 0");
    assert_eq!(alice.age, 25, "Alice's age should remain 25");
    println!("  ✓ assertions passed\n");

    // 复合条件 update_many()
    println!("--- 6. update_many() composite condition: age < 18 OR age > 28 → age = 100 ---\n");

    // 重置数据：先恢复 Bob 和 Diana 的 age 为 17（< 18，以满足后续复合条件）
    let restore_filter: Condition = Column::Age.eq(0).into();
    let restore_updates: Vec<(Column, sea_orm::Value)> = vec![(Column::Age, 17i64.into())];
    Model::update_many(&session, restore_filter, restore_updates).await?;

    // 复合条件: age < 18 OR age > 28
    // 预期: Bob(17), Diana(17), Charlie(30) → 3 条
    let cond = Condition::any()
        .add(Column::Age.lt(18))
        .add(Column::Age.gt(28));
    let updates: Vec<(Column, sea_orm::Value)> = vec![(Column::Age, 100i64.into())];

    let affected = Model::update_many(&session, cond, updates).await?;
    println!(
        "  ✓ update_many with composite conditions done, rows affected: {}",
        affected
    );
    assert_eq!(affected, 3, "should update 3 (Bob, Diana, Charlie)");

    let conn = session.connection()?;
    let count_age_100 = Entity::find()
        .filter(Column::Age.eq(100))
        .count(conn)
        .await?;
    println!("  ✓ count of records with age=100: {}", count_age_100);
    assert_eq!(count_age_100, 3);
    println!("  ✓ assertions passed\n");

    // 总结
    println!("========================================");
    println!("✨ Example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  schema(backend)         generates migration::schema::Table from the Entity");
    println!("    → reuses sea_orm::Schema::create_table_from_entity");
    println!("    → the converter turns TableCreateStatement into migration::schema::Table");
    println!("  query(session)          returns the native Sea-ORM Select<E>");
    println!("    → supports .filter().order_by().limit().all() chaining");
    println!("    → zero duplicate implementation, fully reusing the Sea-ORM query capability");
    println!("  paginate(session, size) paginated query");
    println!("    → .num_items() / .num_pages() / .fetch_page(n)");
    println!("  insert_many(session, v) batch insert");
    println!("    → returns InsertResult including last_insert_id");
    println!("  update_many(session, c, u) conditional batch update");
    println!("    → accepts Condition + Vec<(Column, Value)>");
    println!("    → returns the number of affected rows");

    Ok(())
}
