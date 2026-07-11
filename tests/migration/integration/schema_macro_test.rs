// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Task 4.9: schema() 方法集成测试
//!
//! 验证 `#[db_entity]` 宏生成的 `schema(backend)` 方法：
//! 1. 返回的 `Table` 结构包含完整表名/列/主键信息
//! 2. 该 `Table` 可直接喂 `Migration::add_table_change(TableChange::CreateTable(...))`，
//!    `MigrationExecutor::apply_migration` 成功建表

use dbnexus::db_entity;
use dbnexus::foundation::DatabaseType;
use dbnexus::{Migration, MigrationExecutor, TableChange};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbBackend, Statement};

/// 测试用实体 — `#[db_entity]` 宏生成 `schema()` 等方法
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

/// Task 4.9 Scenario 1: schema() 返回完整 Table
///
/// 验证 `Model::schema(DbBackend::Sqlite)` 返回的 `Table` 包含：
/// - 正确的表名
/// - 所有列定义（含列名/类型/可空/主键）
/// - `primary_key_columns` 包含主键列名
#[tokio::test]
async fn test_schema_returns_complete_table() {
    let table = Model::schema(DbBackend::Sqlite);

    // 验证表名
    assert_eq!(table.name, "users");

    // 验证列存在（至少 id、name、email）
    assert!(
        table.columns.len() >= 3,
        "should have at least id, name, email columns; got: {:?}",
        table.columns.iter().map(|c| &c.name).collect::<Vec<_>>()
    );

    // 验证 id 列是主键
    let id_col = table
        .columns
        .iter()
        .find(|c| c.name == "id")
        .expect("id column should exist");
    assert!(id_col.is_primary_key, "id should be primary key");

    // 验证 primary_key_columns 包含 id
    assert!(
        table.primary_key_columns.contains(&"id".to_string()),
        "primary_key_columns should contain 'id'; got: {:?}",
        table.primary_key_columns
    );

    // 验证 name 和 email 列存在
    assert!(
        table.columns.iter().any(|c| c.name == "name"),
        "name column should exist"
    );
    assert!(
        table.columns.iter().any(|c| c.name == "email"),
        "email column should exist"
    );
}

/// Task 4.9 Scenario 2: schema() 返回的 Table 可直接喂 Migration，apply_migration 成功建表
#[tokio::test]
async fn test_schema_table_applied_by_executor() {
    // 使用内存 SQLite 数据库
    let conn = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to SQLite");
    let mut executor = MigrationExecutor::new(conn, DatabaseType::Sqlite);

    // 使用宏生成的 schema() 方法获取 Table
    let table = Model::schema(DbBackend::Sqlite);

    // 创建 Migration 并添加 CreateTable 变更
    let mut migration = Migration::new(1, "create_users_table".to_string());
    migration.add_table_change(TableChange::CreateTable(table));

    // 执行迁移 — 应该成功
    executor
        .apply_migration(&migration)
        .await
        .expect("Migration should succeed");

    // 验证 users 表已在数据库中创建
    let stmt = Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT name FROM sqlite_master WHERE type='table' AND name='users'",
        [],
    );
    let rows = executor
        .connection
        .query_all_raw(stmt)
        .await
        .expect("Query should succeed");
    assert_eq!(
        rows.len(),
        1,
        "users table should exist in sqlite_master after migration"
    );

    // 验证迁移版本已记录
    assert_eq!(
        executor.get_all_versions(),
        vec![1],
        "migration version 1 should be recorded in history"
    );
}

/// Task 4.9 Scenario 3: 验证 schema() 生成的表可被 sea-orm EntityTrait 正常查询
///
/// 确保宏生成的 schema() 产出的 Table 结构与 Sea-ORM Entity 定义一致，
/// 不会出现列名/类型不匹配的问题。
#[tokio::test]
async fn test_schema_table_compatible_with_sea_orm_entity() {
    let conn = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to SQLite");
    let mut executor = MigrationExecutor::new(conn.clone(), DatabaseType::Sqlite);

    // 用 schema() 建表
    let table = Model::schema(DbBackend::Sqlite);
    let mut migration = Migration::new(1, "create_users_for_compat_test".to_string());
    migration.add_table_change(TableChange::CreateTable(table));
    executor
        .apply_migration(&migration)
        .await
        .expect("Migration should succeed");

    // 用 Sea-ORM EntityTrait 插入一条记录
    let active_model: ActiveModel = Model {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    }
    .into();
    let result = Entity::insert(active_model)
        .exec(&conn)
        .await
        .expect("Insert should succeed");
    assert_eq!(result.last_insert_id, 1);

    // 用 Sea-ORM EntityTrait 查询
    let found: Option<Model> = Entity::find_by_id(1).one(&conn).await.expect("Query should succeed");
    let found = found.expect("Record should exist");
    assert_eq!(found.name, "Alice");
    assert_eq!(found.email, "alice@example.com");
}
