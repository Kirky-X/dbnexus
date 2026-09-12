// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 实体事件总线 + Outbox
//!
//! 实体变更（Insert/Update/Delete）产生结构化事件，经 **Outbox 表**持久化
//! （业务写入路径就地登记，MVP：独立记录入口），由**后台投递器**轮询
//! outbox、发布到**事件总线**并标记已投递——订阅端（如索引同步）从总线
//! 消费，失败可重放（status 回退）。
//!
//! # 流程
//!
//! ```text
//! 业务写实体 ──► record_event（outbox 表，status=pending）
//!                    │
//!      OutboxDispatcher（后台/手动 dispatch_once）
//!                    ▼
//!      EntityEventBus.publish ──► 订阅者（索引同步/缓存失效/…）
//!                    ▼
//!      mark_dispatched（status=dispatched）
//! ```
//!
//! # 示例
//!
//! ```ignore
//! let store = DbOutboxStore::new(pool.clone());
//! store.ensure_table().await?;
//! let bus = Arc::new(InMemoryEntityEventBus::default());
//! let mut rx = bus.subscribe();
//! store.record(&EntityEvent::insert("users", "42")).await?;
//! OutboxDispatcher::dispatch_once(&store, &bus).await?; // rx 收到事件
//! ```

use async_trait::async_trait;
use serde_json::Value;

use crate::database::DbPool;
use crate::foundation::{DbError, DbResult};

/// 实体变更动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityAction {
    /// 插入
    Insert,
    /// 更新
    Update,
    /// 删除
    Delete,
}

impl EntityAction {
    /// 动作文本（outbox 存储形态）
    pub fn as_str(self) -> &'static str {
        match self {
            EntityAction::Insert => "insert",
            EntityAction::Update => "update",
            EntityAction::Delete => "delete",
        }
    }

    /// 从文本恢复
    pub fn from_str_raw(text: &str) -> Option<Self> {
        match text {
            "insert" => Some(EntityAction::Insert),
            "update" => Some(EntityAction::Update),
            "delete" => Some(EntityAction::Delete),
            _ => None,
        }
    }
}

/// 实体变更事件
#[derive(Debug, Clone)]
pub struct EntityEvent {
    /// 实体表名
    pub entity: String,
    /// 变更动作
    pub action: EntityAction,
    /// 实体主键（文本形态）
    pub entity_id: String,
    /// 附加负载（如变更摘要；可为 None）
    pub payload: Option<Value>,
    /// 产生时间（Unix 毫秒）
    pub occurred_at_ms: u64,
}

impl EntityEvent {
    /// 构造事件（时间戳取当前时钟）
    pub fn new(entity: &str, action: EntityAction, entity_id: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();
        Self {
            entity: entity.to_string(),
            action,
            entity_id: entity_id.to_string(),
            payload: None,
            occurred_at_ms: now,
        }
    }

    /// Insert 事件
    pub fn insert(entity: &str, entity_id: &str) -> Self {
        Self::new(entity, EntityAction::Insert, entity_id)
    }

    /// Update 事件
    pub fn update(entity: &str, entity_id: &str) -> Self {
        Self::new(entity, EntityAction::Update, entity_id)
    }

    /// Delete 事件
    pub fn delete(entity: &str, entity_id: &str) -> Self {
        Self::new(entity, EntityAction::Delete, entity_id)
    }

    /// 附加负载
    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = Some(payload);
        self
    }
}

/// 实体事件总线端口
#[async_trait]
pub trait EntityEventBus: Send + Sync {
    /// 发布事件（向全部订阅者投递）
    async fn publish(&self, event: &EntityEvent) -> DbResult<()>;
}

/// 内存事件总线：mpsc 每订阅者一通道（MVP）
#[derive(Default)]
pub struct InMemoryEntityEventBus {
    senders: tokio::sync::Mutex<Vec<tokio::sync::mpsc::UnboundedSender<EntityEvent>>>,
}

impl InMemoryEntityEventBus {
    /// 订阅事件流
    pub async fn subscribe(&self) -> tokio::sync::mpsc::UnboundedReceiver<EntityEvent> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.senders.lock().await.push(tx);
        rx
    }
}

#[async_trait]
impl EntityEventBus for InMemoryEntityEventBus {
    async fn publish(&self, event: &EntityEvent) -> DbResult<()> {
        let mut senders = self.senders.lock().await;
        // 清理已关闭的订阅端
        senders.retain(|tx| !tx.is_closed());
        for tx in senders.iter() {
            tx.send(event.clone())
                .map_err(|_| DbError::Config("entity event subscriber dropped".to_string()))?;
        }
        Ok(())
    }
}

/// Outbox 存储端口
#[async_trait]
pub trait OutboxStore: Send + Sync {
    /// 登记（业务侧事务内写入的落点）
    async fn record(&self, event: &EntityEvent) -> DbResult<u64>;
    /// 拉取待投递事件（按登记顺序，至多 `limit` 条）
    async fn fetch_pending(&self, limit: u64) -> DbResult<Vec<(u64, EntityEvent)>>;
    /// 标记已投递
    async fn mark_dispatched(&self, id: u64) -> DbResult<()>;
}

/// 基于 DbPool 的 outbox 表实现
#[derive(Clone)]
pub struct DbOutboxStore {
    pool: std::sync::Arc<DbPool>,
    table: String,
}

