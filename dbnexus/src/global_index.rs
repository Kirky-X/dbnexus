// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 全局索引表模块
//!
//! 提供跨分片的全局索引功能，支持：
//! - 异步同步分片数据到全局索引
//! - 不带时间条件的查询
//! - binlog/CDC 风格的变更捕获
//!
//! # Example
//!
//! ```ignore
//! use dbnexus::global_index::{GlobalIndex, IndexEntry};
//!
//! let index = GlobalIndex::new("sqlite:./global_index.db").await?;
//! // 查询所有分片中的数据
//! let entries = index.query_all("orders", "user_id = ?", &[&"user123"]).await?;
//! ```

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
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "global_index")]
pub struct Model {
    /// 唯一标识符
    #[sea_orm(primary_key)]
    pub id: String,
    /// 表名
    pub table_name: String,
    /// 原始记录ID
    pub record_id: String,
    /// 所在分片ID
    pub shard_id: i32,
    /// 索引键（如 user_id）
    pub index_key: String,
    /// 索引值
    pub index_value: String,
    /// 创建时间
    pub created_at: String,
    /// 更新时间
    pub updated_at: String,
    /// 最后修改时间（用于 CDC 增量查询）
    pub last_modified: String,
    /// 同步状态
    pub sync_status: String,
}

/// 实体关系枚举
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// 索引条目结构
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// 表名
    pub table_name: String,
    /// 记录ID
    pub record_id: String,
    /// 分片ID
    pub shard_id: u32,
    /// 索引键
    pub index_key: String,
    /// 索引值
    pub index_value: String,
}

/// 同步事件类型
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// 插入事件
    Insert {
        /// 表名
        table_name: String,
        /// 记录ID
        record_id: String,
        /// 分片ID
        shard_id: u32,
        /// 索引键
        index_key: String,
        /// 索引值
        index_value: String,
    },
    /// 更新事件
    Update {
        /// 表名
        table_name: String,
        /// 记录ID
        record_id: String,
        /// 分片ID
        shard_id: u32,
        /// 旧索引键
        old_index_key: String,
        /// 旧索引值
        old_index_value: String,
        /// 新索引键
        new_index_key: String,
        /// 新索引值
        new_index_value: String,
    },
    /// 删除事件
    Delete {
        /// 表名
        table_name: String,
        /// 记录ID
        record_id: String,
        /// 分片ID
        shard_id: u32,
        /// 索引键
        index_key: String,
        /// 索引值
        index_value: String,
    },
}

/// 变更捕获配置
#[derive(Debug, Clone)]
pub struct ChangeCaptureConfig {
    /// 批量处理大小
    pub batch_size: usize,
    /// 处理间隔（毫秒）
    pub poll_interval_ms: u64,
    /// 重试次数
    pub max_retries: u32,
    /// 重试间隔（毫秒）
    pub retry_interval_ms: u64,
}

impl Default for ChangeCaptureConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            poll_interval_ms: 1000,
            max_retries: 3,
            retry_interval_ms: 5000,
        }
    }
}

/// 索引缓存类型
type IndexCache = HashMap<String, HashMap<String, HashMap<String, Vec<IndexEntry>>>>;

/// 全局索引管理器
#[derive(Debug)]
pub struct GlobalIndex {
    /// 数据库连接
    conn: DatabaseConnection,
    /// 缓存的索引数据
    cache: Arc<RwLock<IndexCache>>,
    /// 配置
    config: ChangeCaptureConfig,
}

impl GlobalIndex {
    /// 创建新的全局索引管理器
    pub async fn new(database_url: &str) -> Result<Self, DbErr> {
        let conn = Database::connect(database_url).await?;

        // 简单起见，使用 migrations
        Self::init_schema(&conn).await?;

        Ok(Self {
            conn,
            cache: Arc::new(RwLock::new(HashMap::new())),
            config: ChangeCaptureConfig::default(),
        })
    }

