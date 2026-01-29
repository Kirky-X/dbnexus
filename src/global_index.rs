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

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue, Database, QueryOrder, QuerySelect};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 同步状态：待同步
pub const SYNC_STATUS_PENDING: &str = "pending";
/// 同步状态：已同步
pub const SYNC_STATUS_SYNCED: &str = "synced";
/// 同步状态：同步失败
pub const SYNC_STATUS_FAILED: &str = "failed";

/// 全局索引条目实体
///
/// 使用 sea-orm 2.0 的新实体格式
#[sea_orm::model]
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
    sync_cache: Arc<RwLock<HashMap<String, Vec<IndexEntry>>>>,
}

impl GlobalIndex {
    /// 创建新的全局索引管理器
    pub async fn new(db_url: &str) -> Result<Self, sea_orm::DbErr> {
        let db = Database::connect(db_url).await?;
        Ok(Self {
            pool: Arc::new(db),
            sync_cache: Arc::new(RwLock::new(HashMap::new())),
        })
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

    /// 批量同步索引条目
    pub async fn batch_sync(&self, entries: Vec<IndexEntry>) -> Result<SyncResult, sea_orm::DbErr> {
        let mut success = 0;
        let mut failed = 0;
        let mut errors = Vec::new();

        for entry in entries {
            let result = self.sync_entry(&entry).await;
            match result {
                Ok(_) => success += 1,
                Err(e) => {
                    failed += 1;
                    errors.push(format!("Failed to sync entry: {:?}", e));
                }
            }
        }

        Ok(SyncResult {
            success: failed == 0,
            synced_count: success,
            failed_count: failed,
            errors,
        })
    }

    /// 同步单个索引条目
    async fn sync_entry(&self, entry: &IndexEntry) -> Result<(), sea_orm::DbErr> {
        let model = ActiveModel {
            id: Set(entry.index_value.clone()),
            table_name: Set(entry.table_name.clone()),
            record_id: Set(entry.record_id.clone()),
            shard_id: Set(entry.shard_id as i32),
            index_key: Set(entry.index_key.clone()),
            index_value: Set(entry.index_value.clone()),
            created_at: Set(chrono::Utc::now().to_rfc3339()),
            updated_at: Set(chrono::Utc::now().to_rfc3339()),
            last_modified: Set(chrono::Utc::now().to_rfc3339()),
            sync_status: Set(SYNC_STATUS_SYNCED.to_string()),
            ..Default::default()
        };

        Entity::insert(model)
            .on_conflict(
                sea_orm::OnConflict::columns([Column::Id])
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
            .await?;

        Ok(())
    }
}
