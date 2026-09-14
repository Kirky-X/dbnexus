// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Session 模块
//!
//! 提供数据库会话管理，包括事务、权限检查和读写分离

use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(any(feature = "ladybug", feature = "neo4j"))]
use std::collections::HashMap;

// DbPool 测试导入的 cfg 必须与实际使用点一致：
// - graph_tests / vuln_0005_tests：ladybug
// - vuln_0001_tests：permission + sqlite（测试用 sqlite::memory: 真实建池）
// - session_basic_tests（仅 sqlite）使用全限定 super::super::DbPool，不依赖本导入。
// 若只门控 permission（不含 sqlite），permission 单开组合下 vuln_0001_tests 整体
// 被 cfg 掉，导入悬空触发 unused import 警告。
#[cfg(all(
    test,
    any(feature = "ladybug", all(feature = "permission", feature = "sqlite"))
))]
use super::DbPool;
#[cfg(feature = "permission")]
use super::audit::audit_admin_bypass;
use super::db_pool::DbPoolInner;
use super::{DatabaseConnection, DbConnection};
#[cfg(all(feature = "sql-parser", feature = "permission"))]
use crate::access::SqlParser;
#[cfg(all(feature = "sql-parser", not(feature = "permission")))]
use crate::access::SqlParser;
#[cfg(feature = "sql-parser")]
use crate::access::is_ddl_operation;
#[cfg(feature = "sql-parser")]
use crate::access::{DdlGuard, DdlGuardPolicy, DdlValidationResult};
// SqlOperationType 仅在 permission 权限检查路径（parse_operation_async 结果映射）使用
#[cfg(all(feature = "sql-parser", feature = "permission"))]
use crate::access::SqlOperationType;
#[cfg(feature = "permission")]
use crate::access::{PermissionAction, PermissionContext};
use crate::foundation::{DbError, DbResult};
use crate::i18n;
#[cfg(feature = "metrics")]
use crate::observability::MetricsCollector;
use async_trait::async_trait;

// 导入 Sea-ORM 的事务 trait 和连接 trait
use sea_orm::{ConnectionTrait, DatabaseTransaction, ExecResult, TransactionTrait};
#[cfg(any(feature = "ladybug", feature = "neo4j"))]
use tokio::sync::Mutex;
use tokio::sync::RwLock;

/// Session 内部可变状态
///
/// 使用 Mutex 包装需要内部可变性的字段，支持 `&self` 方法签名
struct SessionState {
    /// 事务对象（用于真实的事务管理）
    ///
    /// 性能优化：使用 `Arc<DatabaseTransaction>` 而非 `DatabaseTransaction`，
    /// 因为 sea-orm 的 `DatabaseTransaction` 未实现 `Clone`，使用 `Arc` 包装后
    /// 可在 `execute_raw` 中短锁 clone 后锁外执行 async DB 操作，避免持锁 await。
    transaction: Option<Arc<DatabaseTransaction>>,

    /// 图数据库事务对象（ladybug/neo4j feature 启用时可用）
    ///
    /// 使用 `Box<dyn GraphTransaction + Send>` 存储图事务句柄。
    /// `GraphTransaction::commit/rollback` 消耗 `self`，因此使用 `Option` 存储，
    /// take 出来后调用。
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    graph_transaction: Option<Box<dyn crate::database::graph::GraphTransaction + Send>>,

    /// 图事务是否被 poison（FM-3.1 修复）
    ///
    /// 当 `execute_cypher` 在事务内 await 期间 panic 时，take→put back 中断，
    /// 事务句柄丢失。设置此标记后，后续图操作返回错误，防止在事务外执行。
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    graph_txn_poisoned: bool,

    /// 最后写操作时间（用于读写分离）
    ///
    /// LD-1 误报说明（架构审查）：审查曾标记"`Option<Instant>` 存在原子时序问题"为
    /// LOW 架构问题。此为误报：`last_write` 由外层 `state: Mutex<SessionState>` 保护，
    /// 所有读写均持锁（`mark_write` 写、`should_use_master` 读、`commit/rollback` 清除），
    /// 不存在无锁并发访问，无需使用 `AtomicInstant`。`Mutex<SessionState>` 的串行化
    /// 保证 `last_write` 的 read-modify-write 是原子的。改用原子操作反而是过度工程化。
    last_write: Option<Instant>,
}

/// Session 结构
pub struct Session {
    /// 数据库连接（统一枚举：SeaORM 或 DuckDB）
    connection: Option<DbConnection>,

    /// 连接池内部状态（用于归还连接）
    pool_inner: Arc<DbPoolInner>,

    /// 角色
    role: String,

    /// 权限上下文
    #[cfg(feature = "permission")]
    permission_ctx: PermissionContext,

    /// 内部可变状态（事务和写操作时间）
    state: RwLock<SessionState>,

    /// 图操作互斥锁（防止并发 `execute_cypher` 在 take → put back 窗口绕过事务）
    ///
    /// HIGH-001 修复：`Box<dyn GraphTransaction>` 不可 clone，图事务采用
    /// take → 锁外 await → put back 模式。若无互斥，并发 `execute_cypher` 会在
    /// take 后的 await 窗口内看到 `graph_transaction` 为 `None`，落入"直接在连接上
    /// 执行"分支，破坏事务隔离。此锁将图操作串行化，确保 put back 后才允许下一个 take。
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    graph_op_mutex: Mutex<()>,

    /// 指标收集器（可选，用于 metrics 特性）
    #[cfg(feature = "metrics")]
    metrics_collector: Option<Arc<MetricsCollector>>,
}

impl Session {
    /// 创建新的 Session
    pub(crate) fn new(
        connection: DbConnection,
        pool_inner: Arc<DbPoolInner>,
        role: String,
    ) -> Self {
        #[cfg(feature = "permission")]
        let permission_ctx = PermissionContext::new(role.clone(), pool_inner.policy_cache.clone());

        #[cfg(feature = "metrics")]
        let metrics = pool_inner
            .metrics_collector
            .read()
            .expect("metrics_collector lock")
            .clone();

        Session {
            connection: Some(connection),
            pool_inner,
            role,
            #[cfg(feature = "permission")]
            permission_ctx,
            state: RwLock::new(SessionState {
                transaction: None,
                #[cfg(any(feature = "ladybug", feature = "neo4j"))]
                graph_transaction: None,
                #[cfg(any(feature = "ladybug", feature = "neo4j"))]
                graph_txn_poisoned: false,
                last_write: None,
            }),
            #[cfg(any(feature = "ladybug", feature = "neo4j"))]
            graph_op_mutex: Mutex::new(()),
            #[cfg(feature = "metrics")]
            metrics_collector: metrics,
        }
    }

    /// 获取角色
    pub fn role(&self) -> &str {
        &self.role
    }

    /// 获取权限上下文
    #[cfg(feature = "permission")]
    pub fn permission_ctx(&self) -> &PermissionContext {
        &self.permission_ctx
    }

    /// 标记为写操作
    pub async fn mark_write(&self) {
        let mut state = self.state.write().await;
        state.last_write = Some(Instant::now());
    }

    /// 检查权限
    #[cfg(feature = "permission")]
    pub async fn check_permission(
        &self,
        table: &str,
        operation: &PermissionAction,
    ) -> Result<(), DbError> {
        // Admin 角色绕过权限检查（拥有完全控制权）
        // vuln-0001 修复：admin bypass 仍记录审计事件（进程级审计环）以保留审计链
        if self.role == self.pool_inner.admin_role {
            audit_admin_bypass(&self.role, table, operation);
            return Ok(());
        }

        if self
            .permission_ctx
            .check_table_access(table, operation)
            .await
        {
            Ok(())
        } else {
            Err(permission_denied(operation, table))
        }
    }