    /// 初始化数据库 schema
    async fn init_schema(conn: &DatabaseConnection) -> Result<(), DbErr> {
        // 创建全局索引表
        let create_sql = r#"
        CREATE TABLE IF NOT EXISTS global_index (
            id VARCHAR(64) PRIMARY KEY,
            table_name VARCHAR(128) NOT NULL,
            record_id VARCHAR(128) NOT NULL,
            shard_id INTEGER NOT NULL,
            index_key VARCHAR(128) NOT NULL,
            index_value VARCHAR(512) NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
            last_modified VARCHAR(32) NOT NULL,
            sync_status VARCHAR(20) DEFAULT 'synced',
            CONSTRAINT uk_table_record UNIQUE (table_name, record_id)
        );
        
        CREATE INDEX IF NOT EXISTS idx_global_index_key ON global_index (table_name, index_key, index_value);
        CREATE INDEX IF NOT EXISTS idx_global_index_shard ON global_index (table_name, shard_id);
        CREATE INDEX IF NOT EXISTS idx_global_index_modified ON global_index (table_name, last_modified);
        "#;

        conn.execute_unprepared(create_sql).await?;
        Ok(())
    }

    /// 获取数据库连接
    pub fn get_connection(&self) -> &DatabaseConnection {
        &self.conn
    }

    /// 注册索引条目
    pub async fn register_entry(&self, entry: IndexEntry) -> Result<(), DbErr> {
        let id = Self::generate_id(&entry.table_name, &entry.record_id);
        let now = chrono::Utc::now().to_rfc3339();
        let now_clone = now.clone();

        let active = ActiveModel {
            id: ActiveValue::Set(id),
            table_name: ActiveValue::Set(entry.table_name.clone()),
            record_id: ActiveValue::Set(entry.record_id.clone()),
            shard_id: ActiveValue::Set(entry.shard_id as i32),
            index_key: ActiveValue::Set(entry.index_key.clone()),
            index_value: ActiveValue::Set(entry.index_value.clone()),
            created_at: ActiveValue::Set(now_clone),
            updated_at: ActiveValue::Set(now.clone()),
            last_modified: ActiveValue::Set(now),
            sync_status: ActiveValue::Set(SYNC_STATUS_SYNCED.to_string()),
        };

        Entity::insert(active).exec(&self.conn).await?;

        // 更新缓存
        self.update_cache(&entry).await;
        Ok(())
    }

    /// 批量注册索引条目
    pub async fn register_entries(&self, entries: Vec<IndexEntry>) -> Result<(), DbErr> {
        let now = chrono::Utc::now().to_rfc3339();
        let sync_status = SYNC_STATUS_SYNCED.to_string();
        let now_clone = now.clone();

        let active_models: Vec<ActiveModel> = entries
            .iter()
            .map(|entry| {
                let id = Self::generate_id(&entry.table_name, &entry.record_id);
                ActiveModel {
                    id: ActiveValue::Set(id),
                    table_name: ActiveValue::Set(entry.table_name.clone()),
                    record_id: ActiveValue::Set(entry.record_id.clone()),
                    shard_id: ActiveValue::Set(entry.shard_id as i32),
                    index_key: ActiveValue::Set(entry.index_key.clone()),
                    index_value: ActiveValue::Set(entry.index_value.clone()),
                    created_at: ActiveValue::Set(now_clone.clone()),
                    updated_at: ActiveValue::Set(now.clone()),
                    last_modified: ActiveValue::Set(now.clone()),
                    sync_status: ActiveValue::Set(sync_status.clone()),
                }
            })
            .collect();

        Entity::insert_many(active_models).exec(&self.conn).await?;

        // 更新缓存
        for entry in entries {
            self.update_cache(&entry).await;
        }

        Ok(())
    }

