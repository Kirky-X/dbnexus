// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 全局索引表模块
//!
//! 提供跨分片的全局索引功能，支持：
//! - 异步同步分片数据到全局索引
//! - 不带时间条件的查询
//! - binlog/CDC 风格的变更捕获

#![allow(missing_docs)]

use sea_orm::ActiveValue::Set;
use sea_orm::Database;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::OnConflict;
use std::sync::Arc;

/// 同步状态：待同步
pub const SYNC_STATUS_PENDING: &str = "pending";
/// 同步状态：已同步
pub const SYNC_STATUS_SYNCED: &str = "synced";
/// 同步状态：同步失败
pub const SYNC_STATUS_FAILED: &str = "failed";

/// batch_sync 的默认分块大小
const BATCH_SYNC_CHUNK_SIZE: usize = 500;

/// 全局索引条目实体
///
/// 使用 sea-orm 2.0 的新实体格式
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "global_index")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub table_name: String,
    pub record_id: String,
    pub shard_id: i32,
    pub index_key: String,
    pub index_value: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_modified: String,
    pub sync_status: String,
}

impl sea_orm::ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

/// 索引条目结构
#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub table_name: String,
    pub record_id: String,
    pub shard_id: u32,
    pub index_key: String,
    pub index_value: String,
}

/// 同步事件类型
#[derive(Debug, Clone)]
pub enum SyncEvent {
    Insert(IndexEntry),
    Update(IndexEntry),
    Delete(IndexEntry),
}

/// 同步结果
#[derive(Debug)]
pub struct SyncResult {
    pub success: bool,
    pub synced_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

/// 全局索引管理器
pub struct GlobalIndex {
    pool: Arc<sea_orm::DbConn>,
}

impl GlobalIndex {
    /// 创建新的全局索引管理器
    pub async fn new(db_url: &str) -> Result<Self, sea_orm::DbErr> {
        let db = Database::connect(db_url).await?;

        Self::create_table_if_not_exists(&db).await?;

        Ok(Self { pool: Arc::new(db) })
    }

    /// 创建全局索引表（如果不存在）
    async fn create_table_if_not_exists(db: &sea_orm::DbConn) -> Result<(), sea_orm::DbErr> {
        use sea_orm::Schema;

        let builder = db.get_database_backend();
        let schema = Schema::new(builder);

        let mut create_table_stmt = schema.create_table_from_entity(Entity);
        create_table_stmt.if_not_exists();

        db.execute(&create_table_stmt).await?;

        Ok(())
    }

    /// 通过索引查询记录
    pub async fn query_by_index(
        &self,
        table_name: &str,
        index_key: &str,
        index_value: &str,
    ) -> Result<Vec<IndexEntry>, sea_orm::DbErr> {
        let results = Entity::find()
            .filter(Column::TableName.eq(table_name))
            .filter(Column::IndexKey.eq(index_key))
            .filter(Column::IndexValue.eq(index_value))
            .all(&*self.pool)
            .await?;

        Ok(results
            .into_iter()
            .map(|m| IndexEntry {
                table_name: m.table_name,
                record_id: m.record_id,
                shard_id: m.shard_id as u32,
                index_key: m.index_key,
                index_value: m.index_value,
            })
            .collect())
    }

    /// 批量同步索引条目（分批插入以避免 SQLite 参数限制）
    ///
    /// 将大量条目分成每批最多 500 条进行插入，配合 `OnConflict` 实现 upsert。
    /// 即使部分批次失败，已成功的批次仍会持久化（部分成功语义）。
    pub async fn batch_sync(&self, entries: Vec<IndexEntry>) -> Result<SyncResult, sea_orm::DbErr> {
        if entries.is_empty() {
            return Ok(SyncResult {
                success: true,
                synced_count: 0,
                failed_count: 0,
                errors: Vec::new(),
            });
        }

        let total = entries.len();
        let (total_synced, all_errors) = self.chunk_and_upsert(&entries, BATCH_SYNC_CHUNK_SIZE).await;

        let failed_count = total.saturating_sub(total_synced);
        Ok(SyncResult {
            success: all_errors.is_empty(),
            synced_count: total_synced,
            failed_count,
            errors: all_errors,
        })
    }

