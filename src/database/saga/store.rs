// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分布式事务 Saga 编排器
//!
//! 每分片独立事务 + 补偿操作，应用层协调，无跨分片锁。

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;

use super::types::*;

/// Saga 执行日志
#[derive(Debug, Clone)]
pub struct SagaLog {
    /// Saga 唯一标识
    pub saga_id: String,
    /// 执行状态
    pub status: SagaStatus,
    /// 各步骤日志
    pub steps: Vec<SagaStepLog>,
}

/// 内存 Saga 日志存储
pub struct InMemorySagaLog {
    logs: DashMap<String, SagaLog>,
}

impl InMemorySagaLog {
    /// 创建内存日志存储
    pub fn new() -> Self {
        Self {
            logs: DashMap::new(),
        }
    }

    /// 获取指定 saga 的日志
    pub fn get(&self, saga_id: &str) -> Option<SagaLog> {
        self.logs.get(saga_id).map(|r| r.value().clone())
    }

    /// 插入日志
    pub fn insert(&self, log: SagaLog) {
        self.logs.insert(log.saga_id.clone(), log);
    }

    /// 更新 saga 状态
    pub fn update_status(&self, saga_id: &str, status: SagaStatus) {
        if let Some(mut log) = self.logs.get_mut(saga_id) {
            log.status = status;
        }
    }
}

impl Default for InMemorySagaLog {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SagaLogStore trait
// ============================================================================

/// Saga 日志存储端口
///
/// 内存实现为默认；`DbSagaLog`（`sql-parser` feature）提供基于数据库的持久化。
#[async_trait]
pub trait SagaLogStore: Send + Sync {
    /// 插入/覆盖指定 saga 的整条日志（幂等 upsert）
    async fn persist(&self, log: &SagaLog) -> Result<(), String>;

    /// 加载未终结（Running/Compensating）的 saga 日志（启动恢复用）
    async fn load_pending(&self) -> Result<Vec<SagaLog>, String>;

    /// 按 saga_id 获取日志
    async fn get(&self, saga_id: &str) -> Result<Option<SagaLog>, String>;
}

#[async_trait]
impl SagaLogStore for InMemorySagaLog {
    async fn persist(&self, log: &SagaLog) -> Result<(), String> {
        self.logs.insert(log.saga_id.clone(), log.clone());
        Ok(())
    }

    async fn load_pending(&self) -> Result<Vec<SagaLog>, String> {
        Ok(self
            .logs
            .iter()
            .map(|r| r.value().clone())
            .filter(|l| matches!(l.status, SagaStatus::Running | SagaStatus::Compensating))
            .collect())
    }

    async fn get(&self, saga_id: &str) -> Result<Option<SagaLog>, String> {
        Ok(InMemorySagaLog::get(self, saga_id))
    }
}

/// DB 持久化 Saga 日志存储（`sql-parser` feature）
///
/// 复用 dbnexus 自身的连接池执行 DDL/UPSERT，行查询经统一 API `query_rows`。
/// 步骤日志以 JSON 文本列存储，无需为公共类型引入 serde 依赖。
#[cfg(feature = "sql-parser")]
pub struct DbSagaLog {
    pool: Arc<crate::database::DbPool>,
}

#[cfg(feature = "sql-parser")]
impl DbSagaLog {
    /// 创建存储（表按需幂等创建：首次 persist/get/load 前调用 `init`）
    pub fn new(pool: Arc<crate::database::DbPool>) -> Self {
        Self { pool }
    }

    /// 建表（幂等）
    pub async fn init(&self) -> Result<(), String> {
        let session = self
            .pool
            .get_session("admin")
            .await
            .map_err(|e| e.to_string())?;
        session
            .execute_raw_ddl(
                "CREATE TABLE IF NOT EXISTS saga_logs (\
                 saga_id TEXT PRIMARY KEY, status TEXT NOT NULL, \
                 steps TEXT NOT NULL, updated_at TEXT NOT NULL)",
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(feature = "sql-parser")]
#[async_trait]
impl SagaLogStore for DbSagaLog {
    async fn persist(&self, log: &SagaLog) -> Result<(), String> {
        self.init().await?;
        let steps_text = saga_steps_to_json(&log.steps).to_string();
        let status = log.status.as_str();
        // updated_at 用客户端时间戳（跨 sqlite/postgres 方言安全）
        let updated_at = chrono::Utc::now().to_rfc3339();
        let sql = format!(
            "INSERT INTO saga_logs (saga_id, status, steps, updated_at) \
             VALUES ('{}', '{}', '{}', '{}') \
             ON CONFLICT(saga_id) DO UPDATE SET status = excluded.status, \
             steps = excluded.steps, updated_at = excluded.updated_at",
            log.saga_id.replace('\'', "''"),
            status,
            steps_text.replace('\'', "''"),
            updated_at,
        );
        let session = self
            .pool
            .get_session("admin")
            .await
            .map_err(|e| e.to_string())?;
        session.execute_raw(&sql).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn load_pending(&self) -> Result<Vec<SagaLog>, String> {
        let rows = self
            .pool
            .query_rows(
                "SELECT saga_id, status, steps FROM saga_logs \
                 WHERE status IN ('running', 'compensating')",
                "admin",
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.iter().filter_map(saga_log_from_row).collect())
    }

    async fn get(&self, saga_id: &str) -> Result<Option<SagaLog>, String> {
        let rows = self
            .pool
            .query_rows(
                &format!(
                    "SELECT saga_id, status, steps FROM saga_logs WHERE saga_id = '{}'",
                    saga_id.replace('\'', "''")
                ),
                "admin",
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.first().and_then(saga_log_from_row))
    }
}

/// 步骤日志 → JSON（避免公共类型引入 serde derive）
#[cfg(feature = "sql-parser")]
fn saga_steps_to_json(steps: &[SagaStepLog]) -> serde_json::Value {
    serde_json::Value::Array(
        steps
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "shard_id": s.shard_id,
                    "action_success": s.action_success,
                    "compensation_success": s.compensation_success,
                    "error": s.error,
                })
            })
            .collect(),
    )
}

/// 查询行 → SagaLog（postgres/sqlite 通用列名约定）
#[cfg(feature = "sql-parser")]
fn saga_log_from_row(row: &serde_json::Value) -> Option<SagaLog> {
    let saga_id = row.get("saga_id")?.as_str()?.to_string();
    let status = row
        .get("status")
        .and_then(|v| v.as_str())
        .map(SagaStatus::from_str_kind)
        .unwrap_or(SagaStatus::Running);
    let steps = row
        .get("steps")
        .and_then(|v| v.as_str())
        .and_then(|txt| serde_json::from_str::<serde_json::Value>(txt).ok())
        .and_then(|v| v.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    Some(SagaStepLog {
                        name: s.get("name")?.as_str()?.to_string(),
                        shard_id: s.get("shard_id")?.as_u64()? as u32,
                        action_success: s
                            .get("action_success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        compensation_success: s
                            .get("compensation_success")
                            .and_then(|v| v.as_bool()),
                        error: s.get("error").and_then(|v| v.as_str()).map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(SagaLog {
        saga_id,
        status,
        steps,
    })
}

// ============================================================================
// SagaExecutionResult
// ============================================================================
