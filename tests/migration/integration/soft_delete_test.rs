// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Tasks 6.10-6.12: soft_delete 集成测试
//!
//! 验证 `#[db_entity(table_name = "...", primary_key = "...", soft_delete = true)]`：
//! - 6.10: find_all 不返回已软删除记录，find_with_deleted 返回全部，find_only_deleted 仅返回已删除
//! - 6.11: delete 软删除，force_delete 物理删除
//! - 6.12: count 自动过滤已软删除记录

use dbnexus::db_entity;
use dbnexus::foundation::DatabaseType;
use dbnexus::{Migration, MigrationExecutor, TableChange};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, DbBackend, EntityTrait};

/// 测试用实体 — 启用 soft_delete（deleted_at 自动注入）
#[db_entity(table_name = "articles", primary_key = "id", soft_delete = true)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "articles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub title: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

/// 测试夹具：创建内存 SQLite 数据库 + articles 表 + 3 条初始记录
async fn setup_with_seed() -> dbnexus::DbPool {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 用 schema() 建表
    let table = Model::schema(DbBackend::Sqlite);
    let conn = session.connection().expect("Failed to get connection");
    let mut executor = MigrationExecutor::new(conn.clone(), DatabaseType::Sqlite);
    let mut migration = Migration::new(1, "create_articles_table".to_string());
    migration.add_table_change(TableChange::CreateTable(table));
    executor
        .apply_migration(&migration)
        .await
        .expect("Migration should succeed");

    // 插入 3 条种子数据
    let seed = vec![
        Model {
            id: 1,
            title: "Article 1".to_string(),
            deleted_at: None,
        },
        Model {
            id: 2,
            title: "Article 2".to_string(),
            deleted_at: None,
        },
        Model {
            id: 3,
            title: "Article 3".to_string(),
            deleted_at: None,
        },
    ];
    for m in seed {
        let am: ActiveModel = m.into();
        let _: Model = am
            .insert(session.connection().expect("conn"))
            .await
            .expect("insert should succeed");
    }

    pool
}

/// Task 6.10: find_all 不返回已软删除记录，find_with_deleted 返回全部
#[tokio::test]
async fn test_find_all_excludes_soft_deleted() {
    let pool = setup_with_seed().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 初始 find_all 返回 3 条
    let all = Model::find_all(&session).await.expect("find_all should succeed");
    assert_eq!(all.len(), 3, "initially should have 3 records");

    // 软删除 id=2
    let affected = Model::delete(&session, 2).await.expect("delete should succeed");
    assert_eq!(affected, 1, "should soft-delete 1 record");

    // find_all 现在返回 2 条（排除已软删除的 id=2）
    let all = Model::find_all(&session).await.expect("find_all should succeed");
    assert_eq!(all.len(), 2, "find_all should exclude soft-deleted record");
    assert!(
        all.iter().all(|m| m.id != 2),
        "soft-deleted record should not appear in find_all"
    );

    // find_with_deleted 返回 3 条（包括已软删除的）
    let with_deleted = Model::find_with_deleted(&session)
        .await
        .expect("find_with_deleted should succeed");
    assert_eq!(
        with_deleted.len(),
        3,
        "find_with_deleted should include soft-deleted records"
    );

    // find_only_deleted 返回 1 条（仅已软删除的 id=2）
    let only_deleted = Model::find_only_deleted(&session)
        .await
        .expect("find_only_deleted should succeed");
    assert_eq!(only_deleted.len(), 1, "should have 1 soft-deleted record");
    assert_eq!(only_deleted[0].id, 2, "soft-deleted record should be id=2");
    assert!(
        only_deleted[0].deleted_at.is_some(),
        "soft-deleted record should have deleted_at set"
    );
}

/// Task 6.10: find_by_id 不返回已软删除记录
#[tokio::test]
async fn test_find_by_id_excludes_soft_deleted() {
    let pool = setup_with_seed().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 初始 find_by_id(2) 返回 Some
    let found = Model::find_by_id(&session, 2).await.expect("find_by_id should succeed");
    assert!(found.is_some(), "record 2 should exist before soft delete");

    // 软删除 id=2
    Model::delete(&session, 2).await.expect("delete should succeed");

    // find_by_id(2) 现在返回 None（已软删除）
    let found = Model::find_by_id(&session, 2).await.expect("find_by_id should succeed");
    assert!(found.is_none(), "find_by_id should return None for soft-deleted record");

    // find_by_id(1) 仍然返回 Some（未软删除）
    let found = Model::find_by_id(&session, 1).await.expect("find_by_id should succeed");
    assert!(found.is_some(), "record 1 should still exist");
}

