// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 大文件拆分：自 session.rs 按职责纯移动的 impl 块（行为不变）。

use super::*;

impl Session {
    /// 以指定隔离级别开启事务
    ///
    /// SeaORM 关系型路径经 `begin_with_config` 传递隔离级别；图连接与
    /// DuckDB raw 路径**显性拒绝**（`DbError::Unsupported`）而非静默降级——
    /// 静默忽略隔离级别会让调用方误以为获得了该隔离保证。
    ///
    /// # Errors
    ///
    /// - 已在事务中：`DbError::Transaction`（与 `begin_transaction` 一致）
    /// - 图连接 / DuckDB raw 后端：`DbError::Unsupported`
    pub async fn begin_transaction_with_isolation(
        &self,
        level: DbIsolationLevel,
    ) -> Result<(), DbError> {
        // 短锁：检查是否已在事务中
        {
            let state = self.state.write().await;
            #[cfg(any(feature = "ladybug", feature = "neo4j"))]
            if state.graph_transaction.is_some() {
                return Err(DbError::Transaction(i18n::t_simple(
                    "session-already-in-graph-transaction",
                )));
            }
            if state.transaction.is_some() {
                return Err(DbError::Transaction(i18n::t_simple(
                    "session-already-in-transaction",
                )));
            }
        }

        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| DbError::Config(i18n::t_simple("session-connection-not-available")))?;

        // 图连接与 raw 后端不支持隔离级别语义
        #[cfg(any(feature = "ladybug", feature = "neo4j"))]
        if conn.is_graph() {
            return Err(DbError::Unsupported(i18n::t_simple(
                "session-isolation-relational-required",
            )));
        }
        let conn = conn.as_sea_orm().map_err(|_| {
            DbError::Unsupported(i18n::t_simple("session-isolation-relational-required"))
        })?;

        let transaction = conn
            .begin_with_config(Some(level.into_sea_orm()), None)
            .await
            .map_err(|e| {
                DbError::Transaction(i18n::t(
                    "session-txn-begin-failed",
                    &[("error", e.to_string())],
                ))
            })?;

