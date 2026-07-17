// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Task 6.1-6.3 验证: timestamps = true 自动时间戳集成测试
//!
//! 验证 `#[db_entity(table_name = "...", primary_key = "...", timestamps = true)]`：
//! - insert 时自动设置 `created_at` + `updated_at`
//! - update 时仅更新 `updated_at`，`created_at` 保持不变
//! - 时间戳字段类型为 `Option<time::OffsetDateTime>`（Sea-ORM 标准）

use dbnexus::db_entity;
use dbnexus::foundation::DatabaseType;
use dbnexus::sea_orm::entity::prelude::*;
use dbnexus::sea_orm::{ActiveModelTrait, DbBackend, EntityTrait, Set};
use dbnexus::{Migration, MigrationExecutor, TableChange};

/// 测试用实体 — 启用 timestamps
#[db_entity(table_name = "events", primary_key = "id", timestamps = true)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub created_at: Option<time::OffsetDateTime>,
    pub updated_at: Option<time::OffsetDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

/// 测试夹具：创建内存 SQLite 数据库 + events 表
async fn setup() -> dbnexus::DbPool {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let table = Model::schema(DbBackend::Sqlite);
    let conn = session.connection().expect("Failed to get connection");
    let mut executor = MigrationExecutor::new(conn.clone(), DatabaseType::Sqlite);
    let mut migration = Migration::new(1, "create_events_table".to_string());
    migration.add_table_change(TableChange::CreateTable(table));
    executor
        .apply_migration(&migration)
        .await
        .expect("Migration should succeed");

    pool
}

/// Task 6.2 验证: insert 时自动设置 created_at + updated_at
#[tokio::test]
async fn test_insert_sets_both_timestamps() {
    let pool = setup().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let conn = session.connection().expect("conn");

    // 构造 ActiveModel — 不设置 created_at/updated_at
    let active_model: ActiveModel = Model {
        id: 1,
        name: "event1".to_string(),
        created_at: None,
        updated_at: None,
    }
    .into();

    // 用 ActiveModel::insert 触发 before_save(insert=true)
    let model: Model = active_model.insert(conn).await.expect("insert should succeed");
    assert_eq!(model.id, 1);

    // 查询验证
    let found: Option<Model> = Entity::find_by_id(1).one(conn).await.expect("query should succeed");
    let found = found.expect("record should exist");

    // 验证 created_at 和 updated_at 都被自动设置
    assert!(found.created_at.is_some(), "created_at should be auto-set on insert");
    assert!(found.updated_at.is_some(), "updated_at should be auto-set on insert");

    // 两者应该相等（同一时刻设置）
    let created = found.created_at.unwrap();
    let updated = found.updated_at.unwrap();
    assert_eq!(
        created, updated,
        "created_at and updated_at should be equal on insert (set at same time)"
    );
}

/// Task 6.2 验证: update 时仅更新 updated_at，created_at 保持不变
#[tokio::test]
async fn test_update_only_changes_updated_at() {
    let pool = setup().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let conn = session.connection().expect("conn");

    // 先插入一条记录
    let active_model: ActiveModel = Model {
        id: 1,
        name: "original".to_string(),
        created_at: None,
        updated_at: None,
    }
    .into();
    let _: Model = active_model.insert(conn).await.expect("insert should succeed");

    // 查询获取原始时间戳
    let original: Model = Entity::find_by_id(1)
        .one(conn)
        .await
        .expect("query should succeed")
        .expect("record should exist");
    let original_created = original.created_at.expect("created_at should be set");
    let original_updated = original.updated_at.expect("updated_at should be set");

    // 等待一小段时间，确保 updated_at 会不同
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 更新 name 字段 — 用 from_db 查询后转 ActiveModel 再修改
    let mut to_update: ActiveModel = original.into();
    to_update.name = Set("updated".to_string());
    let _: Model = to_update.update(conn).await.expect("update should succeed");

    // 查询验证
    let found: Model = Entity::find_by_id(1)
        .one(conn)
        .await
        .expect("query should succeed")
        .expect("record should exist");

    // created_at 应保持不变
    assert_eq!(
        found.created_at.expect("created_at should exist"),
        original_created,
        "created_at should NOT change on update"
    );

    // updated_at 应该变了
    assert_ne!(
        found.updated_at.expect("updated_at should exist"),
        original_updated,
        "updated_at SHOULD change on update"
    );

    // name 应该已更新
    assert_eq!(found.name, "updated");
}

/// Task 6.3 验证: timestamps=true 要求 Model 包含 created_at/updated_at 字段
///
/// 此测试通过实际编译验证：如果字段缺失或类型错误，编译会失败。
/// 这里我们验证字段类型为 Option<time::OffsetDateTime> 时一切正常。
#[tokio::test]
async fn test_timestamps_field_type_compiles() {
    let pool = setup().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let conn = session.connection().expect("conn");

    // 正常插入 — 验证类型系统正常工作
    let am: ActiveModel = Model {
        id: 42,
        name: "type_check".to_string(),
        created_at: None,
        updated_at: None,
    }
    .into();
    let _: Model = am.insert(conn).await.expect("insert should succeed");

    let found: Option<Model> = Entity::find_by_id(42).one(conn).await.expect("query should succeed");
    assert!(found.is_some(), "record should exist");
}