impl DbOutboxStore {
    /// 建 outbox 表（幂等 DDL）
    ///
    /// # Errors
    ///
    /// DDL 执行失败（连接/权限）时返回 `DbError`
    pub async fn ensure_table(&self) -> DbResult<()> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, entity TEXT NOT NULL, action TEXT NOT NULL, entity_id TEXT NOT NULL, payload TEXT, status TEXT NOT NULL DEFAULT 'pending', created_at INTEGER)",
            self.table
        );
        let session = self.pool.get_session("admin").await?;
        session.execute_raw_ddl(&sql).await?;
        Ok(())
    }

    /// 创建存储（默认 outbox 表名 `dbnexus_outbox`）
    ///
    /// # Errors
    ///
    /// 表名不是安全标识符时返回 `DbError::Config`
    pub fn new(pool: std::sync::Arc<DbPool>) -> DbResult<Self> {
        Self::with_table(pool, "dbnexus_outbox")
    }

    /// 自定义 outbox 表名
    ///
    /// # Errors
    ///
    /// 表名不是安全标识符时返回 `DbError::Config`
    pub fn with_table(pool: std::sync::Arc<DbPool>, table: &str) -> DbResult<Self> {
        if crate::database::repository::is_safe_identifier(table) {
            Ok(Self {
                pool,
                table: table.to_string(),
            })
        } else {
            Err(DbError::Config(format!(
                "outbox table name must be a safe identifier: '{table}'"
            )))
        }
    }

    fn sql_value(v: &Value) -> String {
        crate::database::repository::sql_literal(v).unwrap_or_else(|_| "NULL".to_string())
    }
}

#[async_trait]
impl OutboxStore for DbOutboxStore {
    async fn record(&self, event: &EntityEvent) -> DbResult<u64> {
        let payload = match &event.payload {
            Some(v) => Self::sql_value(v),
            None => "NULL".to_string(),
        };
        // entity/entity_id 为调用方提供的自由文本，经 sql_literal 转义后进语句
        // （单引号加倍），杜绝拼接注入
        let entity = Self::sql_value(&Value::String(event.entity.clone()));
        let entity_id = Self::sql_value(&Value::String(event.entity_id.clone()));
        let sql = format!(
            "INSERT INTO {} (entity, action, entity_id, payload, status, created_at) VALUES ({}, '{}', {}, {}, 'pending', {})",
            self.table, entity, event.action.as_str(), entity_id, payload, event.occurred_at_ms
        );
        let session = self.pool.get_session("admin").await?;
        let exec = session.execute_raw(&sql).await?;
        Ok(exec.last_insert_id() as u64)
    }

    async fn fetch_pending(&self, limit: u64) -> DbResult<Vec<(u64, EntityEvent)>> {
        let sql = format!(
            "SELECT id, entity, action, entity_id, payload, created_at FROM {} WHERE status = 'pending' ORDER BY id LIMIT {}",
            self.table, limit
        );
        let rows = self.pool.query_rows(&sql, "admin").await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id = row.get("id").and_then(|v| v.as_u64()).unwrap_or_default();
            let Some(entity) = row.get("entity").and_then(|v| v.as_str().map(String::from)) else {
                continue;
            };
            let Some(action_text) = row.get("action").and_then(|v| v.as_str().map(String::from))
            else {
                continue;
            };
            let Some(action) = EntityAction::from_str_raw(&action_text) else {
                continue;
            };
            let Some(entity_id) = row.get("entity_id").and_then(|v| v.as_str().map(String::from))
            else {
                continue;
            };
            // payload 以 JSON 文本存储：能解析则还原为结构化 Value，否则保留原文本
            let payload: Option<Value> = match row
                .get("payload")
                .and_then(|v| v.as_str().map(String::from))
            {
                Some(text) => {
                    Some(serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text)))
                }
                None => None,
            };
            let occurred_at_ms = row
                .get("created_at")
                .and_then(|v| v.as_u64())
                .unwrap_or_default();
            out.push((
                id,
                EntityEvent {
                    entity,
                    action,
                    entity_id,
                    payload,
                    occurred_at_ms,
                },
            ));
        }
        Ok(out)
    }

    async fn mark_dispatched(&self, id: u64) -> DbResult<()> {
        let sql = format!(
            "UPDATE {} SET status = 'dispatched' WHERE id = {}",
            self.table, id
        );
        let session = self.pool.get_session("admin").await?;
        session.execute_raw(&sql).await?;
        Ok(())
    }
}

/// Outbox 后台投递器
pub struct OutboxDispatcher;

impl OutboxDispatcher {
    /// 单轮投递：拉取 pending → 发布到总线 → 标记 dispatched
    pub async fn dispatch_once(
        store: &dyn OutboxStore,
        bus: &dyn EntityEventBus,
        limit: u64,
    ) -> DbResult<u64> {
        let pending = store.fetch_pending(limit).await?;
        let mut dispatched = 0u64;
        for (id, event) in pending {
            bus.publish(&event).await?;
            store.mark_dispatched(id).await?;
            dispatched += 1;
        }
        Ok(dispatched)
    }

    /// 启动后台投递任务（按 `interval_ms` 轮询；`shutdown` 触发后退出）
    ///
    /// 返回 tokio JoinHandle；错误经日志吞没（投递为最终一致语义，
    /// 未投递事件仍在 outbox 中等待下轮）。
    pub fn spawn(
        store: std::sync::Arc<dyn OutboxStore>,
        bus: std::sync::Arc<dyn EntityEventBus>,
        interval_ms: u64,
        batch: u64,
        shutdown: std::sync::Arc<tokio::sync::Notify>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let _ = Self::dispatch_once(store.as_ref(), bus.as_ref(), batch).await;
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(interval_ms)) => {}
                    _ = shutdown.notified() => break,
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 事件动作文本往返
    #[test]
    fn test_entity_action_roundtrip() {
        for action in [
            EntityAction::Insert,
            EntityAction::Update,
            EntityAction::Delete,
        ] {
            assert_eq!(EntityAction::from_str_raw(action.as_str()), Some(action));
        }
        assert_eq!(EntityAction::from_str_raw("bogus"), None);
    }
}
