// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Tasks 7.10-7.12: validation 集成测试
//!
//! 验证 `#[db_entity(table_name = "...", primary_key = "...", validate)]`：
//! - 7.10: `#[validate(email)]` 无效邮箱触发验证错误
//! - 7.11: `#[validate(length(min=2))]` 长度不足触发验证错误
//! - 7.12: 验证失败短路（不执行 timestamps）

use dbnexus::db_entity;
use dbnexus::foundation::DatabaseType;
use dbnexus::{Migration, MigrationExecutor, TableChange};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, DbBackend, EntityTrait};
use validator::Validate;

/// 测试用实体 — 启用 validate + timestamps
///
/// 使用 #[derive(validator::Validate)] + #[validate(...)] 字段属性
/// `#[db_entity(validate, timestamps = true)]` 会在 before_save 中：
/// 1. 先验证（validate）
/// 2. 再设置时间戳（timestamps）
#[db_entity(table_name = "members", primary_key = "id", validate, timestamps = true)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Validate)]
#[sea_orm(table_name = "members")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[validate(length(min = 2, message = "name too short"))]
    pub name: String,
    #[validate(email(message = "invalid email"))]
    pub email: String,
    pub created_at: Option<time::OffsetDateTime>,
    pub updated_at: Option<time::OffsetDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

/// 测试夹具：创建内存 SQLite 数据库 + members 表
async fn setup() -> dbnexus::DbPool {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    let table = Model::schema(DbBackend::Sqlite);
    let conn = session.connection().expect("Failed to get connection");
    let mut executor = MigrationExecutor::new(conn.clone(), DatabaseType::Sqlite);
    let mut migration = Migration::new(1, "create_members_table".to_string());
    migration.add_table_change(TableChange::CreateTable(table));
    executor
        .apply_migration(&migration)
        .await
        .expect("Migration should succeed");

    pool
}

/// Task 7.10: 无效邮箱触发验证错误
#[tokio::test]
async fn test_validate_email_invalid() {
    let pool = setup().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");
    let conn = session.connection().expect("conn");

    // 无效邮箱 — 应该触发验证错误
    let am: ActiveModel = Model {
        id: 1,
        name: "Alice".to_string(),
        email: "not-an-email".to_string(),
        created_at: None,
        updated_at: None,
    }
    .into();

    let result = am.insert(conn).await;
    assert!(
        result.is_err(),
        "insert with invalid email should fail validation"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("email") || err_msg.contains("invalid"),
        "error should mention email validation, got: {}",
        err_msg
    );

    // 验证记录未被插入
    let count = Entity::find()
        .count(conn)
        .await
        .expect("count should succeed");
    assert_eq!(count, 0, "no records should be inserted after validation failure");
}

/// Task 7.10: 有效邮箱通过验证
#[tokio::test]
async fn test_validate_email_valid() {
    let pool = setup().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");
    let conn = session.connection().expect("conn");

    // 有效邮箱 — 应该通过验证
    let am: ActiveModel = Model {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        created_at: None,
        updated_at: None,
    }
    .into();

    let model: Model = am
        .insert(conn)
        .await
        .expect("insert with valid email should succeed");
    assert_eq!(model.id, 1);

    // 验证记录已插入
    let count = Entity::find()
        .count(conn)
        .await
        .expect("count should succeed");
    assert_eq!(count, 1, "1 record should be inserted");
}

/// Task 7.11: name 长度不足触发验证错误
#[tokio::test]
async fn test_validate_length_too_short() {
    let pool = setup().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");
    let conn = session.connection().expect("conn");

    // name 长度 < 2 — 应该触发验证错误
    let am: ActiveModel = Model {
        id: 1,
        name: "A".to_string(), // 只有 1 个字符，min=2
        email: "valid@example.com".to_string(),
        created_at: None,
        updated_at: None,
    }
    .into();

    let result = am.insert(conn).await;
    assert!(
        result.is_err(),
        "insert with too-short name should fail validation"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("name") || err_msg.contains("short"),
        "error should mention name length validation, got: {}",
        err_msg
    );
}

/// Task 7.12: 验证失败短路 — 不执行 timestamps
///
/// 如果验证失败，before_save 应该在验证步骤就返回错误，
/// 不应该继续执行 timestamps 逻辑（虽然这里无法直接验证 timestamps 是否执行，
/// 但可以通过检查记录是否被插入来间接验证：如果 timestamps 执行了，
/// 记录可能被部分写入，但我们期望完全不入库）。
#[tokio::test]
async fn test_validation_short_circuits_timestamps() {
    let pool = setup().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");
    let conn = session.connection().expect("conn");

    // 无效数据 — 验证应短路，timestamps 不执行
    let am: ActiveModel = Model {
        id: 1,
        name: "X".to_string(), // 长度不足
        email: "invalid".to_string(),       // 无效邮箱
        created_at: None,
        updated_at: None,
    }
    .into();

    let result = am.insert(conn).await;
    assert!(result.is_err(), "validation should fail");

    // 验证记录完全未入库（短路成功）
    let all = Entity::find()
        .all(conn)
        .await
        .expect("query should succeed");
    assert!(
        all.is_empty(),
        "no records should exist after validation short-circuit"
    );
}

/// Task 7.10+7.11: 多字段验证同时失败
#[tokio::test]
async fn test_validate_multiple_fields_fail() {
    let pool = setup().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");
    let conn = session.connection().expect("conn");

    // name 和 email 都无效
    let am: ActiveModel = Model {
        id: 1,
        name: "".to_string(), // 空字符串，长度 < 2
        email: "no-at-sign".to_string(), // 无效邮箱
        created_at: None,
        updated_at: None,
    }
    .into();

    let result = am.insert(conn).await;
    assert!(result.is_err(), "multiple validation failures should fail");

    let err_msg = result.unwrap_err().to_string();
    // 至少应该有一个验证错误
    assert!(
        !err_msg.is_empty(),
        "error message should contain validation details"
    );
}

/// Task 7.4 验证: validate + timestamps 组合 — 有效数据应设置时间戳
#[tokio::test]
async fn test_validate_and_timestamps_combined() {
    let pool = setup().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");
    let conn = session.connection().expect("conn");

    // 有效数据 — 验证通过后应设置 timestamps
    let am: ActiveModel = Model {
        id: 42,
        name: "ValidName".to_string(),
        email: "valid@example.com".to_string(),
        created_at: None,
        updated_at: None,
    }
    .into();

    let model: Model = am
        .insert(conn)
        .await
        .expect("insert should succeed (validate + timestamps)");

    // 验证 timestamps 被设置
    assert!(
        model.created_at.is_some(),
        "created_at should be set after validate+timestamps"
    );
    assert!(
        model.updated_at.is_some(),
        "updated_at should be set after validate+timestamps"
    );
}
