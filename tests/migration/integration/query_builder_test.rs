// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Tasks 5.5-5.8: query/paginate/insert_many/update_many 集成测试
//!
//! 验证 `#[db_entity]` 宏生成的方法：
//! - 5.5: `query().filter().order_by().limit().all()` 链式调用
//! - 5.6: `paginate().fetch_page(n)` + `num_pages()` + `num_items()`
//! - 5.7: `insert_many` 批量插入
//! - 5.8: `update_many` 条件批量更新

use dbnexus::db_entity;
use dbnexus::foundation::DatabaseType;
use dbnexus::{Migration, MigrationExecutor, TableChange};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ColumnTrait, Condition, DbBackend, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};

/// 测试用实体 — 包含 age 字段以支持 update_many 测试
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

/// 测试夹具：创建内存 SQLite 数据库 + users 表 + 5 条初始记录
async fn setup_with_seed() -> dbnexus::DbPool {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    // 用 schema() 建表
    let table = Model::schema(DbBackend::Sqlite);
    let conn = session.connection().expect("Failed to get connection");
    let mut executor = MigrationExecutor::new(conn.clone(), DatabaseType::Sqlite);
    let mut migration = Migration::new(1, "create_users_table".to_string());
    migration.add_table_change(TableChange::CreateTable(table));
    executor
        .apply_migration(&migration)
        .await
        .expect("Migration should succeed");

    // 插入 5 条种子数据
    let seed = vec![
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
    let result = Model::insert_many(&session, seed)
        .await
        .expect("insert_many should succeed");
    assert!(
        result.last_insert_id.is_some(),
        "insert_many should return last_insert_id"
    );

    pool
}

/// Task 5.5: query().filter().order_by().limit().all() 链式调用
#[tokio::test]
async fn test_query_chain_filter_order_limit() {
    let pool = setup_with_seed().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    // 链式调用：filter(age >= 18) + order_by(age desc) + limit(2)
    let select = Model::query(&session)
        .await
        .expect("query should pass permission check");
    let conn = session.connection().expect("connection should be available");
    let adults = select
        .filter(Column::Age.gte(18))
        .order_by_desc(Column::Age)
        .limit(2)
        .all(conn)
        .await
        .expect("query should succeed");

    // 预期：age >= 18 的有 Alice(25), Charlie(30), Eve(22)
    // 按 age desc 排序 + limit 2 → Charlie(30), Alice(25)
    assert_eq!(adults.len(), 2, "should return 2 records after limit");
    assert_eq!(adults[0].name, "Charlie", "first should be Charlie (age 30)");
    assert_eq!(adults[1].name, "Alice", "second should be Alice (age 25)");
}

/// Task 5.6: paginate().fetch_page(n) + num_pages() + num_items()
#[tokio::test]
async fn test_paginate_fetch_page_and_counts() {
    let pool = setup_with_seed().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    // page_size = 2 → 5 条记录应分为 3 页 (2,2,1)
    let paginator = Model::paginate(&session, 2)
        .await
        .expect("paginate should pass permission check");

    let total_items = paginator
        .num_items()
        .await
        .expect("num_items should succeed");
    assert_eq!(total_items, 5, "total items should be 5");

    let total_pages = paginator
        .num_pages()
        .await
        .expect("num_pages should succeed");
    assert_eq!(total_pages, 3, "5 items / 2 per page = 3 pages");

    // 第 0 页：2 条
    let page0 = paginator
        .fetch_page(0)
        .await
        .expect("fetch_page(0) should succeed");
    assert_eq!(page0.len(), 2, "page 0 should have 2 items");

    // 第 2 页：1 条（最后一页）
    let page2 = paginator
        .fetch_page(2)
        .await
        .expect("fetch_page(2) should succeed");
    assert_eq!(page2.len(), 1, "page 2 (last) should have 1 item");
}

/// Task 5.7: insert_many 批量插入
///
/// 验证：插入后行数正确，last_insert_id 返回，且各记录可独立查询
#[tokio::test]
async fn test_insert_many_batch_insert() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    // 建表
    let table = Model::schema(DbBackend::Sqlite);
    let conn = session.connection().expect("Failed to get connection");
    let mut executor = MigrationExecutor::new(conn.clone(), DatabaseType::Sqlite);
    let mut migration = Migration::new(1, "create_users_table".to_string());
    migration.add_table_change(TableChange::CreateTable(table));
    executor
        .apply_migration(&migration)
        .await
        .expect("Migration should succeed");

    // 批量插入 3 条
    let new_users = vec![
        Model {
            id: 10,
            name: "Frank".to_string(),
            email: "frank@example.com".to_string(),
            age: 40,
        },
        Model {
            id: 11,
            name: "Grace".to_string(),
            email: "grace@example.com".to_string(),
            age: 35,
        },
        Model {
            id: 12,
            name: "Heidi".to_string(),
            email: "heidi@example.com".to_string(),
            age: 28,
        },
    ];
    let result = Model::insert_many(&session, new_users)
        .await
        .expect("insert_many should succeed");
    assert!(
        result.last_insert_id.is_some(),
        "last_insert_id should be Some"
    );

    // 验证总行数 = 3
    let count = Entity::find()
        .count(session.connection().expect("conn"))
        .await
        .expect("count should succeed");
    assert_eq!(count, 3, "should have 3 records after insert_many");

    // 验证各记录可独立查询
    for id in [10i64, 11, 12] {
        let found: Option<Model> = Entity::find_by_id(id)
            .one(session.connection().expect("conn"))
            .await
            .expect("find_by_id should succeed");
        assert!(
            found.is_some(),
            "record with id={} should exist after insert_many",
            id
        );
    }
}