    /// 根据索引键查询
    pub async fn query_by_index(
        &self,
        table_name: &str,
        index_key: &str,
        index_value: &str,
    ) -> Result<Vec<IndexEntry>, DbErr> {
        // 先查缓存
        {
            let cache = self.cache.read().await;
            if let Some(table_cache) = cache.get(table_name) {
                if let Some(key_cache) = table_cache.get(index_key) {
                    if let Some(entries) = key_cache.get(index_value) {
                        return Ok(entries.clone());
                    }
                }
            }
        }

        // 缓存未命中，从数据库查询
        let result = Entity::find()
            .filter(Column::TableName.eq(table_name))
            .filter(Column::IndexKey.eq(index_key))
            .filter(Column::IndexValue.eq(index_value))
            .all(&self.conn)
            .await?;

        let entries: Vec<IndexEntry> = result
            .iter()
            .map(|m| IndexEntry {
                table_name: m.table_name.clone(),
                record_id: m.record_id.clone(),
                shard_id: m.shard_id as u32,
                index_key: m.index_key.clone(),
                index_value: m.index_value.clone(),
            })
            .collect();

        // 更新缓存
        for entry in &entries {
            self.update_cache(entry).await;
        }

        Ok(entries)
    }

    /// 查询所有分片的记录
    pub async fn query_all_shards(&self, table_name: &str, index_key: &str) -> Result<Vec<IndexEntry>, DbErr> {
        let result = Entity::find()
            .filter(Column::TableName.eq(table_name))
            .filter(Column::IndexKey.eq(index_key))
            .all(&self.conn)
            .await?;

        Ok(result
            .iter()
            .map(|m| IndexEntry {
                table_name: m.table_name.clone(),
                record_id: m.record_id.clone(),
                shard_id: m.shard_id as u32,
                index_key: m.index_key.clone(),
                index_value: m.index_value.clone(),
            })
            .collect())
    }

    /// 处理同步事件
    pub async fn process_sync_event(&self, event: SyncEvent) -> Result<(), DbErr> {
        match event {
            SyncEvent::Insert {
                table_name,
                record_id,
                shard_id,
                index_key,
                index_value,
            } => {
                let entry = IndexEntry {
                    table_name,
                    record_id,
                    shard_id,
                    index_key,
                    index_value,
                };
                self.register_entry(entry).await?;
            }
            SyncEvent::Update {
                table_name,
                record_id,
                shard_id,
                old_index_key: _,
                old_index_value: _,
                new_index_key,
                new_index_value,
            } => {
                // 删除旧索引
                self.delete_entry(&table_name, &record_id).await?;

                // 注册新索引
                let entry = IndexEntry {
                    table_name,
                    record_id,
                    shard_id,
                    index_key: new_index_key,
                    index_value: new_index_value,
                };
                self.register_entry(entry).await?;
            }
            SyncEvent::Delete {
                table_name, record_id, ..
            } => {
                self.delete_entry(&table_name, &record_id).await?;
            }
        }
        Ok(())
    }

    /// 删除索引条目
    async fn delete_entry(&self, table_name: &str, record_id: &str) -> Result<(), DbErr> {
        let id = Self::generate_id(table_name, record_id);

        Entity::delete_by_id(id).exec(&self.conn).await?;

        // 从缓存中移除
        let mut cache = self.cache.write().await;
        if let Some(table_cache) = cache.get_mut(table_name) {
            for key_cache in table_cache.values_mut() {
                key_cache.retain(|_key, entries| {
                    entries.retain(|e| e.record_id != record_id);
                    !entries.is_empty()
                });
            }
        }

        Ok(())
    }

    /// 生成唯一ID
    fn generate_id(table_name: &str, record_id: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::default();
        hasher.update(format!("{}:{}", table_name, record_id));
        format!("{:x}", hasher.finalize())
    }

    /// 更新缓存
    async fn update_cache(&self, entry: &IndexEntry) {
        let mut cache = self.cache.write().await;
        let table_cache = cache.entry(entry.table_name.clone()).or_insert_with(HashMap::new);
        let key_cache = table_cache.entry(entry.index_key.clone()).or_insert_with(HashMap::new);

        let entries = key_cache.entry(entry.index_value.clone()).or_insert_with(Vec::new);

        // 检查是否已存在
        if !entries.iter().any(|e| e.record_id == entry.record_id) {
            entries.push(entry.clone());
        }
    }