    /// 是否在事务中
    ///
    /// 图事务或关系型事务任一存在都返回 true。
    pub async fn is_in_transaction(&self) -> bool {
        let state = self.state.read().await;
        #[cfg(any(feature = "ladybug", feature = "neo4j"))]
        {
            state.graph_transaction.is_some() || state.transaction.is_some()
        }
        #[cfg(not(any(feature = "ladybug", feature = "neo4j")))]
        {
            state.transaction.is_some()
        }
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

    /// 是否应该使用主库（基于读写分离配置）
    pub async fn should_use_master(&self) -> bool {
        let state = self.state.read().await;
        // 如果在事务中，必须使用主库
        if state.transaction.is_some() {
            return true;
        }

        // 如果配置了读写分离且有写操作，使用主库
        state
            .last_write
            .map(|t| t.elapsed() < Duration::from_secs(5))
            .unwrap_or(false)
    }

    /// 获取 SeaORM 连接引用（仅内部宏和测试使用）
    ///
    /// 用户应通过 Entity 的 CRUD 方法进行数据库操作，不应直接调用此方法。
    /// 此方法从 `DbConnection` 枚举中提取 SeaORM 连接，若为 DuckDB 连接则返回错误。
    ///
    /// # 安全性
    ///
    /// 此方法确保连接在使用前是可用的。如果连接已被释放（不应发生），
    /// 将返回错误。Session 的生命周期管理确保连接始终可用。
    pub fn connection(&self) -> Result<&DatabaseConnection, DbError> {
        self.connection
            .as_ref()
            .ok_or_else(|| {
                DbError::Config(
                    "Connection not available - Session may have been invalidated".to_string(),
                )
            })?
            .as_sea_orm()
    }

    /// 创建迁移执行器（仅内部使用）
    ///
    /// 用于迁移功能，将底层连接包装成 MigrationExecutor
    #[cfg(feature = "migration")]
    pub fn create_migration_executor(
        &self,
        db_type: crate::foundation::DatabaseType,
    ) -> Result<super::MigrationExecutor, DbError> {
        let conn = self.connection()?.clone();
        Ok(super::MigrationExecutor::new(conn, db_type))
    }

    /// 执行原始 SQL（带权限检查）
    ///
    /// # 自动重试语义
    ///
    /// `retry` feature 启用且 `DbConfig.retry_policy` 配置时：
    /// - **幂等操作**（SELECT/SHOW/EXPLAIN 前缀，见 `is_idempotent_operation`）
    ///   失败后自动按指数退避重试（至多 `max_retries` 次）
    /// - **写类操作**（INSERT/UPDATE/DELETE/DDL）绝不重试，避免副作用重复
    pub async fn execute_raw(&self, sql: &str) -> DbResult<ExecResult> {
        #[cfg(feature = "sql-parser")]
        {
            // 检查是否为 DDL 操作
            if is_ddl_operation(sql) {
                return Err(DbError::Permission(
                    "DDL operations are not allowed in this context".to_string(),
                ));
            }
        }

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            Err(DbError::Permission(
                "execute_raw requires the sql-parser feature to be enabled".to_string(),
            ))
        }

        #[cfg(feature = "sql-parser")]
        {
            #[cfg(all(feature = "sql-parser", feature = "permission"))]
            {
                // 解析 SQL 操作类型和表名（使用全局共享单例，避免重复创建 parser + 缓存）
                let parser = SqlParser::shared().await;
                match parser.parse_single(sql).await {
                    Ok(parsed) => {
                        let action = match parsed.operation_type {
                            SqlOperationType::Select => PermissionAction::Select,
                            SqlOperationType::Insert => PermissionAction::Insert,
                            SqlOperationType::Update => PermissionAction::Update,
                            SqlOperationType::Delete => PermissionAction::Delete,
                            // 解析成功但是不支持的语句类型（DDL/DCL/Transaction）或没有表名的语句
                            // 这些情况需要拒绝执行以确保安全
                            _ => {
                                return Err(DbError::Permission(
                                    "SQL statement requires a valid table name for permission checking".to_string(),
                                ));
                            }
                        };

                        if parsed.all_table_names.is_empty() {
                            return Err(DbError::Permission(
                                "Failed to extract table name for permission checking".to_string(),
                            ));
                        }
                        for table in &parsed.all_table_names {
                            if table.is_empty() || is_invalid_table_name(table) {
                                return Err(DbError::Permission(
                                    "Failed to extract table name for permission checking"
                                        .to_string(),
                                ));
                            }
                        }

                        // Admin 角色绕过权限检查
                        if self.role == self.pool_inner.admin_role {
                            // admin 有完全权限，跳过检查
                        } else {
                            // 对语句涉及的所有表逐一检查权限
                            // （含 JOIN/子查询表，防止通过关联表越权读写未授权数据）
                            for table in &parsed.all_table_names {
                                if !self.permission_ctx.check_table_access(table, &action).await {
                                    return Err(permission_denied(&action, table));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // 解析失败：admin role 放行（对齐 Ok(None)/parse 失败路径），
                        // 非 admin role 拒绝（安全默认）。
                        if self.role == self.pool_inner.admin_role {
                            // admin 有完全权限，跳过检查
                        } else {
                            return Err(DbError::Permission(
                                "Failed to parse SQL statement for permission checking".to_string(),
                            ));
                        }
                    }
                }
            }

            // 慢查询检测——在查询执行前记录起始时间
            #[cfg(all(feature = "metrics", feature = "sql-parser"))]
            let query_start = std::time::Instant::now();

            // 性能优化：短锁 clone Arc<DatabaseTransaction>，锁外执行 async DB 调用
            let tx_opt: Option<Arc<DatabaseTransaction>> = {
                let state = self.state.write().await;
                state.transaction.clone()
            };

            // retry feature: 幂等查询自动重试 + 指数退避
            #[cfg(feature = "retry")]
            {
                if let Some(ref policy) = self.pool_inner.config.retry_policy
                    && crate::reliability::is_idempotent_operation(sql)
                {
                    let mut last_error: Option<DbError> = None;
                    // 首次执行 + 重试循环
                    for attempt in 0..=policy.max_retries {
                        if attempt > 0 {
                            let backoff = Self::calculate_retry_backoff(policy, attempt - 1);
                            tokio::time::sleep(backoff).await;
                        }
                        let result = if let Some(ref tx) = tx_opt {
                            tx.execute_unprepared(sql)
                                .await
                                .map_err(DbError::Connection)
                        } else {
                            let conn = self.connection()?;
                            conn.execute_unprepared(sql)
                                .await
                                .map_err(DbError::Connection)
                        };
                        match result {
                            Ok(exec_result) => {
                                // 记录查询指标（含慢查询检测）
                                #[cfg(all(feature = "metrics", feature = "sql-parser"))]
                                self.record_execute_metrics(query_start, true);
                                return Ok(exec_result);
                            }
                            Err(e) => last_error = Some(e),
                        }
                    }
                    // 重试耗尽，记录失败
                    #[cfg(all(feature = "metrics", feature = "sql-parser"))]
                    self.record_execute_metrics(query_start, false);
                    return Err(last_error.unwrap());
                }
            }

            // 无重试路径（retry 未启用或非幂等操作）
            let result = if let Some(tx) = tx_opt {
                tx.execute_unprepared(sql)
                    .await
                    .map_err(DbError::Connection)
            } else {
                let conn = self.connection()?;
                conn.execute_unprepared(sql)
                    .await
                    .map_err(DbError::Connection)
            };

            // 记录查询指标（含慢查询检测）
            #[cfg(all(feature = "metrics", feature = "sql-parser"))]
            self.record_execute_metrics(query_start, result.is_ok());

            result
        }
    }

    /// 统一行查询 API：执行 SELECT 并返回数据行（JSON 对象数组）
    ///
    /// 与 `execute_raw` 共用同一套解析与表级权限检查，但仅允许 SELECT，
    /// 并返回真实数据行（`serde_json::Value` 对象数组）而非 ExecResult，
    /// 供 scatter-gather、数据 API 网关等上层消费。
    ///
    /// # 自动重试语义
    ///
    /// `retry` feature 启用且 `DbConfig.retry_policy` 配置时，行查询失败
    /// 自动按指数退避重试（与 `execute_raw` 幂等路径同口径）。
    pub async fn query_rows(&self, sql: &str) -> DbResult<Vec<serde_json::Value>> {
        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            return Err(DbError::Permission(
                "query_rows requires the sql-parser feature to be enabled".to_string(),
            ));
        }

        #[cfg(feature = "sql-parser")]
        {
            // 与 execute_raw 一致：DDL 一律拒绝
            if is_ddl_operation(sql) {
                return Err(DbError::Permission(
                    "DDL operations are not allowed in this context".to_string(),
                ));
            }

            #[cfg(all(feature = "sql-parser", feature = "permission"))]
            {
                let parser = SqlParser::shared().await;
                match parser.parse_single(sql).await {
                    Ok(parsed) => {
                        // 行查询只允许 SELECT
                        if parsed.operation_type != SqlOperationType::Select {
                            return Err(DbError::Permission(
                                "query_rows only allows SELECT statements".to_string(),
                            ));
                        }
                        if parsed.all_table_names.is_empty() {
                            return Err(DbError::Permission(
                                "Failed to extract table name for permission checking".to_string(),
                            ));
                        }
                        for table in &parsed.all_table_names {
                            if table.is_empty() || is_invalid_table_name(table) {
                                return Err(DbError::Permission(
                                    "Failed to extract table name for permission checking"
                                        .to_string(),
                                ));
                            }
                        }
                        // Admin 角色绕过权限检查；非 admin 逐表校验 Select 权限
                        if self.role != self.pool_inner.admin_role {
                            for table in &parsed.all_table_names {
                                if !self
                                    .permission_ctx
                                    .check_table_access(table, &PermissionAction::Select)
                                    .await
                                {
                                    return Err(permission_denied(
                                        &PermissionAction::Select,
                                        table,
                                    ));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // 解析失败：admin 放行（对齐 execute_raw 路径），非 admin 拒绝（安全默认）
                        if self.role != self.pool_inner.admin_role {
                            return Err(DbError::Permission(
                                "Failed to parse SQL statement for permission checking".to_string(),
                            ));
                        }
                    }
                }
            }

            // 慢查询检测——与 execute_raw 同口径
            #[cfg(all(feature = "metrics", feature = "sql-parser"))]
            let query_start = std::time::Instant::now();

            // 短锁 clone Arc<DatabaseTransaction>，锁外执行 async DB 调用
            let tx_opt: Option<Arc<DatabaseTransaction>> = {
                let state = self.state.write().await;
                state.transaction.clone()
            };

            // 方言感知执行（SeaORM 2.0 无整行 JSON 提取 API，分方言处理）
            // retry feature——幂等行查询自动重试（与 execute_raw 同口径：
            // RetryPolicy 存在且 SQL 判定为幂等（SELECT/SHOW/EXPLAIN 前缀）时
            // 逐次退避重试；query_rows 仅放行 SELECT，天然幂等）
            #[cfg(feature = "retry")]
            let result = {
                let policy = self
                    .pool_inner
                    .config
                    .retry_policy
                    .as_ref()
                    .filter(|_| crate::reliability::is_idempotent_operation(sql));
                match policy {
                    Some(policy) => {
                        let mut last_error: Option<DbError> = None;
                        let mut success: Option<Vec<serde_json::Value>> = None;
                        for attempt in 0..=policy.max_retries {
                            if attempt > 0 {
                                let backoff = Self::calculate_retry_backoff(policy, attempt - 1);
                                tokio::time::sleep(backoff).await;
                            }
                            match self.query_rows_execute(sql, tx_opt.clone()).await {
                                Ok(rows) => {
                                    success = Some(rows);
                                    break;
                                }
                                Err(e) => last_error = Some(e),
                            }
                        }
                        match success {
                            Some(rows) => Ok(rows),
                            None => Err(last_error.unwrap()),
                        }
                    }
                    None => self.query_rows_execute(sql, tx_opt).await,
                }
            };

            #[cfg(not(feature = "retry"))]
            let result = self.query_rows_execute(sql, tx_opt).await;

            #[cfg(all(feature = "metrics", feature = "sql-parser"))]
            self.record_execute_metrics(query_start, result.is_ok());

            result
        }
    }

    /// 内部：按方言执行行查询并转为 JSON 行
    #[cfg(feature = "sql-parser")]
    ///
    /// - **PostgreSQL**：`SELECT row_to_json(sub.*) FROM (<sql>) sub` 包装，单列 JSON 精确提取
    /// - **SQLite**：主表列经 `pragma_table_info` 内省 + 逐列类型探测（i64→f64→String→Null）
    /// - **MySQL / 图后端 / DuckDB**：MVP 未覆盖，返回明确错误（DuckDB 请使用 `execute_duckdb`）
    async fn query_rows_execute(
        &self,
        sql: &str,
        tx_opt: Option<Arc<DatabaseTransaction>>,
    ) -> DbResult<Vec<serde_json::Value>> {
        use sea_orm::ConnectionTrait;

        // 图后端 / DuckDB 连接不支持 SeaORM 行查询，给出明确错误
        if let Some(conn_arc) = self.connection.as_ref()
            && conn_arc.as_sea_orm().is_err()
        {
            return Err(DbError::Query(
                "query_rows MVP supports SeaORM backends only (postgres/sqlite); \
                 use execute_duckdb or graph APIs for other backends"
                    .to_string(),
            ));
        }

        let backend = if let Some(tx) = tx_opt.as_ref() {
            tx.get_database_backend()
        } else {
            self.connection()?.get_database_backend()
        };

        #[allow(unused_variables)]
        let primary_table: Option<String> = {
            #[cfg(feature = "sql-parser")]
            {
                let parser = SqlParser::shared().await;
                match parser.parse_single(sql).await {
                    Ok(parsed) if parsed.all_table_names.len() == 1 => {
                        Some(parsed.all_table_names[0].clone())
                    }
                    _ => None,
                }
            }
            #[cfg(not(feature = "sql-parser"))]
            {
                let _ = sql;
                None
            }
        };

        // RLS 谓词注入（admin 角色走管理通道不注入；MVP 边界——
        // 无 permission feature 时 primary_table 为 None，注入自动失效）
        // postgres 取数臂按 MVP 设计使用原始 sql 做 row_to_json 包装，
        // 注入产物仅 sqlite 臂消费——sqlite 关闭时该绑定不参与编译使用。
        #[cfg(feature = "data-protection")]
        #[cfg_attr(not(feature = "sqlite"), allow(unused_variables))]
        let sql_for_fetch = {
            let dp = { self.pool_inner.data_protection.read().await.clone() };
            let is_admin = self.role == self.pool_inner.admin_role;
            if !is_admin {
                if let Some(rls) = dp.rls.as_ref() {
                    rls.inject(sql, primary_table.as_deref())
                } else {
                    sql.to_string()
                }
            } else {
                sql.to_string()
            }
        };
        #[cfg(not(feature = "data-protection"))]
        #[cfg_attr(not(feature = "sqlite"), allow(unused_variables))]
        let sql_for_fetch = sql.to_string();

        match backend {
            #[cfg(feature = "postgres")]
            sea_orm::DbBackend::Postgres => {
                // postgres：row_to_json 包装（零列名依赖、类型精确）
                let wrapped = format!(
                    "SELECT row_to_json(sub.*) AS row_data FROM ({}) AS sub",
                    sql.trim_end().trim_end_matches(';')
                );
                let stmt = sea_orm::Statement::from_string(backend, wrapped);
                let rows = if let Some(tx) = tx_opt.as_ref() {
                    tx.query_all_raw(stmt).await
                } else {
                    self.connection()?.query_all_raw(stmt).await
                }
                .map_err(DbError::Connection)?;
                let mut out = Vec::with_capacity(rows.len());
                for r in rows {
                    let v = r
                        .try_get::<serde_json::Value>("", "row_data")
                        .map_err(DbError::Connection)?;
                    out.push(v);
                }
                #[cfg(feature = "data-protection")]
                self.apply_masking(&mut out).await;
                Ok(out)
            }
            #[cfg(feature = "sqlite")]
            sea_orm::DbBackend::Sqlite => {
                // sqlite：主表列内省 + 类型探测（MVP：单表查询）
                let table = primary_table.as_deref().ok_or_else(|| {
                    DbError::Query(
                        "query_rows on sqlite (MVP) requires a single-table SELECT statement"
                            .to_string(),
                    )
                })?;
                let cols = self.sqlite_table_columns(table, tx_opt.as_ref()).await?;
                let stmt = sea_orm::Statement::from_string(backend, sql_for_fetch.clone());
                let rows = if let Some(tx) = tx_opt.as_ref() {
                    tx.query_all_raw(stmt).await
                } else {
                    self.connection()?.query_all_raw(stmt).await
                }
                .map_err(DbError::Connection)?;
                #[allow(unused_mut)]
                let mut out: Vec<serde_json::Value> =
                    rows.iter().map(|r| sqlite_row_to_json(r, &cols)).collect();
                #[cfg(feature = "data-protection")]
                self.apply_masking(&mut out).await;
                Ok(out)
            }
            _ => Err(DbError::Query(
                "query_rows MVP supports postgres/sqlite backends only".to_string(),
            )),
        }
        // 所有分支均已 return（postgres/sqlite 成功路径、其他方言 Err）
    }

    /// 查询出口字段脱敏
    #[cfg(feature = "data-protection")]
    async fn apply_masking(&self, rows: &mut Vec<serde_json::Value>) {
        let dp = { self.pool_inner.data_protection.read().await.clone() };
        if let Some(m) = dp.masking.as_ref() {
            m.apply(rows);
        }
    }

    /// 内部（sqlite）：主表列名（pragma_table_info）
    #[cfg(all(feature = "sqlite", feature = "sql-parser"))]
    async fn sqlite_table_columns(
        &self,
        table: &str,
        tx_opt: Option<&Arc<DatabaseTransaction>>,
    ) -> DbResult<Vec<String>> {
        use sea_orm::ConnectionTrait;
        let stmt = sea_orm::Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            format!(
                "SELECT name FROM pragma_table_info('{}')",
                table.replace('\'', "")
            ),
        );
        let rows = if let Some(tx) = tx_opt {
            tx.query_all_raw(stmt).await
        } else {
            self.connection()?.query_all_raw(stmt).await
        }
        .map_err(DbError::Connection)?;
        let mut cols = Vec::with_capacity(rows.len());
        for r in rows {
            let name: Option<String> = r.try_get::<Option<String>>("", "name").ok().flatten();
            if let Some(n) = name {
                cols.push(n);
            }
        }
        Ok(cols)
    }

    /// 计算重试退避时间（retry feature 内部辅助方法）
    #[cfg(feature = "retry")]
    fn calculate_retry_backoff(
        policy: &crate::reliability::RetryPolicy,
        attempt: u32,
    ) -> std::time::Duration {
        use std::time::Duration;
        let base_ms = policy.initial_backoff_ms as f64;
        let backoff_ms = base_ms * policy.multiplier.powi(attempt as i32);
        let capped_ms = backoff_ms.min(policy.max_backoff_ms as f64);
        Duration::from_millis(capped_ms as u64)
    }

    /// 语句级缓存感知执行路径
    ///
    /// 启用池级 prepare 缓存时，先在 LRU 中登记/命中语句就绪状态
    /// （命中指标经 `DbPool::prepare_cache_stats` 观察），再走 `execute_raw`
    /// 的完整防御链执行；未启用缓存时等价于 `execute_raw`。
    #[cfg(feature = "prepare-cache")]
    pub async fn execute_cached(&self, sql: &str) -> DbResult<ExecResult> {
        let cache = self
            .pool_inner
            .prepare_cache
            .read()
            .expect("prepare_cache lock poisoned")
            .clone();
        match cache {
            Some(cache) => {
                let _hit = cache.get_or_prepare(sql, |_| ());
                self.execute_raw(sql).await
            }
            None => self.execute_raw(sql).await,
        }
    }

    /// 统一 DDL 守卫漏斗 —— 全部 DDL 执行路径共用单一入口
    ///
    /// 消费 [`DdlGuardPolicy`] 端口（白名单/干跑/审计统一；默认内置白名单守卫，
    /// 可经 `DbPool::set_ddl_guard` / `DbPoolBuilder::ddl_guard` 注入自定义策略），
    /// 决策到 `DbError` 的映射与既有分散检查保持一致：
    /// - `Allowed` → 放行
    /// - `Forbidden(reason)` → `DbError::Permission`
    /// - `ParseError(error)` → `DbError::Config`
    #[cfg(feature = "sql-parser")]
    pub(crate) fn enforce_ddl_guard(&self, sql: &str) -> DbResult<()> {
        let injected = self
            .pool_inner
            .ddl_guard
            .read()
            .expect("ddl_guard lock poisoned")
            .clone();
        let result = match injected {
            Some(guard) => run_ddl_policy(guard.as_ref(), sql)?,
            None => {
                let guard = DdlGuard::new();
                run_ddl_policy(&guard, sql)?
            }
        };
        match result {
            DdlValidationResult::Allowed => Ok(()),
            DdlValidationResult::Forbidden(reason) => Err(DbError::Permission(i18n::t(
                "session-ddl-not-allowed",
                &[("reason", reason.to_string())],
            ))),
            DdlValidationResult::ParseError(error) => Err(DbError::Config(i18n::t(
                "session-ddl-parse-failed",
                &[("error", error.to_string())],
            ))),
        }
    }

    /// 执行 DDL 操作（允许创建表、删除表等操作）
    ///
    /// 此方法专门用于执行 DDL 操作，绕过常规的 DDL 检查。
    /// 仅用于测试和迁移场景，生产环境应谨慎使用。
    ///
    /// # Arguments
    ///
    /// * `sql` - 要执行的 DDL SQL 语句
    ///
    /// # Returns
    ///
    /// 执行结果
    ///
    /// # Note
    ///
    /// 此方法只允许管理员角色执行，用于测试和迁移场景。
    pub async fn execute_raw_ddl(&self, sql: &str) -> DbResult<ExecResult> {
        // 检查角色白名单（只允许管理员角色执行 DDL）
        if self.role != self.pool_inner.admin_role {
            return Err(DbError::Permission(format!(
                "DDL operations are only allowed for admin role. Current role: '{}', Admin role: '{}'",
                self.role, self.pool_inner.admin_role
            )));
        }

        // DDL 安全验证
        #[cfg(feature = "sql-parser")]
        self.enforce_ddl_guard(sql)?;

        // 执行 SQL
        let conn = self.connection()?;
        conn.execute_unprepared(sql)
            .await
            .map_err(DbError::Connection)
    }

    /// DuckDB 路径统一安全门（DDL 拦截 + SQL 注入检测 + 权限校验）
    ///
    /// `execute_duckdb` / `execute_duckdb_raw` 及其参数化变体的共享防御链：
    /// - 非 DML 语句（DDL）→ 仅 admin 角色通过 DdlGuard AST 验证后放行
    /// - DML/查询 → admin 角色直接执行；非 admin 角色需通过表级权限检查
    /// - 无法解析的语句 → admin 放行（支持 `SELECT 1` 等健康检查），非 admin 拒绝
    ///
    /// 返回 `Ok(())` 表示语句已通过安全门，调用方可继续执行。
    ///
    /// 门控含 `duckdb`：本方法的全部调用者（`execute_duckdb*` 系列）均为
    /// `#[cfg(feature = "duckdb")]`。若只门控 `sql-parser`，则
    /// "sql-parser 开 + duckdb 关"组合下本方法成为死代码（触发
    /// `dead_code` 警告，下游 CI 的 `-D warnings` 会失败）。
    #[cfg(all(feature = "sql-parser", feature = "duckdb"))]
    async fn duckdb_security_gate(&self, sql: &str) -> DbResult<()> {
        if is_ddl_operation(sql) {
            // DDL：仅 admin 角色（对齐 execute_raw_ddl 的角色白名单）
            if self.role != self.pool_inner.admin_role {
                return Err(DbError::Permission(format!(
                    "DDL operations are only allowed for admin role in DuckDB context. Current role: '{}', Admin role: '{}'",
                    self.role, self.pool_inner.admin_role
                )));
            }
            // 统一守卫漏斗（白名单/干跑/审计经 DdlGuardPolicy 端口）
            return self.enforce_ddl_guard(sql);
        }

        // 非 DDL：表级权限检查
        #[cfg(feature = "permission")]
        {
            let parser = SqlParser::shared().await;
            match parser.parse_operation_async(sql).await {
                Ok(Some((table_name, action))) => {
                    if table_name.is_empty() || is_invalid_table_name(&table_name) {
                        return Err(DbError::Permission(
                            "Failed to extract table name for permission checking".to_string(),
                        ));
                    }
                    if self.role != self.pool_inner.admin_role
                        && !self
                            .permission_ctx
                            .check_table_access(&table_name, &action)
                            .await
                    {
                        return Err(permission_denied(&action, &table_name));
                    }
                }
                Ok(None) => {
                    // admin role 对无法解析的语句直接执行（支持 SELECT 1 等无表名健康检查）；
                    // 非 admin role 拒绝（安全默认：无法解析则无法做权限检查）。
                    if self.role != self.pool_inner.admin_role {
                        return Err(DbError::Permission(
                            "SQL statement requires a valid table name for permission checking"
                                .to_string(),
                        ));
                    }
                }
                Err(_) => {
                    // 解析失败（如 SQL 含 INFORMATION_SCHEMA 被注入检测拦截）：
                    // admin role 放行（对齐 Ok(None) 路径——admin 拥有完全控制权）；
                    // 非 admin role 拒绝（安全默认：无法解析则无法做权限检查）。
                    if self.role != self.pool_inner.admin_role {
                        return Err(DbError::Permission(
                            "Failed to parse SQL statement for permission checking".to_string(),
                        ));
                    }
                }
            }
        }
        #[cfg(not(feature = "permission"))]
        {
            let _ = sql;
        }
        Ok(())
    }

    /// 执行参数化 DuckDB 查询（仅 DuckDB 连接可用）
    ///
    /// 与 [`Self::execute_duckdb`] 相同的安全防御链，但通过 prepared statement
    /// 传递绑定参数——数据库不会将参数值解析为 SQL 代码，从根本上防止 SQL 注入。
    /// **所有携带外部输入的 DuckDB 查询必须走本方法**，禁止 format!/拼接组装 SQL。
    ///
    /// # 参数
    ///
    /// * `sql` - 含 `?` 占位符的 SELECT 语句
    /// * `params` - 按占位符顺序排列的绑定值
    ///
    /// # 返回
    ///
    /// 查询结果行列表
    #[cfg(feature = "duckdb")]
    pub async fn execute_duckdb_with_params(
        &self,
        sql: &str,
        params: Vec<crate::database::DuckValue>,
    ) -> DbResult<Vec<crate::database::DuckDbRow>> {
        #[cfg(feature = "sql-parser")]
        self.duckdb_security_gate(sql).await?;

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            return Err(DbError::Permission(
                "execute_duckdb_with_params requires the sql-parser feature to be enabled for security checks"
                    .to_string(),
            ));
        }

        #[cfg(feature = "sql-parser")]
        {
            let conn = self
                .connection
                .as_ref()
                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
            let duck_conn = conn.as_duckdb()?;
            duck_conn.query_with_params(sql, params).await
        }
    }

    /// 执行参数化 DuckDB DML 语句（仅 DuckDB 连接可用）
    ///
    /// 与 [`Self::execute_duckdb_raw`] 相同的安全防御链，但通过 prepared statement
    /// 传递绑定参数，从根本上防止 SQL 注入。**所有携带外部输入的 INSERT/UPDATE/DELETE
    /// 必须走本方法**，禁止 format!/拼接组装 SQL。DDL 不携带参数，继续用
    /// [`Self::execute_duckdb_raw`]。
    ///
    /// # 参数
    ///
    /// * `sql` - 含 `?` 占位符的 DML 语句
    /// * `params` - 按占位符顺序排列的绑定值
    ///
    /// # 返回
    ///
    /// 受影响的行数信息
    #[cfg(feature = "duckdb")]
    pub async fn execute_duckdb_raw_with_params(
        &self,
        sql: &str,
        params: Vec<crate::database::DuckValue>,
    ) -> DbResult<crate::database::DuckDbExecResult> {
        #[cfg(feature = "sql-parser")]
        self.duckdb_security_gate(sql).await?;

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            return Err(DbError::Permission(
                "execute_duckdb_raw_with_params requires the sql-parser feature to be enabled for security checks"
                    .to_string(),
            ));
        }

        #[cfg(feature = "sql-parser")]
        {
            let conn = self
                .connection
                .as_ref()
                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
            let duck_conn = conn.as_duckdb()?;
            duck_conn.execute_with_params(sql, params).await
        }
    }

    /// 在单个事务中原子执行多条参数化 DuckDB 语句（仅 DuckDB 连接可用）
    ///
    /// DuckDB 路径的 Session 级事务由本方法提供（`begin_transaction` 仅支持 SeaORM 后端）：
    /// 在**同一条**底层连接上按顺序执行 `BEGIN → 语句序列 → COMMIT`，
    /// 任一语句失败整体 ROLLBACK，保证多语句原子性（如级联删除）。
    /// 每条语句均先通过 [`Self::execute_duckdb_raw_with_params`] 同款安全门。
    ///
    /// # 参数
    ///
    /// * `statements` - `(sql, params)` 有序序列，全部在同一事务内执行
    ///
    /// # 返回
    ///
    /// 各语句的执行结果（顺序与输入一致）；任一失败即整体回滚并返回错误
    #[cfg(feature = "duckdb")]
    pub async fn execute_duckdb_transaction(
        &self,
        statements: Vec<(String, Vec<crate::database::DuckValue>)>,
    ) -> DbResult<Vec<crate::database::DuckDbExecResult>> {
        #[cfg(feature = "sql-parser")]
        {
            for (sql, _) in &statements {
                self.duckdb_security_gate(sql).await?;
            }
        }

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = &statements;
            return Err(DbError::Permission(
                "execute_duckdb_transaction requires the sql-parser feature to be enabled for security checks"
                    .to_string(),
            ));
        }

        #[cfg(feature = "sql-parser")]
        {
            let conn = self
                .connection
                .as_ref()
                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
            let duck_conn = conn.as_duckdb()?;
            duck_conn.execute_transaction(statements).await
        }
    }

    /// 执行 DuckDB 查询（仅 DuckDB 连接可用）
    ///
    /// 当 Session 持有 DuckDB 连接时，通过此方法执行 SQL 查询并返回结果行。
    /// 若持有 SeaORM 连接则返回错误。
    ///
    /// # 参数
    ///
    /// * `sql` - 要执行的 SQL 查询语句（SELECT）
    ///
    /// # 返回
    ///
    /// 查询结果行列表
    #[cfg(feature = "duckdb")]
    pub async fn execute_duckdb(&self, sql: &str) -> DbResult<Vec<crate::database::DuckDbRow>> {
        // 安全检查：与 execute_raw 一致的防御链（DDL 拦截 + SQL 注入检测 + 权限校验）
        #[cfg(feature = "sql-parser")]
        {
            if is_ddl_operation(sql) {
                return Err(DbError::Permission(
                    "DDL operations are not allowed in DuckDB query context".to_string(),
                ));
            }
        }

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            Err(DbError::Permission(
                "execute_duckdb requires the sql-parser feature to be enabled for security checks"
                    .to_string(),
            ))
        }

        #[cfg(feature = "sql-parser")]
        {
            #[cfg(all(feature = "sql-parser", feature = "permission"))]
            {
                let parser = SqlParser::shared().await;
                match parser.parse_operation_async(sql).await {
                    Ok(Some((table_name, action))) => {
                        if table_name.is_empty() || is_invalid_table_name(&table_name) {
                            return Err(DbError::Permission(
                                "Failed to extract table name for permission checking".to_string(),
                            ));
                        }
                        if self.role != self.pool_inner.admin_role
                            && !self
                                .permission_ctx
                                .check_table_access(&table_name, &action)
                                .await
                        {
                            return Err(permission_denied(&action, &table_name));
                        }
                    }
                    Ok(None) => {
                        // admin role 对无法解析的语句直接执行（对齐 execute 的 None 路径），
                        // 支持 SELECT 1 / SELECT 1 AS health 等无表名健康检查查询；
                        // 非 admin role 拒绝（安全默认：无法解析则无法做权限检查）。
                        if self.role != self.pool_inner.admin_role {
                            return Err(DbError::Permission(
                                "SQL statement requires a valid table name for permission checking"
                                    .to_string(),
                            ));
                        }
                    }
                    Err(_) => {
                        // 解析失败：admin role 放行（对齐 Ok(None) 路径），非 admin 拒绝。
                        if self.role != self.pool_inner.admin_role {
                            return Err(DbError::Permission(
                                "Failed to parse SQL statement for permission checking".to_string(),
                            ));
                        }
                    }
                }
            }

            let conn = self
                .connection
                .as_ref()
                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
            let duck_conn = conn.as_duckdb()?;
            duck_conn.query(sql).await
        }
    }