/// Task 5.8: update_many 条件批量更新
///
/// 验证：filter(age < 18) + updates(status="minor") → 受影响行数 = 2
#[tokio::test]
async fn test_update_many_conditional_batch() {
    let pool = setup_with_seed().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    // 初始数据中 age < 18 的有 Bob(17) 和 Diana(16) → 2 条
    // 但我们的 Model 没有 status 字段，所以改为更新 age 字段
    // filter: age < 18, update: age = 0
    let filter: Condition = Column::Age.lt(18).into();
    let updates: Vec<(Column, sea_orm::Value)> = vec![(Column::Age, 0i64.into())];

    let affected = Model::update_many(&session, filter, updates)
        .await
        .expect("update_many should succeed");
    assert_eq!(
        affected, 2,
        "should update 2 records (Bob and Diana, both age < 18)"
    );

    // 验证 Bob 和 Diana 的 age 现在都是 0
    let conn = session.connection().expect("conn");
    let bob: Option<Model> = Entity::find_by_id(2)
        .one(conn)
        .await
        .expect("find_by_id should succeed");
    let bob = bob.expect("Bob should exist");
    assert_eq!(bob.age, 0, "Bob's age should be updated to 0");

    let diana: Option<Model> = Entity::find_by_id(4)
        .one(conn)
        .await
        .expect("find_by_id should succeed");
    let diana = diana.expect("Diana should exist");
    assert_eq!(diana.age, 0, "Diana's age should be updated to 0");

    // 验证其他记录未受影响（Alice age=25 未变）
    let alice: Option<Model> = Entity::find_by_id(1)
        .one(conn)
        .await
        .expect("find_by_id should succeed");
    let alice = alice.expect("Alice should exist");
    assert_eq!(alice.age, 25, "Alice's age should be unchanged (25)");
}

/// 额外测试：update_many 用 Condition 组合多条件
#[tokio::test]
async fn test_update_many_with_complex_condition() {
    let pool = setup_with_seed().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    // Condition: age < 18 OR age > 28 → Bob(17), Diana(16), Charlie(30)
    let cond = Condition::any()
        .add(Column::Age.lt(18))
        .add(Column::Age.gt(28));
    let updates: Vec<(Column, sea_orm::Value)> = vec![(Column::Age, 100i64.into())];

    let affected = Model::update_many(&session, cond, updates)
        .await
        .expect("update_many should succeed");
    assert_eq!(
        affected, 3,
        "should update 3 records (Bob, Diana, Charlie)"
    );

    // 验证 age=100 的记录数 = 3
    let conn = session.connection().expect("conn");
    let count = Entity::find()
        .filter(Column::Age.eq(100))
        .count(conn)
        .await
        .expect("count should succeed");
    assert_eq!(count, 3, "should have 3 records with age=100");
}