    /// 分块执行 upsert，返回 (已同步条数, 错误列表)
    ///
    /// 即使部分批次失败也会继续处理后续批次（部分成功语义），
    /// 失败信息累积到返回的错误列表中。
    async fn chunk_and_upsert(&self, entries: &[IndexEntry], chunk_size: usize) -> (usize, Vec<String>) {
        let mut total_synced = 0usize;
        let mut all_errors: Vec<String> = Vec::new();

        for (batch_idx, chunk) in entries.chunks(chunk_size).enumerate() {
            let active_models: Vec<ActiveModel> = chunk
                .iter()
                .map(|entry| {
                    let now = chrono::Utc::now().to_rfc3339();
                    let id = format!("{}:{}:{}", entry.table_name, entry.index_key, entry.record_id);
                    ActiveModel {
                        id: Set(id),
                        table_name: Set(entry.table_name.clone()),
                        record_id: Set(entry.record_id.clone()),
                        shard_id: Set(entry.shard_id as i32),
                        index_key: Set(entry.index_key.clone()),
                        index_value: Set(entry.index_value.clone()),
                        created_at: Set(now.clone()),
                        updated_at: Set(now.clone()),
                        last_modified: Set(now),
                        sync_status: Set(SYNC_STATUS_SYNCED.to_string()),
                    }
                })
                .collect();

            match Entity::insert_many(active_models)
                .on_conflict(
                    OnConflict::columns([Column::Id])
                        .update_columns([
                            Column::TableName,
                            Column::RecordId,
                            Column::ShardId,
                            Column::IndexKey,
                            Column::IndexValue,
                            Column::UpdatedAt,
                            Column::LastModified,
                            Column::SyncStatus,
                        ])
                        .to_owned(),
                )
                .exec(&*self.pool)
                .await
            {
                Ok(_) => {
                    total_synced += chunk.len();
                }
                Err(e) => {
                    all_errors.push(format!(
                        "Batch {} (entries {}-{}) failed: {:?}",
                        batch_idx,
                        batch_idx * chunk_size,
                        (batch_idx + 1) * chunk_size - 1,
                        e
                    ));
                }
            }
        }

        (total_synced, all_errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建基于 SQLite 内存的 GlobalIndex 实例
    async fn create_global_index() -> GlobalIndex {
        GlobalIndex::new("sqlite::memory:")
            .await
            .expect("Failed to create GlobalIndex")
    }

    fn make_entry(table: &str, record_id: &str, shard: u32, key: &str, value: &str) -> IndexEntry {
        IndexEntry {
            table_name: table.to_string(),
            record_id: record_id.to_string(),
            shard_id: shard,
            index_key: key.to_string(),
            index_value: value.to_string(),
        }
    }

    // ===== 常量测试 =====

    #[test]
    fn test_sync_status_constants() {
        assert_eq!(SYNC_STATUS_PENDING, "pending");
        assert_eq!(SYNC_STATUS_SYNCED, "synced");
        assert_eq!(SYNC_STATUS_FAILED, "failed");
    }

    // ===== IndexEntry / SyncEvent / SyncResult 结构测试 =====

    #[test]
    fn test_index_entry_construction() {
        let entry = make_entry("users", "user_1", 1, "email", "test@example.com");
        assert_eq!(entry.table_name, "users");
        assert_eq!(entry.record_id, "user_1");
        assert_eq!(entry.shard_id, 1);
        assert_eq!(entry.index_key, "email");
        assert_eq!(entry.index_value, "test@example.com");
    }

    #[test]
    fn test_sync_event_insert_variant() {
        let entry = make_entry("users", "user_1", 1, "email", "test@example.com");
        let event = SyncEvent::Insert(entry.clone());
        match event {
            SyncEvent::Insert(e) => {
                assert_eq!(e.table_name, "users");
                assert_eq!(e.record_id, "user_1");
            }
            _ => panic!("expected Insert"),
        }
    }

    #[test]
    fn test_sync_event_update_variant() {
        let entry = make_entry("users", "user_1", 1, "email", "updated@example.com");
        let event = SyncEvent::Update(entry);
        assert!(matches!(event, SyncEvent::Update(_)));
    }

    #[test]
    fn test_sync_event_delete_variant() {
        let entry = make_entry("users", "user_1", 1, "email", "test@example.com");
        let event = SyncEvent::Delete(entry);
        assert!(matches!(event, SyncEvent::Delete(_)));
    }

    #[test]
    fn test_sync_result_success_construction() {
        let result = SyncResult {
            success: true,
            synced_count: 5,
            failed_count: 0,
            errors: vec![],
        };
        assert!(result.success);
        assert_eq!(result.synced_count, 5);
        assert_eq!(result.failed_count, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_sync_result_failure_construction() {
        let result = SyncResult {
            success: false,
            synced_count: 3,
            failed_count: 2,
            errors: vec!["batch 1 failed".to_string()],
        };
        assert!(!result.success);
        assert_eq!(result.synced_count, 3);
        assert_eq!(result.failed_count, 2);
        assert_eq!(result.errors.len(), 1);
    }

    // ===== 数据库测试 =====

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_global_index_new_creates_table() {
        let index = create_global_index().await;
        // 验证 global_index 表已创建（通过查询不报错）
        let results = index.query_by_index("any", "any", "any").await;
        assert!(results.is_ok());
        assert!(results.unwrap().is_empty());
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_batch_sync_empty_returns_success() {
        let index = create_global_index().await;
        let result = index.batch_sync(vec![]).await.expect("batch_sync failed");
        assert!(result.success);
        assert_eq!(result.synced_count, 0);
        assert_eq!(result.failed_count, 0);
        assert!(result.errors.is_empty());
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_batch_sync_single_entry() {
        let index = create_global_index().await;
        let entry = make_entry("users", "user_1", 1, "email", "test@example.com");

        let result = index.batch_sync(vec![entry]).await.expect("batch_sync failed");
        assert!(result.success);
        assert_eq!(result.synced_count, 1);
        assert_eq!(result.failed_count, 0);

        // 验证条目已被持久化
        let queried = index
            .query_by_index("users", "email", "test@example.com")
            .await
            .expect("query failed");
        assert_eq!(queried.len(), 1);
        assert_eq!(queried[0].record_id, "user_1");
        assert_eq!(queried[0].shard_id, 1);
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_batch_sync_multiple_entries() {
        let index = create_global_index().await;
        let entries = vec![
            make_entry("users", "user_1", 0, "email", "a@example.com"),
            make_entry("users", "user_2", 1, "email", "b@example.com"),
            make_entry("users", "user_3", 2, "email", "c@example.com"),
        ];

        let result = index.batch_sync(entries).await.expect("batch_sync failed");
        assert!(result.success);
        assert_eq!(result.synced_count, 3);
        assert_eq!(result.failed_count, 0);

        // 验证所有条目
        let queried = index
            .query_by_index("users", "email", "a@example.com")
            .await
            .expect("query failed");
        assert_eq!(queried.len(), 1);
        assert_eq!(queried[0].record_id, "user_1");
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_batch_sync_upsert_on_conflict() {
        let index = create_global_index().await;

        // 第一次插入
        let entry_v1 = make_entry("users", "user_1", 0, "email", "old@example.com");
        index.batch_sync(vec![entry_v1]).await.expect("first sync failed");

        // 第二次插入相同 id 但不同 value（upsert）
        let entry_v2 = make_entry("users", "user_1", 1, "email", "new@example.com");
        let result = index.batch_sync(vec![entry_v2]).await.expect("second sync failed");
        assert!(result.success);
        assert_eq!(result.synced_count, 1);

        // 验证条目被更新（不是新增）
        let all = index
            .query_by_index("users", "email", "new@example.com")
            .await
            .expect("query failed");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].shard_id, 1); // shard_id 应该被更新

        // 旧值应该不存在
        let old = index
            .query_by_index("users", "email", "old@example.com")
            .await
            .expect("query failed");
        assert!(old.is_empty());
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_query_by_index_no_match() {
        let index = create_global_index().await;
        let entry = make_entry("users", "user_1", 0, "email", "test@example.com");
        index.batch_sync(vec![entry]).await.expect("sync failed");

        // 查询不存在的值
        let results = index
            .query_by_index("users", "email", "nonexistent@example.com")
            .await
            .expect("query failed");
        assert!(results.is_empty());

        // 查询不存在的表
        let results = index
            .query_by_index("orders", "email", "test@example.com")
            .await
            .expect("query failed");
        assert!(results.is_empty());

        // 查询不存在的索引键
        let results = index
            .query_by_index("users", "phone", "test@example.com")
            .await
            .expect("query failed");
        assert!(results.is_empty());
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_query_by_index_multiple_matches() {
        let index = create_global_index().await;
        let entries = vec![
            make_entry("users", "user_1", 0, "status", "active"),
            make_entry("users", "user_2", 1, "status", "active"),
            make_entry("users", "user_3", 2, "status", "inactive"),
        ];
        index.batch_sync(entries).await.expect("sync failed");

        // 查询所有 status=active 的记录
        let results = index
            .query_by_index("users", "status", "active")
            .await
            .expect("query failed");
        assert_eq!(results.len(), 2);
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_batch_sync_large_batch() {
        let index = create_global_index().await;
        // 创建 600 个条目以触发分批（chunk_size = 500）
        let entries: Vec<IndexEntry> = (0..600)
            .map(|i| make_entry("users", &format!("user_{}", i), i % 4, "id", &i.to_string()))
            .collect();

        let result = index.batch_sync(entries).await.expect("batch_sync failed");
        assert!(result.success);
        assert_eq!(result.synced_count, 600);
        assert_eq!(result.failed_count, 0);

        // 验证条目数
        let queried = index.query_by_index("users", "id", "599").await.expect("query failed");
        assert_eq!(queried.len(), 1);
        assert_eq!(queried[0].record_id, "user_599");
    }
}