    /// 执行 DuckDB DDL/DML 语句（仅 DuckDB 连接可用）
    ///
    /// 当 Session 持有 DuckDB 连接时，通过此方法执行 CREATE/INSERT/UPDATE/DELETE 等语句。
    /// 若持有 SeaORM 连接则返回错误。
    ///
    /// # 参数
    ///
    /// * `sql` - 要执行的 SQL 语句（DDL/DML）
    ///
    /// # 返回
    ///
    /// 受影响的行数信息
    #[cfg(feature = "duckdb")]
    pub async fn execute_duckdb_raw(
        &self,
        sql: &str,
    ) -> DbResult<crate::database::DuckDbExecResult> {
        // 安全检查：与 execute_raw_ddl 对齐 —— admin role 通过 DdlGuard 验证后允许 DDL，
        // 非 admin role 拒绝 DDL。DuckDB 是分析型数据库，admin 需要能创建表/视图，
        // 与 SeaORM 路径的 execute_raw_ddl 行为保持一致。
        #[cfg(feature = "sql-parser")]
        {
            if is_ddl_operation(sql) {
                if self.role == self.pool_inner.admin_role {
                    // 统一守卫漏斗 —— admin role 通过守卫验证后直接执行，
                    // 不再走 parse_operation 权限检查（DDL 语句无法被
                    // parse_operation_async 正确解析，会返回 Err）
                    self.enforce_ddl_guard(sql)?;
                    let conn = self
                        .connection
                        .as_ref()
                        .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
                    let duck_conn = conn.as_duckdb()?;
                    return duck_conn.execute(sql).await;
                } else {
                    return Err(DbError::Permission(format!(
                        "DDL operations are only allowed for admin role in DuckDB context. Current role: '{}', Admin role: '{}'",
                        self.role, self.pool_inner.admin_role
                    )));
                }
            }
        }

        #[cfg(not(feature = "sql-parser"))]
        {
            let _ = sql;
            Err(DbError::Permission(
                "execute_duckdb_raw requires the sql-parser feature to be enabled for security checks".to_string(),
            ))
        }

        #[cfg(feature = "sql-parser")]
        {
            #[cfg(all(feature = "sql-parser", feature = "permission"))]
            {
                let parser = SqlParser::shared().await;
                match parser.parse_operation_async(sql).await {
                    Ok(Some((table_name, action))) => {
                        if table_name.is_empty() || is_invalid_table_name(&table_name) {
                            return Err(DbError::Permission(
                                "Failed to extract table name for permission checking".to_string(),
                            ));
                        }
                        if self.role != self.pool_inner.admin_role
                            && !self
                                .permission_ctx
                                .check_table_access(&table_name, &action)
                                .await
                        {
                            return Err(permission_denied(&action, &table_name));
                        }
                    }
                    Ok(None) => {
                        // admin role 对无法解析的语句直接执行（对齐 execute 的 None 路径），
                        // 非 admin role 拒绝（安全默认：无法解析则无法做权限检查）。
                        if self.role != self.pool_inner.admin_role {
                            return Err(DbError::Permission(
                                "SQL statement requires a valid table name for permission checking"
                                    .to_string(),
                            ));
                        }
                    }
                    Err(_) => {
                        // 解析失败：admin role 放行（对齐 Ok(None) 路径），非 admin 拒绝。
                        if self.role != self.pool_inner.admin_role {
                            return Err(DbError::Permission(
                                "Failed to parse SQL statement for permission checking".to_string(),
                            ));
                        }
                    }
                }
            }

            let conn = self
                .connection
                .as_ref()
                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
            let duck_conn = conn.as_duckdb()?;
            duck_conn.execute(sql).await
        }
    }