    /// 获取配置
    pub fn get_config(&self) -> &ChangeCaptureConfig {
        &self.config
    }

    /// 设置配置
    pub fn set_config(&mut self, config: ChangeCaptureConfig) {
        self.config = config;
    }
}

/// Binlog/CDC 变更捕获 trait
///
/// 提供数据库变更捕获接口，支持：
/// - INSERT 事件检测
/// - UPDATE 事件检测（包含旧值和新值）
/// - DELETE 事件检测
#[async_trait]
pub trait ChangeCapture: Send + Sync {
    /// 初始化变更捕获
    async fn start(&mut self) -> Result<(), DbErr>;

    /// 停止变更捕获
    async fn stop(&mut self) -> Result<(), DbErr>;

    /// 获取下一个变更事件
    async fn next_event(&mut self) -> Option<SyncEvent>;

    /// 检查是否正在运行
    fn is_running(&self) -> bool;
}

/// 轮询变更捕获配置
#[derive(Debug, Clone)]
pub struct PollingCaptureConfig {
    /// 轮询间隔（毫秒）
    pub interval_ms: u64,
    /// 批量获取大小
    pub batch_size: usize,
    /// 监控的表列表（空列表表示监控所有表）
    pub watched_tables: Vec<String>,
}

impl Default for PollingCaptureConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            batch_size: 100,
            watched_tables: Vec::new(),
        }
    }
}

/// 轮询变更捕获实现
///
/// 通过定期查询变更追踪表来捕获数据库变更
#[derive(Debug)]
pub struct PollingChangeCapture {
    /// 配置
    config: PollingCaptureConfig,
    /// 全局索引引用
    global_index: Arc<GlobalIndex>,
    /// 运行状态
    running: bool,
    /// 最后轮询时间
    last_poll: RwLock<DateTime<Utc>>,
    /// 待处理的变更事件队列
    event_queue: RwLock<Vec<SyncEvent>>,
}

impl PollingChangeCapture {
    /// 创建新的轮询变更捕获
    pub fn new(global_index: Arc<GlobalIndex>, config: Option<PollingCaptureConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
            global_index,
            running: false,
            last_poll: RwLock::new(Utc::now()),
            event_queue: RwLock::new(Vec::new()),
        }
    }

    /// 获取自上次轮询以来修改的条目
    async fn fetch_changes(&self) -> Result<Vec<Model>, DbErr> {
        let last_poll_time = *self.last_poll.read().await;

        let mut query = Entity::find().filter(Column::LastModified.gt(last_poll_time.to_rfc3339()));

        // 如果配置了监控表列表，只查询这些表
        if !self.config.watched_tables.is_empty() {
            query = query.filter(Column::TableName.is_in(&self.config.watched_tables));
        }

        // 按时间排序，获取最早的变更
        query = query.order_by(Column::LastModified, sea_orm::Order::Asc);

        // 限制批量大小
        query = query.limit(self.config.batch_size as u64);

        let results = query.all(self.global_index.get_connection()).await?;

        // 更新最后轮询时间
        if let Some(latest) = results.last() {
            let latest_modified = DateTime::parse_from_rfc3339(&latest.last_modified)
                .map(|dt| dt.into())
                .unwrap_or_else(|_| Utc::now());
            let mut last_poll = self.last_poll.write().await;
            if latest_modified > *last_poll {
                *last_poll = latest_modified;
            }
        }

        Ok(results)
    }

    /// 将模型转换为同步事件
    #[allow(clippy::unused_self)]
    fn model_to_event(model: &Model, _previous_value: Option<&Model>) -> Option<SyncEvent> {
        // 简单实现：假设所有变更都是 INSERT
        // 实际实现需要比较前后来确定是 INSERT/UPDATE/DELETE
        Some(SyncEvent::Insert {
            table_name: model.table_name.clone(),
            record_id: model.record_id.clone(),
            shard_id: model.shard_id as u32,
            index_key: model.index_key.clone(),
            index_value: model.index_value.clone(),
        })
    }
}