/// Task 6.11: delete 软删除，force_delete 物理删除
#[tokio::test]
async fn test_delete_soft_force_delete_physical() {
    let pool = setup_with_seed().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 软删除 id=1
    let affected = Model::delete(&session, 1).await.expect("soft delete should succeed");
    assert_eq!(affected, 1, "should soft-delete 1 record");

    // 验证记录仍在数据库中（deleted_at 已设置）
    let with_deleted = Model::find_with_deleted(&session)
        .await
        .expect("find_with_deleted should succeed");
    assert_eq!(with_deleted.len(), 3, "record should still exist in DB");
    let deleted_record = with_deleted.iter().find(|m| m.id == 1).expect("record 1 should exist");
    assert!(
        deleted_record.deleted_at.is_some(),
        "soft-deleted record should have deleted_at set"
    );

    // force_delete id=1 — 物理删除
    let affected = Model::force_delete(&session, 1)
        .await
        .expect("force_delete should succeed");
    assert_eq!(affected, 1, "should physically delete 1 record");

    // 验证记录已从数据库中删除
    let with_deleted = Model::find_with_deleted(&session)
        .await
        .expect("find_with_deleted should succeed");
    assert_eq!(with_deleted.len(), 2, "record should be physically removed from DB");
    assert!(
        with_deleted.iter().all(|m| m.id != 1),
        "force-deleted record should not exist"
    );
}

/// Task 6.12: count 自动过滤已软删除记录
#[tokio::test]
async fn test_count_excludes_soft_deleted() {
    let pool = setup_with_seed().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 初始 count = 3
    let count = Model::count(&session).await.expect("count should succeed");
    assert_eq!(count, 3, "initial count should be 3");

    // 软删除 id=1 和 id=2
    Model::delete(&session, 1).await.expect("delete should succeed");
    Model::delete(&session, 2).await.expect("delete should succeed");

    // count 现在应该 = 1（排除 2 条已软删除）
    let count = Model::count(&session).await.expect("count should succeed");
    assert_eq!(
        count, 1,
        "count should exclude soft-deleted records (only id=3 remains)"
    );
}

/// Task 6.10: delete_many 批量软删除
#[tokio::test]
async fn test_delete_many_soft_delete_batch() {
    let pool = setup_with_seed().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 批量软删除 id=1 和 id=3
    use sea_orm::ColumnTrait;
    let cond = sea_orm::Condition::any().add(Column::Id.eq(1)).add(Column::Id.eq(3));
    let affected = Model::delete_many(&session, cond)
        .await
        .expect("delete_many should succeed");
    assert_eq!(affected, 2, "should soft-delete 2 records");

    // find_all 仅返回 id=2
    let all = Model::find_all(&session).await.expect("find_all should succeed");
    assert_eq!(all.len(), 1, "only 1 record should remain visible");
    assert_eq!(all[0].id, 2, "remaining record should be id=2");

    // find_only_deleted 返回 2 条
    let only_deleted = Model::find_only_deleted(&session)
        .await
        .expect("find_only_deleted should succeed");
    assert_eq!(only_deleted.len(), 2, "should have 2 soft-deleted records");
}

/// Task 6.5 验证: soft_delete=true 自动注入 deleted_at 字段
///
/// 此测试通过编译验证：Model 没有显式定义 deleted_at 字段，
/// 但 soft_delete=true 时宏自动注入了该字段，代码可以访问 deleted_at。
#[tokio::test]
async fn test_auto_injected_deleted_at_field() {
    let pool = setup_with_seed().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 软删除 id=1
    Model::delete(&session, 1).await.expect("delete should succeed");

    // 通过 find_with_deleted 获取记录，验证 deleted_at 字段存在且已设置
    let with_deleted = Model::find_with_deleted(&session)
        .await
        .expect("find_with_deleted should succeed");
    let deleted = with_deleted.iter().find(|m| m.id == 1).expect("record 1 should exist");

    // 访问 deleted_at 字段 — 如果字段未注入，此行会编译失败
    assert!(
        deleted.deleted_at.is_some(),
        "auto-injected deleted_at should be set after soft delete"
    );
}