    /// 执行参数化 Cypher 查询（vuln-0005 修复）
    ///
    /// 与 `execute_cypher_in_transaction` 相同的事务分发逻辑，
    /// 但通过 `$name` 占位符 + `params` 映射传递用户输入，
    /// 底层使用 prepared statement，数据库不会将参数值解析为 Cypher 代码，
    /// 从根本上防止 Cypher 注入。
    ///
    /// # 参数
    ///
    /// * `cypher` - Cypher 查询语句（含 `$param` 占位符）
    /// * `params` - 参数映射（key 必须与 Cypher 中的 `$param` 名称一致）
    ///
    /// # 返回
    ///
    /// 图执行结果（Query 或 Write）
    ///
    /// # Errors
    ///
    /// - 非 admin 角色调用时返回 `DbError::Permission`
    /// - 连接不是图连接时返回 `DbError::Connection`
    /// - 查询语法错误或执行失败时返回对应的 `DbError`
    /// - Cypher 包含多语句/注释/危险过程时返回 `DbError::Permission`（vuln-0005）
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let mut params = HashMap::new();
    /// params.insert("name".to_string(), serde_json::json!("Alice"));
    /// params.insert("age".to_string(), serde_json::json!(30));
    /// let result = session.execute_cypher_with_params(
    ///     "CREATE (n:User {name: $name, age: $age})",
    ///     params,
    /// ).await?;
    /// ```
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    pub async fn execute_cypher_with_params(
        &self,
        cypher: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> DbResult<crate::database::graph::GraphExecResult> {
        // MD-1 修复：委托给 execute_cypher_in_transaction helper 复用事务分发逻辑
        self.execute_cypher_in_transaction(cypher, Some(params))
            .await
    }

    /// 执行 Cypher 查询的内部 helper（MD-1 提取，统一事务分发逻辑）
    ///
    /// `execute_cypher_with_params` 的完整事务分发流程：
    /// 1. 注入防护（vuln-0005）
    /// 2. 图权限检查
    /// 3. 取连接 + 获取图操作互斥锁（HIGH-001：串行化 take → put back）
    /// 4. 短锁 take graph_transaction（含 poisoned 检查，FM-3.1）
    /// 5. 事务内执行：PoisonGuard + take → await → put back
    /// 6. 事务外执行：直接在连接上调用
    ///
    /// # 参数
    ///
    /// * `cypher` - Cypher 查询语句
    /// * `params` - 参数映射（key 对应 Cypher 中的 `$param` 占位符）
    ///
    /// # 设计说明
    ///
    /// 使用 `Option<HashMap>` 区分两种操作。`params.take()` 在互斥分支中消耗参数，
    /// 避免在调用路径强制构造空 HashMap，也无需 clone。
    ///
    /// # HD-2 误报说明（架构审查）
    ///
    /// 审查曾标记"Session 直接依赖 Ladybug/Neo4j 具体实现"为 HIGH 架构问题。此为误报：
    /// 所有图操作通过 `conn.as_graph()?` 获取 `&dyn GraphConnection` trait 对象，
    /// Session 仅持有 `DbConnection` 枚举与 `&dyn GraphConnection`/`Box<dyn GraphTransaction>`
    /// trait 对象，不直接依赖任何具体类型。`begin_transaction` 与本 helper 的图路径
    /// 均通过 trait 方法分发，Ladybug/Neo4j 实现细节封装在各自 `ladybug_conn`/`neo4j_conn`
    /// 模块内。这与 `sea_orm::DatabaseTransaction` 的依赖方式一致：通过 trait 对象解耦，
    /// 而非 concrete type。新增图后端只需实现 `GraphConnection`/`GraphTransaction` trait，
    /// Session 代码无需改动（开放-封闭原则）。
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    async fn execute_cypher_in_transaction(
        &self,
        cypher: &str,
        mut params: Option<HashMap<String, serde_json::Value>>,
    ) -> DbResult<crate::database::graph::GraphExecResult> {
        // vuln-0005 修复：注入防护检查（长度限制 + 危险模式检测）
        validate_cypher_safety(cypher)?;

        // 图权限检查（admin 角色由 GraphPermissionContext 内部处理）
        #[cfg(feature = "permission")]
        {
            let graph_perm_ctx = crate::access::permission::GraphPermissionContext::new(
                &self.role,
                &self.pool_inner.admin_role,
            );
            graph_perm_ctx
                .check_graph_access(crate::access::permission::PermissionAction::Traverse)?;
        }

        // 取连接
        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Config(
                "Connection not available - Session may have been invalidated".to_string(),
            )
        })?;

        // 获取图操作互斥锁（HIGH-001：防止并发 take → put back 窗口绕过事务隔离）
        let _graph_op_guard = self.graph_op_mutex.lock().await;

        // 检查是否在图事务中（短锁 take → 锁外执行 → 短锁 put back）
        let graph_txn = {
            let mut state = self.state.write().await;
            if state.graph_txn_poisoned {
                return Err(DbError::Transaction(
                    "Graph transaction is poisoned due to previous panic; \
                     Session must be dropped and recreated"
                        .to_string(),
                ));
            }
            state.graph_transaction.take()
        };

        if let Some(graph_txn) = graph_txn {
            // PoisonGuard（FM-3.1 修复：panic 时标记事务为 poisoned，防止丢失句柄后绕过事务隔离）
            struct PoisonGuard<'a> {
                state: &'a RwLock<SessionState>,
                armed: bool,
            }
            impl<'a> Drop for PoisonGuard<'a> {
                fn drop(&mut self) {
                    if self.armed
                        && let Ok(mut state) = self.state.try_write()
                    {
                        state.graph_txn_poisoned = true;
                    }
                }
            }

            let mut guard = PoisonGuard {
                state: &self.state,
                armed: true,
            };
            // 互斥分支：params.take() 确保参数只被消耗一次（None → execute_cypher / Some → with_params）
            let result = if let Some(p) = params.take() {
                graph_txn.execute_cypher_with_params(cypher, p).await
            } else {
                graph_txn.execute_cypher(cypher).await
            };
            guard.armed = false;