#[async_trait]
impl ChangeCapture for PollingChangeCapture {
    async fn start(&mut self) -> Result<(), DbErr> {
        self.running = true;
        *self.last_poll.write().await = Utc::now();
        tracing::info!(
            "PollingChangeCapture started with interval {}ms",
            self.config.interval_ms
        );
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), DbErr> {
        self.running = false;
        tracing::info!("PollingChangeCapture stopped");
        Ok(())
    }

    async fn next_event(&mut self) -> Option<SyncEvent> {
        if !self.running {
            return None;
        }

        // 首先检查是否有缓存的事件
        {
            let mut queue = self.event_queue.write().await;
            if let Some(event) = queue.pop() {
                return Some(event);
            }
        }

        // 获取新的变更
        match self.fetch_changes().await {
            Ok(changes) => {
                for change in changes {
                    if let Some(event) = Self::model_to_event(&change, None) {
                        let mut queue = self.event_queue.write().await;
                        queue.push(event);
                    }
                }

                // 返回第一个事件
                let mut queue = self.event_queue.write().await;
                queue.pop()
            }
            Err(e) => {
                tracing::error!("Failed to fetch changes: {}", e);
                None
            }
        }
    }

    fn is_running(&self) -> bool {
        self.running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id1 = GlobalIndex::generate_id("orders", "order_123");
        let id2 = GlobalIndex::generate_id("orders", "order_123");
        let id3 = GlobalIndex::generate_id("orders", "order_456");

        // 相同输入应生成相同ID
        assert_eq!(id1, id2);
        // 不同输入应生成不同ID
        assert_ne!(id1, id3);
        // ID 应该是 64 字符的十六进制字符串 (SHA256)
        assert_eq!(id1.len(), 64);
    }

    #[test]
    fn test_index_entry() {
        let entry = IndexEntry {
            table_name: "orders".to_string(),
            record_id: "order_123".to_string(),
            shard_id: 4,
            index_key: "user_id".to_string(),
            index_value: "user_456".to_string(),
        };

        assert_eq!(entry.table_name, "orders");
        assert_eq!(entry.shard_id, 4);
    }

    #[test]
    fn test_sync_event_variants() {
        let insert = SyncEvent::Insert {
            table_name: "orders".to_string(),
            record_id: "order_123".to_string(),
            shard_id: 4,
            index_key: "user_id".to_string(),
            index_value: "user_456".to_string(),
        };

        let update = SyncEvent::Update {
            table_name: "orders".to_string(),
            record_id: "order_123".to_string(),
            shard_id: 4,
            old_index_key: "user_id".to_string(),
            old_index_value: "user_456".to_string(),
            new_index_key: "user_id".to_string(),
            new_index_value: "user_789".to_string(),
        };

        let delete = SyncEvent::Delete {
            table_name: "orders".to_string(),
            record_id: "order_123".to_string(),
            shard_id: 4,
            index_key: "user_id".to_string(),
            index_value: "user_456".to_string(),
        };

        match insert {
            SyncEvent::Insert { table_name, .. } => assert_eq!(table_name, "orders"),
            _ => panic!("Expected Insert variant"),
        }

        match update {
            SyncEvent::Update { new_index_value, .. } => assert_eq!(new_index_value, "user_789"),
            _ => panic!("Expected Update variant"),
        }

        match delete {
            SyncEvent::Delete { record_id, .. } => assert_eq!(record_id, "order_123"),
            _ => panic!("Expected Delete variant"),
        }
    }

    #[test]
    fn test_change_capture_config_defaults() {
        let config = ChangeCaptureConfig::default();

        assert_eq!(config.batch_size, 1000);
        assert_eq!(config.poll_interval_ms, 1000);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_interval_ms, 5000);
    }

    #[test]
    fn test_sync_status_constants() {
        assert_eq!(SYNC_STATUS_PENDING, "pending");
        assert_eq!(SYNC_STATUS_SYNCED, "synced");
        assert_eq!(SYNC_STATUS_FAILED, "failed");
    }
}
