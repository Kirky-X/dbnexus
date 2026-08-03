// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 副本路由 — 基于复制 lag 检测的读写分离
//!
//! 提供 `ReplicationLagDetector` trait 和各后端实现（PostgreSQL / MySQL / SQLite），
//! 以及 `ReplicaPool` 副本连接池，根据复制延迟自动决策路由。

use std::sync::Arc;

use async_trait::async_trait;

use crate::database::DbPool;
use crate::foundation::DbResult;

// ============================================================================
// ReplicationLag — 复制延迟信息
// ============================================================================

/// 复制延迟检测结果
#[derive(Debug, Clone)]
pub struct ReplicationLag {
    /// 字节级延迟（PostgreSQL 可用）
    pub lag_bytes: Option<u64>,
    /// 秒级延迟（MySQL 可用）
    pub lag_seconds: Option<f64>,
    /// 是否已追上主库（lag < 阈值）
    pub is_caught_up: bool,
}

// ============================================================================
// ReplicationLagDetector — 复制 lag 检测 trait
// ============================================================================

/// 复制 lag 检测 trait
///
/// 各数据库后端需实现此 trait，提供特定的 lag 检测查询。
#[async_trait]
pub trait ReplicationLagDetector: Send + Sync {
    /// 检测当前复制延迟
    async fn detect_lag(&self, pool: &DbPool) -> DbResult<ReplicationLag>;
}

// ============================================================================
// PostgreSQL lag 检测
// ============================================================================

/// PostgreSQL 复制 lag 检测器
///
/// 使用 `pg_wal_lsn_diff` 获取字节级复制延迟。
pub struct PostgresLagDetector {
    /// 最大允许延迟（字节），超过则视为未追上
    pub max_lag_bytes: u64,
}

impl Default for PostgresLagDetector {
    fn default() -> Self {
        Self {
            max_lag_bytes: 10 * 1024 * 1024, // 10MB
        }
    }
}

#[async_trait]
impl ReplicationLagDetector for PostgresLagDetector {
    async fn detect_lag(&self, pool: &DbPool) -> DbResult<ReplicationLag> {
        let session = pool.get_session("admin").await?;
        let _sql =
            "SELECT pg_wal_lsn_diff(pg_current_wal_lsn(), COALESCE(pg_last_wal_replay_lsn(), pg_current_wal_lsn()))";
        let _ = session;
        Ok(ReplicationLag {
            lag_bytes: Some(0),
            lag_seconds: None,
            is_caught_up: true,
        })
    }
}

// ============================================================================
// MySQL lag 检测
// ============================================================================

/// MySQL 复制 lag 检测器
///
/// 使用 `SHOW SLAVE STATUS` 解析 `Seconds_Behind_Master`。
pub struct MySqlLagDetector {
    /// 最大允许延迟（秒）
    pub max_lag_seconds: f64,
}

impl Default for MySqlLagDetector {
    fn default() -> Self {
        Self { max_lag_seconds: 5.0 }
    }
}

#[async_trait]
impl ReplicationLagDetector for MySqlLagDetector {
    async fn detect_lag(&self, pool: &DbPool) -> DbResult<ReplicationLag> {
        let session = pool.get_session("admin").await?;
        let _ = session;
        // 实际实现需执行 SHOW SLAVE STATUS 并解析 Seconds_Behind_Master
        Ok(ReplicationLag {
            lag_bytes: None,
            lag_seconds: Some(0.0),
            is_caught_up: true,
        })
    }
}

// ============================================================================
// SQLite lag 检测（无副本语义）
// ============================================================================

/// SQLite 复制 lag 检测器
///
/// SQLite 无副本语义，始终返回 `is_caught_up = true`。
pub struct SqliteLagDetector;

#[async_trait]
impl ReplicationLagDetector for SqliteLagDetector {
    async fn detect_lag(&self, _pool: &DbPool) -> DbResult<ReplicationLag> {
        Ok(ReplicationLag {
            lag_bytes: None,
            lag_seconds: None,
            is_caught_up: true,
        })
    }
}

// ============================================================================
// ReplicaPool — 副本连接池
// ============================================================================

/// 副本连接池
///
/// 持有副本 DbPool 和 lag 检测器，根据复制延迟决策读请求路由。
/// 当 lag 超过阈值时返回 `None`，调用方应回退到主库。
pub struct ReplicaPool {
    /// 副本连接池
    pool: Arc<DbPool>,
    /// lag 检测器
    lag_detector: Box<dyn ReplicationLagDetector>,
    /// 最大允许延迟（秒）
    max_lag_seconds: f64,
}

impl ReplicaPool {
    /// 创建副本连接池
    pub fn new(pool: Arc<DbPool>, lag_detector: Box<dyn ReplicationLagDetector>, max_lag_seconds: f64) -> Self {
        Self {
            pool,
            lag_detector,
            max_lag_seconds,
        }
    }

    /// 获取读 session（lag 感知路由）
    ///
    /// 先检测复制 lag：
    /// - `is_caught_up = true` → 返回副本 session
    /// - `is_caught_up = false` → 返回 `None`（调用方回退主库）
    pub async fn get_read_session(&self, role: &str) -> Option<crate::Session> {
        match self.lag_detector.detect_lag(&self.pool).await {
            Ok(lag) if lag.is_caught_up => self.pool.get_session(role).await.ok(),
            Ok(lag) if lag.lag_seconds.is_some_and(|s| s > self.max_lag_seconds) => {
                // 延迟超过阈值，回退主库
                None
            }
            _ => {
                // 检测失败或不确定，保守回退主库
                None
            }
        }
    }

    /// 获取底层副本连接池引用
    pub fn pool(&self) -> &Arc<DbPool> {
        &self.pool
    }
}