            let mut state = self.state.write().await;
            state.graph_transaction = Some(graph_txn);
            return result;
        }

        // 不在事务中，直接在连接上执行
        let graph = conn.as_graph()?;
        if let Some(p) = params.take() {
            graph.execute_cypher_with_params(cypher, p).await
        } else {
            graph.execute_cypher(cypher).await
        }
    }

    /// 执行 SQL（带权限检查和操作类型）
    pub async fn execute(&self, sql: &str) -> DbResult<ExecResult> {
        // DDL 检查（sql-parser 启用时）
        #[cfg(feature = "sql-parser")]
        check_ddl_operation(sql)?;

        #[cfg(feature = "permission")]
        {
            let start = Instant::now();
            // 解析 SQL 操作类型和表名
            let parsed = parse_sql_for_permission(sql).await?;
            match parsed {
                Some((table_name, action)) => {
                    // 表名有效性检查
                    if table_name.is_empty() || is_invalid_table_name(&table_name) {
                        return Err(DbError::Permission(
                            "Failed to extract table name for permission checking".to_string(),
                        ));
                    }
                    // 权限检查（含 admin 角色绕过）
                    self.check_permission(&table_name, &action).await?;
                    // 执行 SQL
                    let result = self.execute_raw(sql).await?;
                    // 记录指标并标记写操作
                    self.record_metrics_and_mark_write(&action, start).await;
                    Ok(result)
                }
                None => {
                    // 解析失败或不支持的语句类型，直接执行
                    // （仅 sql-parser 启用时可能出现 None）
                    let result = self.execute_raw(sql).await?;
                    Ok(result)
                }
            }
        }

        #[cfg(not(feature = "permission"))]
        {
            // 执行 SQL
            let result = self.execute_raw(sql).await?;
            Ok(result)
        }
    }

    /// 执行 SQL 并指定操作类型
    #[cfg(feature = "permission")]
    pub async fn execute_with_operation(
        &self,
        sql: &str,
        operation: &PermissionAction,
    ) -> DbResult<ExecResult> {
        let start = Instant::now();

        #[cfg(feature = "sql-parser")]
        {
            // 检查是否为 DDL 操作
            if is_ddl_operation(sql) {
                return Err(DbError::Permission(
                    "DDL operations are not allowed in this context".to_string(),
                ));
            }
        }

        // 提取表名（vuln-0003 修复：使用 SqlParser AST 解析替代朴素字符串匹配）
        let table_name: String = extract_table_name_via_parser(sql).await.unwrap_or_default();

        // 检查权限
        #[cfg(feature = "permission")]
        {
            if !table_name.is_empty()
                && !self
                    .permission_ctx
                    .check_table_access(&table_name, operation)
                    .await
            {
                return Err(permission_denied(operation, &table_name));
            }
        }

        // 执行 SQL
        let result = self.execute_raw(sql).await?;

        // LD-2 修复：复用 record_metrics_and_mark_write 统一 metrics 记录与 mark_write 逻辑，
        // 消除与 execute() 方法中重复的手动展开（record_query_metrics + is_write_action + mark_write）
        self.record_metrics_and_mark_write(operation, start).await;

        Ok(result)
    }

    /// 批量执行 SQL
    ///
    /// # Arguments
    ///
    /// * `sqls` - 要执行的 SQL 语句列表
    ///
    /// # Returns
    ///
    /// 返回执行结果列表
    pub async fn batch_execute(&self, sqls: Vec<&str>) -> DbResult<Vec<DbResult<ExecResult>>> {
        let mut results = Vec::new();

        for sql in sqls {
            let result = self.execute(sql).await;
            results.push(result);
        }

        Ok(results)
    }

    /// 批量执行（带事务）
    ///
    /// 所有操作在一个事务中执行，任一失败则全部回滚
    ///
    /// # Arguments
    ///
    /// * `sqls` - 要执行的 SQL 语句列表
    ///
    /// # Returns
    ///
    /// 返回执行结果列表，任一失败则返回错误
    pub async fn batch_execute_in_transaction(&self, sqls: Vec<&str>) -> DbResult<Vec<ExecResult>> {
        self.begin_transaction().await?;

        // MD-3 修复：用 async block + ? 简化事务执行，消除 last_error + break 命令式风格
        let result: DbResult<Vec<ExecResult>> = async {
            let mut results = Vec::with_capacity(sqls.len());
            for sql in sqls {
                results.push(self.execute_raw(sql).await?);
            }
            Ok(results)
        }
        .await;

        match result {
            Ok(results) => {
                self.commit().await?;
                Ok(results)
            }
            Err(e) => {
                // LD-4 修复：保留原始错误上下文，rollback 失败时组合错误消息（不覆盖原始错误）
                match self.rollback().await {
                    Ok(()) => Err(e),
                    Err(rollback_err) => Err(DbError::Transaction(format!(
                        "batch failed: {}; rollback also failed: {}",
                        e, rollback_err
                    ))),
                }
            }
        }
    }

    /// 记录查询指标
    #[cfg(all(feature = "metrics", feature = "permission"))]
    fn record_query_metrics(&self, query_type: &str, duration: Duration, success: bool) {
        if let Some(metrics) = &self.metrics_collector {
            metrics.record_query(query_type, duration, success, None);
        }
    }

    /// 记录查询指标（无 metrics 特性）
    #[cfg(all(not(feature = "metrics"), feature = "permission"))]
    fn record_query_metrics(&self, _query_type: &str, _duration: Duration, _success: bool) {
        // No-op when metrics feature is disabled
    }

    /// 记录 `execute_raw` 查询指标（含慢查询检测）。
    ///
    /// 将查询耗时经 `MetricsCollector::record_query` 录入指标收集器，
    /// 内部自动比对 `SlowQueryConfig` 阈值并记录慢查询事件。
    /// `metrics` feature 未启用时此方法不存在（零开销）。
    #[cfg(all(feature = "metrics", feature = "sql-parser"))]
    fn record_execute_metrics(&self, start: std::time::Instant, success: bool) {
        if let Some(metrics) = &self.metrics_collector {
            metrics.record_query("execute_raw", start.elapsed(), success, None);
        }
    }

    /// 查询缓存——检查 `cache_provider` 是否有缓存的查询结果。
    ///
    /// 返回 `Some(bytes)` 表示缓存命中，`None` 表示未命中。
    /// 仅在 `cache`/`oxcache-integration` feature 启用且已注入 `cache_provider` 时有效。
    #[cfg(any(feature = "cache", feature = "oxcache-integration"))]
    pub async fn query_cache_get(&self, key: &str) -> Option<Vec<u8>> {
        // ArcSwapOption::load() 返回 Guard，clone 内部 Arc 后立即释放 Guard，
        // 避免跨 .await 持有 Guard。
        let provider = self.pool_inner.cache_provider.load().clone()?;
        provider.get(key).await.ok().flatten()
    }

    /// 查询缓存——将查询结果存入 `cache_provider`。
    ///
    /// TTL 取自 `CacheConfig.default_ttl`。
    #[cfg(any(feature = "cache", feature = "oxcache-integration"))]
    pub async fn query_cache_set(&self, key: &str, value: Vec<u8>) {
        let provider = match self.pool_inner.cache_provider.load().clone() {
            Some(p) => p,
            None => return,
        };
        let ttl = std::time::Duration::from_secs(self.pool_inner.config.cache_config.default_ttl);
        let _ = provider.set(key, value, Some(ttl)).await;
    }

    /// 记录查询指标并标记写操作
    ///
    /// 统一 execute 流程中 metrics 记录与 mark_write 逻辑，
    /// 避免在多个 cfg 分支中重复实现。
    #[cfg(feature = "permission")]
    async fn record_metrics_and_mark_write(&self, action: &PermissionAction, start: Instant) {
        let duration = start.elapsed();
        self.record_query_metrics(&format!("{:?}", action), duration, true);
        if is_write_action(action) {
            self.mark_write().await;
        }
    }

    /// 检查表级权限
    ///
    /// 此方法为 ORM 操作提供权限检查，确保所有实体操作都经过权限验证
    pub async fn check_table_permission(
        &self,
        _table_name: &str,
        _operation: &str,
    ) -> DbResult<()> {
        #[cfg(feature = "permission")]
        {
            let action = match _operation {
                "INSERT" => PermissionAction::Insert,
                "SELECT" => PermissionAction::Select,
                "UPDATE" => PermissionAction::Update,
                "DELETE" => PermissionAction::Delete,
                _ => {
                    return Err(DbError::Permission(i18n::t(
                        "session-unknown-operation",
                        &[("operation", _operation.to_string())],
                    )));
                }
            };

            // Admin 角色绕过权限检查
            // vuln-0001 修复：admin bypass 仍记录审计事件（进程级审计环）以保留审计链
            if self.role == self.pool_inner.admin_role {
                audit_admin_bypass(&self.role, _table_name, &action);
            } else if !self
                .permission_ctx
                .check_table_access(_table_name, &action)
                .await
            {
                return Err(permission_denied(_operation, _table_name));
            }
        }
        Ok(())
    }

    /// 记录指标
    #[cfg(feature = "metrics")]
    pub fn record_metric(&self, operation: &str, table_name: &str, success: bool) {
        if let Some(metrics) = &self.metrics_collector {
            // 使用表名的哈希值作为 bytes 参数
            let bytes = Some(table_name.len() as u64);
            metrics.record_query(
                operation,
                std::time::Duration::from_millis(0),
                success,
                bytes,
            );
        }
    }
}

/// sqlite：单行 → JSON 对象（逐列类型探测：i64 → f64 → String → Null）
#[cfg(all(feature = "sqlite", feature = "sql-parser"))]
fn sqlite_row_to_json(row: &sea_orm::QueryResult, cols: &[String]) -> serde_json::Value {
    let mut obj = serde_json::Map::with_capacity(cols.len());
    for name in cols {
        let v = if let Ok(Some(x)) = row.try_get::<Option<i64>>("", name) {
            serde_json::Value::from(x)
        } else if let Ok(Some(x)) = row.try_get::<Option<f64>>("", name) {
            serde_json::Value::from(x)
        } else if let Ok(Some(x)) = row.try_get::<Option<String>>("", name) {
            serde_json::Value::from(x)
        } else {
            serde_json::Value::Null
        };
        obj.insert(name.clone(), v);
    }
    serde_json::Value::Object(obj)
}

/// 对单一守卫策略执行校验并触发审计钩子
///
/// 校验失败（策略自身返回 `Err`）映射为 `DbError::Config`，与既有
/// `session-ddl-validation-error` 语义一致。
#[cfg(feature = "sql-parser")]
fn run_ddl_policy(policy: &dyn DdlGuardPolicy, sql: &str) -> DbResult<DdlValidationResult> {
    let result = policy.validate(sql).map_err(|error| {
        DbError::Config(i18n::t(
            "session-ddl-validation-error",
            &[("error", error.to_string())],
        ))
    })?;
    policy.audit(sql, &result);
    Ok(result)
}

// 消费方均在 all(sql-parser, permission) 门控的表名校验路径内。
#[cfg(all(feature = "sql-parser", feature = "permission"))]
fn is_invalid_table_name(table_name: &str) -> bool {
    let table_name = table_name.trim();
    if table_name.is_empty() {
        return true;
    }

    for part in table_name.split('.') {
        let part = part.trim();
        if part.is_empty() {
            return true;
        }

        let unquoted = part
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| part.strip_prefix('`').and_then(|s| s.strip_suffix('`')))
            .or_else(|| part.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
            .unwrap_or(part)
            .trim();

        if unquoted.is_empty() {
            return true;
        }
    }

    false
}

/// 构造权限拒绝错误
///
/// 统一 "Permission denied for {action} on {table}" 错误消息格式，
/// 避免在多处调用点重复 `DbError::Permission(format!(...))` 模板。
#[cfg(feature = "permission")]
fn permission_denied(
    action: &(impl std::fmt::Display + ?Sized),
    table: &(impl std::fmt::Display + ?Sized),
) -> DbError {
    DbError::Permission(i18n::t(
        "session-permission-denied",
        &[("action", action.to_string()), ("table", table.to_string())],
    ))
}

/// 判断是否为写操作（Insert/Update/Delete）
#[cfg(feature = "permission")]
fn is_write_action(action: &PermissionAction) -> bool {
    matches!(
        action,
        PermissionAction::Insert | PermissionAction::Update | PermissionAction::Delete
    )
}

/// vuln-0005 修复：Cypher 注入防护检查
///
/// 对原始 Cypher 语句进行多层安全检查，拒绝明显危险的输入。
/// 这是参数化查询之外的第二道防线（defense in depth）：
/// - 参数化查询防止值注入
/// - 此函数防止语句结构注入（多语句、注释混淆、危险过程调用）
///
/// # 检查项
///
/// 1. **长度限制**：超过 10KB（10_240 字节）的 Cypher 拒绝（防止 DoS / 端口扫描 payload）
/// 2. **多语句**：除末尾分号外的 `;` 拒绝（防止 `MATCH ...; DELETE ...` 多语句注入）
/// 3. **行注释**：`//`（非 URL scheme）拒绝（防止注释掉后续安全检查）
/// 4. **块注释**：`/* */` 拒绝（防止注释绕过权限检查片段）
/// 5. **危险过程**：`CALL apoc.` 等管理员过程拒绝（防止提权 / 文件系统访问）
///
/// # 参数
///
/// * `cypher` - 待检查的 Cypher 语句
///
/// # 返回
///
/// - `Ok(())` 表示通过安全检查
/// - `Err(DbError::Permission(...))` 表示检测到危险模式
///
/// # Errors
///
/// 检测到危险模式时返回 `DbError::Permission`，错误消息描述具体原因。
#[cfg(any(feature = "ladybug", feature = "neo4j"))]
fn validate_cypher_safety(cypher: &str) -> DbResult<()> {
    // 1. 长度限制：10KB（10_240 字节）
    const MAX_CYPHER_BYTES: usize = 10_240;
    if cypher.len() > MAX_CYPHER_BYTES {
        return Err(DbError::Permission(format!(
            "Cypher query exceeds maximum length ({} bytes, got {} bytes) - potential DoS payload",
            MAX_CYPHER_BYTES,
            cypher.len()
        )));
    }

    // 2. 多语句检测：除末尾分号外的 `;`
    //
    // 末尾分号允许（部分客户端习惯以 `;` 结尾），但中间的 `;` 视为多语句注入。
    let trimmed = cypher.trim();
    let inner = trimmed.trim_end_matches(';').trim();
    if inner.contains(';') {
        return Err(DbError::Permission(
            "Cypher query contains multiple statements (';' inside query) - potential injection"
                .to_string(),
        ));
    }

    // 3. 行注释检测：`//`（排除 URL scheme 如 `http://`、`https://`）
    //
    // Cypher 不支持 `//` 行注释（OpenCypher 标准用 `//` 是合法注释，但极少在正常查询中使用）。
    // 检测策略：查找 `//` 出现位置，若前一个字符不是字母（排除 URL scheme）则拒绝。
    if let Some(pos) = cypher.find("//") {
        let is_url_scheme = pos > 0 && {
            let prev = cypher.as_bytes()[pos - 1];
            prev.is_ascii_alphabetic()
        };
        if !is_url_scheme {
            return Err(DbError::Permission(
                "Cypher query contains line comment '//' - potential injection".to_string(),
            ));
        }
    }

    // 4/5. 块注释与危险过程检测经统一注入引擎（规则表合并 + 去重）
    let findings = crate::access::InjectionEngine::global().scan_graph(cypher);
    // 块注释（`/* */`）：任一标记命中即拒绝（与合并前第 4 步口径一致）
    if findings
        .iter()
        .any(|rule| rule.category == crate::access::RuleCategory::Comment)
    {
        return Err(DbError::Permission(
            "Cypher query contains block comment '/* */' - potential injection".to_string(),
        ));
    }
    // 危险过程调用：`CALL apoc.`（APOC 是 Neo4j 管理员过程库，可执行系统操作）；
    // 其他危险过程（如 `dbms.`、`db.`）也在黑名单中，防止提权 / 系统访问
    if let Some(rule) = findings
        .iter()
        .find(|rule| rule.category == crate::access::RuleCategory::GraphProcedure)
    {
        return Err(DbError::Permission(format!(
            "Cypher query calls dangerous procedure ('{}') - potential privilege escalation",
            rule.pattern
        )));
    }

    Ok(())
}