        // 短锁：写入 transaction（含并发冲突处理）
        let has_conflict = {
            let state = self.state.write().await;
            state.transaction.is_some()
        };
        if has_conflict {
            let _ = transaction.rollback().await;
            return Err(DbError::Transaction(i18n::t_simple(
                "session-already-in-transaction-concurrent",
            )));
        }
        let mut state = self.state.write().await;
        state.transaction = Some(Arc::new(transaction));
        Ok(())
    }

    /// 开始事务
    ///
    /// 性能优化：短锁模式，避免持锁期间 async DB 调用。
    /// 流程：短锁检查 → 锁外 begin → 短锁写入（含并发冲突处理）
    ///
    /// 图事务双轨：按连接类型分发到关系型（SeaORM）或图（GraphConnection）事务路径。
    pub async fn begin_transaction(&self) -> Result<(), DbError> {
        // 短锁：检查是否已在事务中
        {
            let state = self.state.write().await;
            #[cfg(any(feature = "ladybug", feature = "neo4j"))]
            if state.graph_transaction.is_some() {
                return Err(DbError::Transaction(
                    "Already in graph transaction".to_string(),
                ));
            }
            if state.transaction.is_some() {
                return Err(DbError::Transaction("Already in transaction".to_string()));
            }
        }

        // 获取连接
        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Config(
                "Connection not available - Session may have been invalidated".to_string(),
            )
        })?;

        // 图连接分发：调用 begin_graph_txn
        #[cfg(any(feature = "ladybug", feature = "neo4j"))]
        if conn.is_graph() {
            let graph = conn.as_graph()?;
            let graph_txn = graph.begin_graph_txn().await.map_err(|e| {
                DbError::Transaction(i18n::t(
                    "session-txn-begin-graph-failed",
                    &[("error", e.to_string())],
                ))
            })?;

            // 短锁：写入 graph_transaction（含并发冲突处理）
            // extract-take-operate-writeback：锁内仅检查，rollback 在锁外执行
            let has_conflict = {
                let state = self.state.write().await;
                state.graph_transaction.is_some()
            };
            if has_conflict {
                // 并发冲突：锁已释放，安全执行 rollback
                let _ = graph_txn.rollback().await;
                return Err(DbError::Transaction(
                    "Already in graph transaction (concurrent begin detected)".to_string(),
                ));
            }
            // 无冲突：安全写入
            let mut state = self.state.write().await;
            state.graph_transaction = Some(graph_txn);
            return Ok(());
        }

        // SeaORM 逻辑：锁外执行 async DB 操作
        let conn = conn.as_sea_orm()?;
        let transaction = conn.begin().await.map_err(|e| {
            DbError::Transaction(i18n::t(
                "session-txn-begin-failed",
                &[("error", e.to_string())],
            ))
        })?;

        // 短锁：写入 transaction（含并发冲突处理）
        // extract-take-operate-writeback：锁内仅检查，rollback 在锁外执行
        let has_conflict = {
            let state = self.state.write().await;
            state.transaction.is_some()
        };
        if has_conflict {
            // 并发冲突：锁已释放，安全执行 rollback
            let _ = transaction.rollback().await;
            return Err(DbError::Transaction(
                "Already in transaction (concurrent begin detected)".to_string(),
            ));
        }
        // 无冲突：安全包装并写入
        let transaction = Arc::new(transaction);
        let mut state = self.state.write().await;
        state.transaction = Some(transaction);
        Ok(())
    }

    /// 提交事务
    ///
    /// 性能优化：短锁模式，take transaction 后锁外执行 commit。
    ///
    /// 图事务双轨：优先检查 graph_transaction，有则提交图事务，否则走 SeaORM 逻辑。
    ///
    /// # 并发安全
    ///
    /// 如果在 commit 时有其他查询正在执行（持有 transaction 的 Arc clone），
    /// `Arc::try_unwrap` 会失败并返回错误。这是预期行为：用户不应在查询执行中提交事务。
    pub async fn commit(&self) -> Result<(), DbError> {
        // 图事务优先：短锁 take graph_transaction
        #[cfg(any(feature = "ladybug", feature = "neo4j"))]
        {
            let graph_txn = {
                let mut state = self.state.write().await;
                state.graph_transaction.take()
            };
            if let Some(graph_txn) = graph_txn {
                // 锁外：执行 async commit（commit 消耗 self）
                graph_txn.commit().await.map_err(|e| {
                    DbError::Transaction(i18n::t(
                        "session-txn-commit-failed",
                        &[("error", e.to_string())],
                    ))
                })?;

                // 短锁：清除 last_write
                let mut state = self.state.write().await;
                state.last_write = None;
                return Ok(());
            }
        }

        // SeaORM 逻辑：短锁 take transaction
        let transaction_arc = {
            let mut state = self.state.write().await;
            state.transaction.take().ok_or_else(|| {
                DbError::Transaction("No active transaction to commit".to_string())
            })?
        };

        // 锁外：try_unwrap 解包 Arc（如果有并发查询持有引用，会失败）
        let transaction = Arc::try_unwrap(transaction_arc).map_err(|_| {
            DbError::Transaction(
                "Cannot commit: transaction is in use by a concurrent query".to_string(),
            )
        })?;

        // 锁外：执行 async commit（commit 消耗 self）
        transaction
            .commit()
            .await
            .map_err(|e| DbError::Transaction(e.to_string()))?;

        // 短锁：清除 last_write
        let mut state = self.state.write().await;
        state.last_write = None;
        Ok(())
    }

    /// 回滚事务
    ///
    /// 性能优化：短锁模式，take transaction 后锁外执行 rollback。
    ///
    /// 图事务双轨：优先检查 graph_transaction，有则回滚图事务，否则走 SeaORM 逻辑。
    ///
    /// # 并发安全
    ///
    /// 如果在 rollback 时有其他查询正在执行（持有 transaction 的 Arc clone），
    /// `Arc::try_unwrap` 会失败并返回错误。这是预期行为：用户不应在查询执行中回滚事务。
    pub async fn rollback(&self) -> Result<(), DbError> {
        // 图事务优先：短锁 take graph_transaction
        #[cfg(any(feature = "ladybug", feature = "neo4j"))]
        {
            let graph_txn = {
                let mut state = self.state.write().await;
                state.graph_transaction.take()
            };
            if let Some(graph_txn) = graph_txn {
                // 锁外：执行 async rollback（rollback 消耗 self）
                graph_txn.rollback().await.map_err(|e| {
                    DbError::Transaction(i18n::t(
                        "session-txn-rollback-graph-failed",
                        &[("error", e.to_string())],
                    ))
                })?;
                return Ok(());
            }
        }

        // SeaORM 逻辑：短锁 take transaction
        let transaction_arc = {
            let mut state = self.state.write().await;
            if state.transaction.is_none() {
                return Err(DbError::Transaction("Not in transaction".to_string()));
            }
            state.transaction.take().ok_or_else(|| {
                DbError::Transaction("No active transaction to rollback".to_string())
            })?
        };

        // 锁外：try_unwrap 解包 Arc
        let transaction = Arc::try_unwrap(transaction_arc).map_err(|_| {
            DbError::Transaction(
                "Cannot rollback: transaction is in use by a concurrent query".to_string(),
            )
        })?;

        // 锁外：执行 async rollback（rollback 消耗 self）
        transaction.rollback().await.map_err(|e| {
            DbError::Transaction(i18n::t(
                "session-txn-rollback-failed",
                &[("error", e.to_string())],
            ))
        })?;

        Ok(())
    }
}