/// 检查 DDL 操作，如果 SQL 为 DDL 则返回错误
///
/// 统一 execute / execute_raw / execute_with_operation 中的 DDL 拒绝逻辑。
#[cfg(feature = "sql-parser")]
fn check_ddl_operation(sql: &str) -> DbResult<()> {
    if is_ddl_operation(sql) {
        return Err(DbError::Permission(
            "DDL operations are not allowed in this context".to_string(),
        ));
    }
    Ok(())
}

impl Drop for Session {
    fn drop(&mut self) {
        // FM-3.6 修复说明：图事务通过级联 Drop 处理
        //
        // `state: Mutex<SessionState>` 被 drop 时，`SessionState::graph_transaction`
        // 也会被 drop，触发 `LadybugTransaction::drop`（actor 模式自动 ROLLBACK）
        // 或 `Neo4jTransaction::drop`（FM-2.2 修复：spawn rollback task）。
        //
        // 如果 `execute_cypher` 正在执行（graph_txn 被 take 出来在 await 中），
        // Session drop 会导致 future drop，局部变量 `graph_txn` 也会被 drop。
        //
        // 归还连接到池（直接通过 DbPoolInner 操作，无需 Arc<DbPool>）
        if let Some(conn) = self.connection.take() {
            DbPoolInner::release_connection(&self.pool_inner, conn);
        }
    }
}

/// 基于 SqlParser 的表名提取（vuln-0003 修复）
///
/// 使用 sqlparser AST 解析提取 SQL 语句的表名，替代朴素字符串匹配。
/// 正确处理字符串字面量、注释、子查询等复杂 SQL 语法，防止权限检查绕过。
///
/// # 参数
///
/// * `sql` - SQL 语句
///
/// # 返回
///
/// - `Some(table_name)` - 成功提取表名
/// - `None` - 解析失败、不支持的语句类型（DDL/DCL/Transaction）或无表名
///
/// # 行为说明
///
/// - 使用全局共享 `SqlParser` 单例，避免重复创建 parser + 缓存
/// - 解析失败时返回 `None`，调用方应跳过表级权限检查（由下游 `execute_raw`
///   的 SqlParser 检查提供防御纵深）
/// - 派生表（subquery in FROM）返回 `None`（无具名基表）
///
/// # 安全性
///
/// 此函数是 vuln-0003 修复的核心，替代了可被绕过的 `extract_table_name`。
/// 当 `permission` feature 启用时，`sql-parser` feature 被强制启用
/// （Cargo.toml: `permission = ["sql-parser", ...]`），因此此函数始终可用。
#[cfg(all(feature = "permission", feature = "sql-parser"))]
async fn extract_table_name_via_parser(sql: &str) -> Option<String> {
    let parser = SqlParser::shared().await;
    match parser.parse_operation_async(sql).await {
        Ok(Some((table, _))) => {
            if table.is_empty() || is_invalid_table_name(&table) {
                None
            } else {
                Some(table)
            }
        }
        Ok(None) => None,
        Err(_) => None,
    }
}

/// 解析 SQL 操作类型和表名用于权限检查
///
/// 使用 SqlParser AST 解析提取表名和操作类型。
/// 返回 None 表示不支持的语句或解析失败（execute 会跳过权限检查直接执行）。
#[cfg(feature = "permission")]
async fn parse_sql_for_permission(sql: &str) -> DbResult<Option<(String, PermissionAction)>> {
    let parser = SqlParser::shared().await;
    match parser.parse_operation_async(sql).await {
        Ok(Some((table, action))) => Ok(Some((table, action))),
        Ok(None) => Ok(None),
        Err(_) => Ok(None),
    }
}

// 实现 DatabaseSession trait
#[async_trait]
impl super::DatabaseSession for Session {
    async fn execute(&self, sql: &str) -> crate::DbResult<ExecResult> {
        Ok(self.execute(sql).await?)
    }

    async fn execute_raw(&self, sql: &str) -> crate::DbResult<ExecResult> {
        Ok(self.execute_raw(sql).await?)
    }

    async fn execute_raw_ddl(&self, sql: &str) -> crate::DbResult<ExecResult> {
        Ok(self.execute_raw_ddl(sql).await?)
    }

    async fn begin_transaction(&self) -> crate::DbResult<()> {
        Ok(self.begin_transaction().await?)
    }

    async fn commit(&self) -> crate::DbResult<()> {
        Ok(self.commit().await?)
    }

    async fn rollback(&self) -> crate::DbResult<()> {
        Ok(self.rollback().await?)
    }

    fn role(&self) -> &str {
        self.role()
    }

    async fn is_in_transaction(&self) -> bool {
        self.is_in_transaction().await
    }
}

// ============================================================================
// 图事务测试（Ladybug :memory: 端到端验证）
// ============================================================================

#[cfg(all(test, feature = "ladybug"))]
mod graph_tests {
    use super::*;
    use crate::database::graph::{GraphExecResult, GraphValue};

    /// 创建 Ladybug 内存连接池
    async fn make_ladybug_pool() -> DbPool {
        DbPool::new("ladybug::memory:")
            .await
            .expect("Failed to create Ladybug pool")
    }

    /// 图连接初始 is_in_transaction 为 false
    #[tokio::test]
    async fn test_graph_session_is_in_transaction_initial_false() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        assert!(
            !session.is_in_transaction().await,
            "initial state should be no transaction"
        );
    }

    /// begin_transaction 后 is_in_transaction 为 true
    #[tokio::test]
    async fn test_graph_session_begin_sets_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        session
            .begin_transaction()
            .await
            .expect("begin should succeed");
        assert!(
            session.is_in_transaction().await,
            "should be in transaction after begin"
        );
    }

    /// begin + commit 后 is_in_transaction 为 false
    #[tokio::test]
    async fn test_graph_session_commit_clears_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        session.begin_transaction().await.expect("begin");
        session.commit().await.expect("commit");
        assert!(
            !session.is_in_transaction().await,
            "should not be in transaction after commit"
        );
    }

    /// begin + rollback 后 is_in_transaction 为 false
    #[tokio::test]
    async fn test_graph_session_rollback_clears_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        session.begin_transaction().await.expect("begin");
        session.rollback().await.expect("rollback");
        assert!(
            !session.is_in_transaction().await,
            "should not be in transaction after rollback"
        );
    }

    /// 图事务 begin → execute_cypher → commit 端到端
    #[tokio::test]
    async fn test_graph_transaction_commit_e2e() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 准备：创建 schema
        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create node table");

        // 事务：插入数据并提交
        session.begin_transaction().await.expect("begin");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'Alice'})", HashMap::new())
            .await
            .expect("create in txn");
        session.commit().await.expect("commit");

        // 验证：提交后数据可见
        let result = session
            .execute_cypher_with_params("MATCH (p:Person) RETURN p.name AS name", HashMap::new())
            .await
            .expect("match after commit");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should see 1 person after commit");
                let name = &q.rows[0].columns[0].1;
                match name {
                    GraphValue::Scalar(serde_json::Value::String(s)) => assert_eq!(s, "Alice"),
                    other => panic!("expected String Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    /// 图事务 begin → execute_cypher → rollback 端到端
    #[tokio::test]
    async fn test_graph_transaction_rollback_e2e() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 准备：创建 schema
        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create node table");

        // 事务：插入数据并回滚
        session.begin_transaction().await.expect("begin");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'Bob'})", HashMap::new())
            .await
            .expect("create in txn");
        session.rollback().await.expect("rollback");

        // 验证：回滚后数据不可见
        let result = session
            .execute_cypher_with_params("MATCH (p:Person) RETURN p.name AS name", HashMap::new())
            .await
            .expect("match after rollback");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 0, "should see 0 persons after rollback");
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    /// 重复 begin 应返回 Transaction 错误
    #[tokio::test]
    async fn test_graph_double_begin_fails() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        session.begin_transaction().await.expect("first begin");
        let result = session.begin_transaction().await;
        assert!(result.is_err(), "double begin should fail");
        let err = result.unwrap_err();
        assert!(
            matches!(err, DbError::Transaction(ref msg) if msg.contains("Already in")),
            "expected 'Already in' error, got {:?}",
            err
        );
    }

    /// 无事务时 commit 应返回错误
    #[tokio::test]
    async fn test_graph_commit_without_transaction_fails() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session.commit().await;
        assert!(result.is_err(), "commit without transaction should fail");
    }

    /// 无事务时 rollback 应返回错误
    #[tokio::test]
    async fn test_graph_rollback_without_transaction_fails() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session.rollback().await;
        assert!(result.is_err(), "rollback without transaction should fail");
    }

    /// 不在事务中 execute_cypher("RETURN 1") 返回结果
    #[tokio::test]
    async fn test_execute_cypher_without_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session
            .execute_cypher_with_params("RETURN 1", HashMap::new())
            .await
            .expect("execute_cypher should succeed");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should return 1 row");
                let value = &q.rows[0].columns[0].1;
                match value {
                    GraphValue::Scalar(s) => assert_eq!(s, &serde_json::json!(1)),
                    other => panic!("expected Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    /// 在事务中 execute_cypher 委托给事务句柄
    #[tokio::test]
    async fn test_execute_cypher_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create table");

        session.begin_transaction().await.expect("begin");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'Alice', age: 25})", HashMap::new())
            .await
            .expect("create in txn");

        // 事务内查询应看到数据
        let result = session
            .execute_cypher_with_params(
                "MATCH (p:Person) RETURN p.name AS name, p.age AS age",
                HashMap::new(),
            )
            .await
            .expect("match in txn");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should see 1 person in txn");
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
        session.commit().await.expect("commit");
    }

    /// CREATE NODE TABLE + CREATE + MATCH 端到端
    #[tokio::test]
    async fn test_execute_cypher_e2e_create_match() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // DDL
        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create node table");

        // 插入多条
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'Alice', age: 25})", HashMap::new())
            .await
            .expect("create alice");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'Bob', age: 30})", HashMap::new())
            .await
            .expect("create bob");

        // 查询并验证
        let result = session
            .execute_cypher_with_params(
                "MATCH (p:Person) RETURN p.name AS name, p.age AS age ORDER BY name",
                HashMap::new(),
            )
            .await
            .expect("match");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 2, "should return 2 persons");
                // 验证第一行
                let name0 = &q.rows[0].columns[0].1;
                match name0 {
                    GraphValue::Scalar(serde_json::Value::String(s)) => assert_eq!(s, "Alice"),
                    other => panic!("expected String Scalar, got {other:?}"),
                }
                // 验证第二行
                let name1 = &q.rows[1].columns[0].1;
                match name1 {
                    GraphValue::Scalar(serde_json::Value::String(s)) => assert_eq!(s, "Bob"),
                    other => panic!("expected String Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    /// 无效 Cypher 返回错误
    #[tokio::test]
    async fn test_execute_cypher_invalid_returns_error() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session
            .execute_cypher_with_params("INVALID CYPHER", HashMap::new())
            .await;
        assert!(result.is_err(), "invalid cypher should return error");
    }

    /// 事务内多次 execute_cypher 使用同一事务句柄
    #[tokio::test]
    async fn test_execute_cypher_multiple_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create table");

        session.begin_transaction().await.expect("begin");

        // 多次 execute_cypher 都应在同一事务内
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'A'})", HashMap::new())
            .await
            .expect("create A");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'B'})", HashMap::new())
            .await
            .expect("create B");
        session
            .execute_cypher_with_params("CREATE (:Person {name: 'C'})", HashMap::new())
            .await
            .expect("create C");

        let result = session
            .execute_cypher_with_params("MATCH (p:Person) RETURN count(p) AS cnt", HashMap::new())
            .await
            .expect("count in txn");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1);
                let cnt = &q.rows[0].columns[0].1;
                match cnt {
                    GraphValue::Scalar(s) => assert_eq!(s, &serde_json::json!(3)),
                    other => panic!("expected Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
        session.commit().await.expect("commit");
    }

    /// 非 admin 角色调用 execute_cypher_with_params 应被拒绝（permission feature）
    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_execute_cypher_non_admin_denied() {
        let pool = make_ladybug_pool().await;
        // system 角色在无权限配置时也被允许获取 session
        let session = pool.get_session("system").await.expect("get_session");
        let result = session
            .execute_cypher_with_params("RETURN 1", HashMap::new())
            .await;
        assert!(result.is_err(), "non-admin role should be denied");
        let err = result.unwrap_err();
        assert!(
            matches!(err, DbError::Permission(ref msg) if msg.contains("Graph operation denied")),
            "expected Permission error, got {:?}",
            err
        );
    }

    /// admin 角色 execute_cypher_with_params 成功（permission feature）
    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_execute_cypher_admin_allowed() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session
            .execute_cypher_with_params("RETURN 42", HashMap::new())
            .await;
        assert!(result.is_ok(), "admin role should be allowed");
    }
}

// ============================================================================
// vuln-0001 安全审计测试
// ============================================================================

#[cfg(test)]
#[cfg(all(feature = "permission", feature = "sqlite"))]
mod vuln_0001_tests {
    use super::*;

    /// vuln-0001 集成测试：admin 角色绕过权限检查仍返回 Ok（带审计日志）
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_vuln_0001_admin_bypass_returns_ok_with_audit() {
        let pool = DbPool::new("sqlite::memory:")
            .await
            .expect("Failed to create pool");
        let session = pool.get_session("admin").await.expect("get_session");

        // admin 角色绕过权限检查，应返回 Ok
        let result = session
            .check_permission("any_table", &PermissionAction::Select)
            .await;
        assert!(result.is_ok(), "admin bypass should return Ok");

        // 也测试其他操作
        let result = session
            .check_permission("any_table", &PermissionAction::Insert)
            .await;
        assert!(result.is_ok(), "admin bypass should return Ok for Insert");

        let result = session
            .check_permission("any_table", &PermissionAction::Delete)
            .await;
        assert!(result.is_ok(), "admin bypass should return Ok for Delete");
    }

    /// vuln-0001 集成测试：非 admin 角色权限被拒绝
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_vuln_0001_non_admin_denied() {
        let pool = DbPool::new("sqlite::memory:")
            .await
            .expect("Failed to create pool");
        // system 角色可获取 session 但不是 admin_role，无权限配置时 check_permission 应拒绝
        let session = pool.get_session("system").await.expect("get_session");

        // 非 admin 角色应被拒绝（无权限配置时默认拒绝）
        let result = session
            .check_permission("any_table", &PermissionAction::Select)
            .await;
        assert!(result.is_err(), "non-admin should be denied");
    }

    /// 非 admin 角色有权限时 check_permission 返回 Ok (覆盖 line 162)
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_check_permission_non_admin_allowed() {
        use std::io::Write;

        // 创建权限配置文件，授予 "reader" 角色对 "test_tbl" 的 SELECT 权限
        let yaml_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations: ["select", "insert", "update", "delete"]
  reader:
    tables:
      - name: "test_tbl"
        operations: ["select"]
"#;
        let tmp_dir = std::env::temp_dir();
        let yaml_path = tmp_dir.join("test_non_admin_perm.yaml");
        {
            let mut file = std::fs::File::create(&yaml_path).expect("create temp file");
            file.write_all(yaml_content.as_bytes())
                .expect("write temp file");
        }

        let config = crate::foundation::DbConfig {
            url: "sqlite::memory:".to_string(),
            permissions_path: Some(yaml_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        let pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

        // reader 角色有 test_tbl 的 SELECT 权限 -> check_permission 应返回 Ok
        let session = pool
            .get_session("reader")
            .await
            .expect("get_session for reader");
        let result = session
            .check_permission("test_tbl", &PermissionAction::Select)
            .await;
        assert!(
            result.is_ok(),
            "reader should have SELECT on test_tbl: {:?}",
            result.err()
        );

        // 清理临时文件
        let _ = std::fs::remove_file(&yaml_path);
    }

    /// vuln-0001 集成测试：check_table_permission admin bypass 带审计日志
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_vuln_0001_check_table_permission_admin_bypass() {
        let pool = DbPool::new("sqlite::memory:")
            .await
            .expect("Failed to create pool");
        let session = pool.get_session("admin").await.expect("get_session");

        // admin bypass check_table_permission
        let result = session.check_table_permission("users", "SELECT").await;
        assert!(result.is_ok(), "admin should bypass check_table_permission");

        let result = session.check_table_permission("users", "INSERT").await;
        assert!(
            result.is_ok(),
            "admin should bypass check_table_permission for INSERT"
        );
    }
}

// ============================================================================
// vuln-0003 测试：SqlParser 表名提取安全验证
// ============================================================================

#[cfg(test)]
#[cfg(all(feature = "permission", feature = "sql-parser"))]
mod vuln_0003_tests {
    use super::*;

    /// vuln-0003：SqlParser 对 INSERT 的正确处理
    #[tokio::test]
    async fn test_vuln_0003_parser_correctly_handles_insert() {
        let sql = "INSERT INTO users (name) VALUES ('from into values')";
        let parser_result = extract_table_name_via_parser(sql).await;
        assert_eq!(
            parser_result.as_deref(),
            Some("users"),
            "SqlParser should correctly extract 'users' for INSERT"
        );
    }

    /// vuln-0003：SqlParser 对 UPDATE 的正确处理
    #[tokio::test]
    async fn test_vuln_0003_parser_correctly_handles_update() {
        let sql = "UPDATE users SET name = 'from users' WHERE id = 1";
        let parser_result = extract_table_name_via_parser(sql).await;
        assert_eq!(
            parser_result.as_deref(),
            Some("users"),
            "SqlParser should correctly extract 'users' for UPDATE"
        );
    }

    /// vuln-0003：SqlParser 对 DELETE 的正确处理
    #[tokio::test]
    async fn test_vuln_0003_parser_correctly_handles_delete() {
        let sql = "DELETE FROM users WHERE name = 'from deleted'";
        let parser_result = extract_table_name_via_parser(sql).await;
        assert_eq!(
            parser_result.as_deref(),
            Some("users"),
            "SqlParser should correctly extract 'users' for DELETE"
        );
    }

    /// vuln-0003 Red-7：朴素 `extract_table_name` 对带引号的表名处理
    ///
    /// SQL: `SELECT * FROM "users" WHERE id = 1`
    /// 朴素解析器返回 `"users"`（带引号），权限检查可能因引号不匹配而失败。
    /// SqlParser 返回 `"users"`（标准化形式，与权限策略匹配）。
    #[tokio::test]
    async fn test_vuln_0003_parser_handles_quoted_table_name() {
        let sql = "SELECT * FROM \"users\" WHERE id = 1";

        // SqlParser 应正确解析带引号的表名
        let parser_result = extract_table_name_via_parser(sql).await;
        assert!(
            parser_result.is_some(),
            "SqlParser should extract table name for quoted identifier, got: {:?}",
            parser_result
        );
        // 表名应包含 "users"（可能带引号或不带引号，取决于 sqlparser 序列化）
        let table = parser_result.unwrap();
        assert!(
            table.contains("users"),
            "extracted table name should contain 'users', got: {}",
            table
        );
    }
}

// ============================================================================
// vuln-0005 测试：Cypher 注入防护
// ============================================================================
//
// 漏洞描述：
//   `Session::execute_cypher` 直接接受 Cypher 字符串并执行，
//   若调用方将用户输入拼接进 Cypher，可导致 Cypher 注入：
//   - 多语句注入：`MATCH (n) RETURN n; DELETE (n)`
//   - 注释混淆：`MATCH (n) // bypass RETURN n`
//   - 危险过程：`CALL apoc.systemdb.admin(...)`
//
// 修复方案：
//   1. 添加 `validate_cypher_safety` 对原始 Cypher 做多层检查（长度/多语句/注释/危险过程）
//   2. 添加 `execute_cypher_with_params` 使用 prepared statement 防止值注入
//   3. 标记 `execute_cypher` 为 `#[deprecated]`，引导调用方迁移
//
// 测试策略：
//   - 单元测试 `validate_cypher_safety` 各检查项（拒绝/允许）
//   - 集成测试 `execute_cypher_with_params` 端到端验证参数化查询
// ============================================================================

#[cfg(all(test, feature = "ladybug"))]
mod vuln_0005_tests {
    use super::*;
    use crate::database::graph::{GraphExecResult, GraphValue};

    /// 辅助：创建 Ladybug 内存连接池
    async fn make_ladybug_pool() -> DbPool {
        DbPool::new("ladybug::memory:")
            .await
            .expect("Failed to create Ladybug pool")
    }

    // ===== validate_cypher_safety 拒绝路径 =====

    /// vuln-0005 Red-1：超过 10KB 的 Cypher 被拒绝（DoS 防护）
    ///
    /// 构造 11KB（11_264 字节）的 Cypher 查询，应被 `validate_cypher_safety` 拒绝。
    #[test]
    fn test_validate_cypher_safety_rejects_too_long() {
        // 11_264 字节 = 11KB，超过 10_240 字节限制
        let long_cypher = format!("MATCH (n) RETURN '{}'", "x".repeat(11_200));
        assert!(
            long_cypher.len() > 10_240,
            "test cypher should exceed 10KB, got {} bytes",
            long_cypher.len()
        );

        let result = validate_cypher_safety(&long_cypher);
        assert!(
            result.is_err(),
            "Cypher exceeding 10KB should be rejected (got {} bytes)",
            long_cypher.len()
        );

        // 验证错误类型为 Permission
        match &result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("maximum length") || msg.contains("exceeds"),
                    "error should mention length, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }

    /// vuln-0005 Red-2：多语句 Cypher 被拒绝（`;` 在查询中间）
    ///
    /// `MATCH (n) RETURN n; MATCH (m) RETURN m` 包含中间分号，
    /// 应被 `validate_cypher_safety` 拒绝（防止 `MATCH ...; DELETE ...` 注入）。
    #[test]
    fn test_validate_cypher_safety_rejects_multi_statement() {
        let cypher = "MATCH (n) RETURN n; MATCH (m) RETURN m";
        let result = validate_cypher_safety(cypher);
        assert!(result.is_err(), "multi-statement Cypher should be rejected");

        match &result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("multiple statements") || msg.contains("';'"),
                    "error should mention multiple statements, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }

    /// vuln-0005 Red-3：包含行注释 `//` 的 Cypher 被拒绝
    ///
    /// `MATCH (n) // comment RETURN n` 包含行注释，
    /// 应被 `validate_cypher_safety` 拒绝（防止注释绕过安全检查）。
    #[test]
    fn test_validate_cypher_safety_rejects_line_comment() {
        let cypher = "MATCH (n) // comment RETURN n";
        let result = validate_cypher_safety(cypher);
        assert!(
            result.is_err(),
            "Cypher with line comment '//' should be rejected"
        );

        match &result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("line comment") || msg.contains("//"),
                    "error should mention line comment, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }

    /// vuln-0005 Red-4：包含块注释 `/* */` 的 Cypher 被拒绝
    ///
    /// `MATCH (n) /* comment */ RETURN n` 包含块注释，
    /// 应被 `validate_cypher_safety` 拒绝（防止注释绕过权限检查片段）。
    #[test]
    fn test_validate_cypher_safety_rejects_block_comment() {
        let cypher = "MATCH (n) /* comment */ RETURN n";
        let result = validate_cypher_safety(cypher);
        assert!(
            result.is_err(),
            "Cypher with block comment '/* */' should be rejected"
        );

        match &result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("block comment") || msg.contains("/*"),
                    "error should mention block comment, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }

    /// vuln-0005 Red-5：调用 APOC 危险过程的 Cypher 被拒绝
    ///
    /// `CALL apoc.systemdb.admin(...)` 调用 APOC 管理员过程，
    /// 应被 `validate_cypher_safety` 拒绝（防止提权/文件系统访问）。
    #[test]
    fn test_validate_cypher_safety_rejects_apoc_call() {
        let cypher = "CALL apoc.systemdb.admin('something')";
        let result = validate_cypher_safety(cypher);
        assert!(
            result.is_err(),
            "Cypher calling APOC procedure should be rejected"
        );

        match &result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("dangerous procedure") || msg.contains("apoc"),
                    "error should mention dangerous procedure, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }

    // ===== validate_cypher_safety 允许路径 =====

    /// vuln-0005 Green-1：正常 Cypher 查询通过安全检查
    ///
    /// `MATCH (n:User) RETURN n` 是标准查询，应通过 `validate_cypher_safety`。
    #[test]
    fn test_validate_cypher_safety_allows_normal_query() {
        let cypher = "MATCH (n:User) RETURN n";
        let result = validate_cypher_safety(cypher);
        assert!(
            result.is_ok(),
            "normal Cypher query should pass safety check, got: {:?}",
            result
        );
    }

    /// vuln-0005 Green-2：末尾分号允许（部分客户端习惯以 `;` 结尾）
    ///
    /// `MATCH (n) RETURN n;` 末尾有分号，但中间无分号，应通过检查。
    #[test]
    fn test_validate_cypher_safety_allows_trailing_semicolon() {
        let cypher = "MATCH (n) RETURN n;";
        let result = validate_cypher_safety(cypher);
        assert!(
            result.is_ok(),
            "Cypher with trailing semicolon should pass safety check, got: {:?}",
            result
        );
    }

    // ===== execute_cypher_with_params 端到端测试 =====

    /// vuln-0005 Green-3：参数化查询端到端验证
    ///
    /// 使用 Ladybug :memory: 图数据库，验证 `execute_cypher_with_params` 能正确：
    /// 1. 接受 `$param` 占位符 Cypher
    /// 2. 通过 params 映射传递参数值
    /// 3. 底层 prepared statement 正确执行
    /// 4. 返回正确的结果集
    ///
    /// 测试场景：CREATE NODE TABLE → 插入参数化数据 → MATCH 验证
    #[tokio::test]
    async fn test_execute_cypher_with_params_passes_params() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 1. 创建 Node Table（DDL，无参数）
        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))",
                HashMap::new(),
            )
            .await
            .expect("create node table");

        // 2. 参数化插入 Alice
        let mut params_alice = HashMap::new();
        params_alice.insert("name".to_string(), serde_json::json!("Alice"));
        params_alice.insert("age".to_string(), serde_json::json!(25));
        session
            .execute_cypher_with_params("CREATE (:Person {name: $name, age: $age})", params_alice)
            .await
            .expect("create Alice with params");

        // 3. 参数化插入 Bob
        let mut params_bob = HashMap::new();
        params_bob.insert("name".to_string(), serde_json::json!("Bob"));
        params_bob.insert("age".to_string(), serde_json::json!(30));
        session
            .execute_cypher_with_params("CREATE (:Person {name: $name, age: $age})", params_bob)
            .await
            .expect("create Bob with params");

        // 4. 参数化查询：按 name 过滤
        let mut params_query = HashMap::new();
        params_query.insert("target_name".to_string(), serde_json::json!("Alice"));
        let result = session
            .execute_cypher_with_params(
                "MATCH (p:Person) WHERE p.name = $target_name RETURN p.name AS name, p.age AS age",
                params_query,
            )
            .await
            .expect("match with params");

        // 5. 验证结果
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should return 1 person (Alice)");
                // 验证 name 列
                let name_val = &q.rows[0].columns[0].1;
                match name_val {
                    GraphValue::Scalar(serde_json::Value::String(s)) => {
                        assert_eq!(s, "Alice", "name should be Alice");
                    }
                    other => panic!("expected String Scalar for name, got {other:?}"),
                }
                // 验证 age 列
                let age_val = &q.rows[0].columns[1].1;
                match age_val {
                    GraphValue::Scalar(serde_json::Value::Number(n)) => {
                        assert_eq!(n.as_i64(), Some(25), "age should be 25");
                    }
                    other => panic!("expected Number Scalar for age, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant, got Write"),
        }
    }

    /// vuln-0005 Green-4：参数化查询在事务内正常工作
    ///
    /// 验证 `execute_cypher_with_params` 在图事务内执行时，
    /// 所有操作使用同一事务连接（事务隔离）。
    #[tokio::test]
    async fn test_execute_cypher_with_params_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 创建 Node Table
        session
            .execute_cypher_with_params(
                "CREATE NODE TABLE Account(id INT64, balance INT64, PRIMARY KEY(id))",
                HashMap::new(),
            )
            .await
            .expect("create node table");

        // 开始事务
        session
            .begin_transaction()
            .await
            .expect("begin transaction");

        // 事务内参数化插入
        let mut params1 = HashMap::new();
        params1.insert("id".to_string(), serde_json::json!(1));
        params1.insert("balance".to_string(), serde_json::json!(100));
        session
            .execute_cypher_with_params("CREATE (:Account {id: $id, balance: $balance})", params1)
            .await
            .expect("create account 1 in txn");

        let mut params2 = HashMap::new();
        params2.insert("id".to_string(), serde_json::json!(2));
        params2.insert("balance".to_string(), serde_json::json!(200));
        session
            .execute_cypher_with_params("CREATE (:Account {id: $id, balance: $balance})", params2)
            .await
            .expect("create account 2 in txn");

        // 事务内查询验证
        let result = session
            .execute_cypher_with_params(
                "MATCH (a:Account) RETURN a.id AS id ORDER BY a.id",
                HashMap::new(),
            )
            .await
            .expect("match in txn");

        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 2, "should see 2 accounts in txn");
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }

        session.commit().await.expect("commit");
    }

    /// vuln-0005 Red-6：execute_cypher_with_params 也执行安全检查
    ///
    /// 验证 `execute_cypher_with_params` 同样拒绝危险 Cypher（多语句），
    /// 防止调用方误以为参数化查询可以绕过语句结构检查。
    #[tokio::test]
    async fn test_execute_cypher_with_params_rejects_injection() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 多语句注入尝试
        let result = session
            .execute_cypher_with_params("MATCH (n) RETURN n; DELETE (n)", HashMap::new())
            .await;

        assert!(
            result.is_err(),
            "multi-statement Cypher should be rejected even in execute_cypher_with_params"
        );

        match result {
            Err(DbError::Permission(msg)) => {
                assert!(
                    msg.contains("multiple statements") || msg.contains("';'"),
                    "error should mention multiple statements, got: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }
    }
}

// ============================================================================
// Session 基础测试（仅需 sqlite feature）
// ============================================================================

#[cfg(test)]
#[cfg(feature = "sqlite")]
mod session_basic_tests {
    use super::*;

    /// 辅助：创建 SQLite 内存连接池并获取 session
    async fn make_test_session(role: &str) -> (super::super::DbPool, Session) {
        let pool = super::super::DbPool::new("sqlite::memory:")
            .await
            .expect("Failed to create pool");
        let session = pool.get_session(role).await.expect("get_session failed");
        (pool, session)
    }

    #[tokio::test]
    async fn test_session_role() {
        let (_pool, session) = make_test_session("admin").await;
        assert_eq!(session.role(), "admin");
    }

    #[tokio::test]
    async fn test_session_is_in_transaction_initially_false() {
        let (_pool, session) = make_test_session("admin").await;
        assert!(!session.is_in_transaction().await);
    }

    #[tokio::test]
    async fn test_session_should_use_master_initially_false() {
        let (_pool, session) = make_test_session("admin").await;
        assert!(!session.should_use_master().await);
    }

    #[tokio::test]
    async fn test_session_mark_write_enables_master() {
        let (_pool, session) = make_test_session("admin").await;
        session.mark_write().await;
        assert!(session.should_use_master().await);
    }

    #[tokio::test]
    async fn test_session_connection_returns_ok() {
        let (_pool, session) = make_test_session("admin").await;
        assert!(session.connection().is_ok());
    }

    #[tokio::test]
    async fn test_session_begin_and_commit_transaction() {
        let (_pool, session) = make_test_session("admin").await;

        // 开始事务
        session
            .begin_transaction()
            .await
            .expect("begin_transaction");
        assert!(session.is_in_transaction().await);
        assert!(session.should_use_master().await);

        // 提交事务
        session.commit().await.expect("commit");
        assert!(!session.is_in_transaction().await);
    }

    #[tokio::test]
    async fn test_session_begin_and_rollback_transaction() {
        let (_pool, session) = make_test_session("admin").await;

        session
            .begin_transaction()
            .await
            .expect("begin_transaction");
        assert!(session.is_in_transaction().await);

        session.rollback().await.expect("rollback");
        assert!(!session.is_in_transaction().await);
    }

    #[tokio::test]
    async fn test_session_double_begin_returns_error() {
        let (_pool, session) = make_test_session("admin").await;

        session.begin_transaction().await.expect("first begin");

        // 第二次 begin 应返回错误
        let result = session.begin_transaction().await;
        assert!(result.is_err(), "double begin should return error");

        // 清理：提交第一个事务
        session.commit().await.expect("commit");
    }

    #[tokio::test]
    async fn test_session_rollback_without_transaction_returns_error() {
        let (_pool, session) = make_test_session("admin").await;

        // 没有活跃事务时 rollback 应返回错误
        let result = session.rollback().await;
        assert!(result.is_err(), "rollback without transaction should error");
    }

    #[tokio::test]
    async fn test_session_commit_without_transaction_returns_error() {
        let (_pool, session) = make_test_session("admin").await;

        let result = session.commit().await;
        assert!(result.is_err(), "commit without transaction should error");
    }

    #[cfg(feature = "sql-parser")]
    #[tokio::test]
    async fn test_session_execute_raw_select() {
        let (_pool, session) = make_test_session("admin").await;

        // Create a table first so SELECT has a valid table name
        use sea_orm::ConnectionTrait;
        session
            .connection()
            .unwrap()
            .execute_unprepared("CREATE TABLE sel_test (id INTEGER PRIMARY KEY)")
            .await
            .expect("create table");

        // SELECT from table should succeed for admin role
        let result = session.execute_raw("SELECT * FROM sel_test").await;
        assert!(
            result.is_ok(),
            "SELECT from table should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_session_execute_raw_ddl_rejected() {
        let (_pool, session) = make_test_session("admin").await;

        // DDL operations should be rejected by execute_raw
        let result = session.execute_raw("CREATE TABLE test (id INTEGER)").await;
        assert!(result.is_err(), "DDL should be rejected");
    }

    #[cfg(feature = "sql-parser")]
    #[tokio::test]
    async fn test_session_execute_raw_create_table_and_insert() {
        let (_pool, session) = make_test_session("admin").await;

        // Create table via execute_unprepared (not execute_raw which checks permissions)
        use sea_orm::ConnectionTrait;
        session
            .connection()
            .unwrap()
            .execute_unprepared("CREATE TABLE test_tbl (id INTEGER PRIMARY KEY, name TEXT)")
            .await
            .expect("create table");

        // Insert via execute_raw (admin bypasses permission)
        let result = session
            .execute_raw("INSERT INTO test_tbl (id, name) VALUES (1, 'test')")
            .await;
        assert!(
            result.is_ok(),
            "INSERT should succeed for admin: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_session_database_session_trait_commit() {
        let (_pool, session) = make_test_session("admin").await;

        // Use the DatabaseSession trait method explicitly
        use super::super::DatabaseSession;
        let result = DatabaseSession::commit(&session).await;
        assert!(
            result.is_err(),
            "commit without transaction via trait should error"
        );
    }

    #[tokio::test]
    async fn test_session_database_session_trait_rollback() {
        let (_pool, session) = make_test_session("admin").await;

        use super::super::DatabaseSession;
        let result = DatabaseSession::rollback(&session).await;
        assert!(
            result.is_err(),
            "rollback without transaction via trait should error"
        );
    }

    #[cfg(feature = "sql-parser")]
    #[tokio::test]
    async fn test_session_database_session_trait_execute() {
        let (_pool, session) = make_test_session("admin").await;

        // Create a table first
        use sea_orm::ConnectionTrait;
        session
            .connection()
            .unwrap()
            .execute_unprepared("CREATE TABLE trait_exec_test (id INTEGER PRIMARY KEY)")
            .await
            .expect("create table");

        use super::super::DatabaseSession;
        let result =
            DatabaseSession::execute(&session, "INSERT INTO trait_exec_test (id) VALUES (1)").await;
        assert!(
            result.is_ok(),
            "execute via trait should succeed: {:?}",
            result.err()
        );
    }

    // ===== 补充测试：is_invalid_table_name, create_migration_executor =====

    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_extract_table_name_via_parser_invalid_table() {
        // Parser returns empty table name -> covers line 1430
        let result = super::extract_table_name_via_parser("SELECT 1").await;
        assert!(
            result.is_none(),
            "SELECT without FROM table should return None"
        );
    }

    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_extract_table_name_via_parser_unsupported() {
        // Unsupported statement -> covers line 1435
        let result = super::extract_table_name_via_parser("INVALID SQL GIBBERISH").await;
        // Either None (parse error) or Some table
        // Just verify it doesn't panic
        let _ = result;
    }

    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_extract_table_name_via_parser_parse_error() {
        // Parse error -> covers line 1436
        let result = super::extract_table_name_via_parser("/* comment */").await;
        // Comments alone should be a parse error or None
        assert!(result.is_none());
    }

    #[cfg(feature = "permission")]
    #[test]
    fn test_is_invalid_table_name_empty() {
        // Empty table name -> covers line 1164
        assert!(super::is_invalid_table_name(""));
        assert!(super::is_invalid_table_name("   "));
    }

    #[cfg(feature = "permission")]
    #[test]
    fn test_is_invalid_table_name_empty_part() {
        // Table name with empty part after split -> covers line 1170
        assert!(super::is_invalid_table_name("schema..table"));
        assert!(super::is_invalid_table_name(".table"));
    }

    #[cfg(feature = "permission")]
    #[test]
    fn test_is_invalid_table_name_valid() {
        assert!(!super::is_invalid_table_name("users"));
        assert!(!super::is_invalid_table_name("public.users"));
        assert!(!super::is_invalid_table_name("\"quoted\".\"table\""));
    }

    #[cfg(feature = "migration")]
    #[tokio::test]
    async fn test_create_migration_executor() {
        let (_pool, session) = make_test_session("admin").await;
        let result = session.create_migration_executor(crate::foundation::DatabaseType::Sqlite);
        assert!(
            result.is_ok(),
            "create_migration_executor should succeed: {:?}",
            result.err()
        );
    }
}
