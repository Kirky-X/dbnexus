// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Session 模块
//!
//! 提供数据库会话管理，包括事务、权限检查和读写分离

use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(any(feature = "ladybug", feature = "neo4j"))]
use std::collections::HashMap;

use super::db_pool::DbPoolInner;
use super::{DatabaseConnection, DbConnection, DbPool};
#[cfg(all(feature = "sql-parser", feature = "permission"))]
use crate::access::SqlParser;
#[cfg(feature = "sql-parser")]
use crate::access::is_ddl_operation;
#[cfg(feature = "sql-parser")]
use crate::access::{DdlGuard, DdlValidationResult};
#[cfg(feature = "permission")]
use crate::access::{PermissionAction, PermissionContext};
use crate::foundation::{DbError, DbResult};
#[cfg(feature = "metrics")]
use crate::observability::MetricsCollector;
use async_trait::async_trait;

// 导入 Sea-ORM 的事务 trait 和连接 trait
use sea_orm::{ConnectionTrait, DatabaseTransaction, ExecResult, TransactionTrait};
use tokio::sync::Mutex;

/// Session 内部可变状态
///
/// 使用 Mutex 包装需要内部可变性的字段，支持 `&self` 方法签名
struct SessionState {
    /// 事务对象（用于真实的事务管理）
    ///
    /// v0.3.0 性能优化：使用 `Arc<DatabaseTransaction>` 而非 `DatabaseTransaction`，
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
    last_write: Option<Instant>,
}

/// Session 结构
pub struct Session {
    /// 数据库连接（统一枚举：SeaORM 或 DuckDB）
    connection: Option<DbConnection>,

    /// 连接池（用于释放连接）
    pool: Arc<DbPool>,

    /// 连接池内部状态
    pool_inner: Arc<DbPoolInner>,

    /// 角色
    role: String,

    /// 权限上下文
    #[cfg(feature = "permission")]
    permission_ctx: PermissionContext,

    /// 内部可变状态（事务和写操作时间）
    state: Mutex<SessionState>,

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
    pub(crate) fn new(connection: DbConnection, pool: Arc<DbPool>, pool_inner: Arc<DbPoolInner>, role: String) -> Self {
        #[cfg(feature = "permission")]
        let permission_ctx = PermissionContext::new(role.clone(), pool_inner.policy_cache.clone());

        #[cfg(feature = "metrics")]
        let metrics = pool_inner.metrics_collector.clone();

        Session {
            connection: Some(connection),
            pool,
            pool_inner,
            role,
            #[cfg(feature = "permission")]
            permission_ctx,
            state: Mutex::new(SessionState {
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
        let mut state = self.state.lock().await;
        state.last_write = Some(Instant::now());
    }

    /// 检查权限
    #[cfg(feature = "permission")]
    pub async fn check_permission(&self, table: &str, operation: &PermissionAction) -> Result<(), DbError> {
        // Admin 角色绕过权限检查（拥有完全控制权）
        // vuln-0001 修复：admin bypass 仍记录审计日志以保留审计链
        if self.role == self.pool_inner.admin_role {
            audit_admin_bypass(&self.role, table, operation);
            return Ok(());
        }

        if self.permission_ctx.check_table_access(table, operation).await {
            Ok(())
        } else {
            Err(permission_denied(operation, table))
        }
    }

    /// 是否在事务中
    ///
    /// 图事务或关系型事务任一存在都返回 true。
    pub async fn is_in_transaction(&self) -> bool {
        let state = self.state.lock().await;
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
    /// v0.3.0 性能优化：短锁模式，避免持锁期间 async DB 调用。
    /// 流程：短锁检查 → 锁外 begin → 短锁写入（含并发冲突处理）
    ///
    /// 图事务双轨：按连接类型分发到关系型（SeaORM）或图（GraphConnection）事务路径。
    pub async fn begin_transaction(&self) -> Result<(), DbError> {
        // 短锁：检查是否已在事务中
        {
            let state = self.state.lock().await;
            #[cfg(any(feature = "ladybug", feature = "neo4j"))]
            if state.graph_transaction.is_some() {
                return Err(DbError::Transaction("Already in graph transaction".to_string()));
            }
            if state.transaction.is_some() {
                return Err(DbError::Transaction("Already in transaction".to_string()));
            }
        }

        // 获取连接
        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Config("Connection not available - Session may have been invalidated".to_string())
        })?;

        // 图连接分发：调用 begin_graph_txn
        #[cfg(any(feature = "ladybug", feature = "neo4j"))]
        if conn.is_graph() {
            let graph = conn.as_graph()?;
            let graph_txn = graph
                .begin_graph_txn()
                .await
                .map_err(|e| DbError::Transaction(format!("Failed to begin graph transaction: {}", e)))?;

            // 短锁：写入 graph_transaction（含并发冲突处理）
            let mut state = self.state.lock().await;
            if state.graph_transaction.is_some() {
                // 并发冲突：两次锁之间有其他调用已开始事务，回滚新创建的图事务
                let _ = graph_txn.rollback().await;
                return Err(DbError::Transaction(
                    "Already in graph transaction (concurrent begin detected)".to_string(),
                ));
            }
            state.graph_transaction = Some(graph_txn);
            return Ok(());
        }

        // SeaORM 逻辑：锁外执行 async DB 操作
        let conn = conn.as_sea_orm()?;
        let transaction = conn
            .begin()
            .await
            .map_err(|e| DbError::Transaction(format!("Failed to begin transaction: {}", e)))?;

        // 短锁：写入 transaction（含并发冲突处理）
        let mut state = self.state.lock().await;
        if state.transaction.is_some() {
            // 并发冲突：两次锁之间有其他调用已开始事务，回滚新创建的事务
            let _ = transaction.rollback().await;
            return Err(DbError::Transaction(
                "Already in transaction (concurrent begin detected)".to_string(),
            ));
        }
        state.transaction = Some(Arc::new(transaction));
        Ok(())
    }

    /// 提交事务
    ///
    /// v0.3.0 性能优化：短锁模式，take transaction 后锁外执行 commit。
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
                let mut state = self.state.lock().await;
                state.graph_transaction.take()
            };
            if let Some(graph_txn) = graph_txn {
                // 锁外：执行 async commit（commit 消耗 self）
                graph_txn
                    .commit()
                    .await
                    .map_err(|e| DbError::Transaction(format!("Failed to commit graph transaction: {}", e)))?;

                // 短锁：清除 last_write
                let mut state = self.state.lock().await;
                state.last_write = None;
                return Ok(());
            }
        }

        // SeaORM 逻辑：短锁 take transaction
        let transaction_arc = {
            let mut state = self.state.lock().await;
            state
                .transaction
                .take()
                .ok_or_else(|| DbError::Transaction("No active transaction to commit".to_string()))?
        };

        // 锁外：try_unwrap 解包 Arc（如果有并发查询持有引用，会失败）
        let transaction = Arc::try_unwrap(transaction_arc).map_err(|_| {
            DbError::Transaction("Cannot commit: transaction is in use by a concurrent query".to_string())
        })?;

        // 锁外：执行 async commit（commit 消耗 self）
        transaction
            .commit()
            .await
            .map_err(|e| DbError::Transaction(e.to_string()))?;

        // 短锁：清除 last_write
        let mut state = self.state.lock().await;
        state.last_write = None;
        Ok(())
    }

    /// 回滚事务
    ///
    /// v0.3.0 性能优化：短锁模式，take transaction 后锁外执行 rollback。
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
                let mut state = self.state.lock().await;
                state.graph_transaction.take()
            };
            if let Some(graph_txn) = graph_txn {
                // 锁外：执行 async rollback（rollback 消耗 self）
                graph_txn
                    .rollback()
                    .await
                    .map_err(|e| DbError::Transaction(format!("Failed to rollback graph transaction: {}", e)))?;
                return Ok(());
            }
        }

        // SeaORM 逻辑：短锁 take transaction
        let transaction_arc = {
            let mut state = self.state.lock().await;
            if state.transaction.is_none() {
                return Err(DbError::Transaction("Not in transaction".to_string()));
            }
            state
                .transaction
                .take()
                .ok_or_else(|| DbError::Transaction("No active transaction to rollback".to_string()))?
        };

        // 锁外：try_unwrap 解包 Arc
        let transaction = Arc::try_unwrap(transaction_arc).map_err(|_| {
            DbError::Transaction("Cannot rollback: transaction is in use by a concurrent query".to_string())
        })?;

        // 锁外：执行 async rollback（rollback 消耗 self）
        transaction
            .rollback()
            .await
            .map_err(|e| DbError::Transaction(format!("Failed to rollback transaction: {}", e)))?;

        Ok(())
    }

    /// 是否应该使用主库（基于读写分离配置）
    pub async fn should_use_master(&self) -> bool {
        let state = self.state.lock().await;
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
            .ok_or_else(|| DbError::Config("Connection not available - Session may have been invalidated".to_string()))?
            .as_sea_orm()
    }

    /// 创建迁移执行器（仅内部使用）
    ///
    /// 用于迁移功能，将底层连接包装成 MigrationExecutor
    #[allow(dead_code)]
    #[cfg(feature = "migration")]
    pub fn create_migration_executor(
        &self,
        db_type: crate::foundation::DatabaseType,
    ) -> Result<super::MigrationExecutor, DbError> {
        let conn = self.connection()?.clone();
        Ok(super::MigrationExecutor::new(conn, db_type))
    }

    /// 执行原始 SQL（带权限检查）
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self), fields(db.role = %self.role)))]
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
                match parser.parse_operation_async(sql).await {
                    Ok(Some((table_name, action))) => {
                        if table_name.is_empty() || is_invalid_table_name(&table_name) {
                            return Err(DbError::Permission(
                                "Failed to extract table name for permission checking".to_string(),
                            ));
                        }
                        // 检查权限
                        // Admin 角色绕过权限检查
                        if self.role == self.pool_inner.admin_role {
                            // admin 有完全权限，跳过检查
                        } else if !self.permission_ctx.check_table_access(&table_name, &action).await {
                            return Err(permission_denied(&action, &table_name));
                        }
                    }
                    Ok(None) => {
                        // 解析成功但是不支持的语句类型（DDL/DCL/Transaction）或没有表名的语句
                        // 这些情况需要拒绝执行以确保安全
                        return Err(DbError::Permission(
                            "SQL statement requires a valid table name for permission checking".to_string(),
                        ));
                    }
                    Err(_) => {
                        // 解析失败，拒绝执行
                        return Err(DbError::Permission(
                            "Failed to parse SQL statement for permission checking".to_string(),
                        ));
                    }
                }
            }

            // v0.3.0 性能优化：短锁 clone Arc<DatabaseTransaction>，锁外执行 async DB 调用
            let tx_opt: Option<Arc<DatabaseTransaction>> = {
                let state = self.state.lock().await;
                state.transaction.clone()
            };
            if let Some(tx) = tx_opt {
                return tx.execute_unprepared(sql).await.map_err(DbError::Connection);
            }

            let conn = self.connection()?;
            conn.execute_unprepared(sql).await.map_err(DbError::Connection)
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

        // DDL 安全验证（基于 AST 解析，防止注入绕过）
        #[cfg(feature = "sql-parser")]
        {
            let guard = DdlGuard::new();
            match guard.validate(sql) {
                Ok(DdlValidationResult::Allowed) => {
                    // 通过验证，继续执行
                }
                Ok(DdlValidationResult::Forbidden(reason)) => {
                    return Err(DbError::Permission(format!("DDL operation not allowed: {}", reason)));
                }
                Ok(DdlValidationResult::ParseError(error)) => {
                    return Err(DbError::Config(format!("Failed to parse DDL SQL: {}", error)));
                }
                Err(error) => {
                    return Err(DbError::Config(format!("DDL validation error: {}", error)));
                }
            }
        }

        // 执行 SQL
        let conn = self.connection()?;
        conn.execute_unprepared(sql).await.map_err(DbError::Connection)
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
                "execute_duckdb requires the sql-parser feature to be enabled for security checks".to_string(),
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
                            && !self.permission_ctx.check_table_access(&table_name, &action).await
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
                                "SQL statement requires a valid table name for permission checking".to_string(),
                            ));
                        }
                    }
                    Err(_) => {
                        return Err(DbError::Permission(
                            "Failed to parse SQL statement for permission checking".to_string(),
                        ));
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
    pub async fn execute_duckdb_raw(&self, sql: &str) -> DbResult<crate::database::DuckDbExecResult> {
        // 安全检查：与 execute_raw_ddl 对齐 —— admin role 通过 DdlGuard 验证后允许 DDL，
        // 非 admin role 拒绝 DDL。DuckDB 是分析型数据库，admin 需要能创建表/视图，
        // 与 SeaORM 路径的 execute_raw_ddl 行为保持一致。
        #[cfg(feature = "sql-parser")]
        {
            if is_ddl_operation(sql) {
                if self.role == self.pool_inner.admin_role {
                    // admin role 通过 DdlGuard AST 验证后直接执行，不再走 parse_operation 权限检查
                    // （DDL 语句无法被 parse_operation_async 正确解析，会返回 Err）
                    let guard = DdlGuard::new();
                    match guard.validate(sql) {
                        Ok(DdlValidationResult::Allowed) => {
                            let conn = self
                                .connection
                                .as_ref()
                                .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
                            let duck_conn = conn.as_duckdb()?;
                            return duck_conn.execute(sql).await;
                        }
                        Ok(DdlValidationResult::Forbidden(reason)) => {
                            return Err(DbError::Permission(format!("DDL operation not allowed: {}", reason)));
                        }
                        Ok(DdlValidationResult::ParseError(error)) => {
                            return Err(DbError::Config(format!("Failed to parse DDL SQL: {}", error)));
                        }
                        Err(error) => {
                            return Err(DbError::Config(format!("DDL validation error: {}", error)));
                        }
                    }
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
                            && !self.permission_ctx.check_table_access(&table_name, &action).await
                        {
                            return Err(permission_denied(&action, &table_name));
                        }
                    }
                    Ok(None) => {
                        // admin role 对无法解析的语句直接执行（对齐 execute 的 None 路径），
                        // 非 admin role 拒绝（安全默认：无法解析则无法做权限检查）。
                        if self.role != self.pool_inner.admin_role {
                            return Err(DbError::Permission(
                                "SQL statement requires a valid table name for permission checking".to_string(),
                            ));
                        }
                    }
                    Err(_) => {
                        return Err(DbError::Permission(
                            "Failed to parse SQL statement for permission checking".to_string(),
                        ));
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

    /// 执行 Cypher 查询（图数据库专用，ladybug/neo4j feature 启用时可用）
    ///
    /// 按事务状态自动分发：
    /// - 在图事务中：委托给事务句柄执行（确保事务内所有操作使用同一连接）
    /// - 不在事务中：直接在连接上执行
    ///
    /// # 权限检查
    ///
    /// Phase 1 stub：admin 角色绕过所有检查，非 admin 角色被拒绝。
    ///
    /// # 安全性警告（vuln-0005）
    ///
    /// 直接拼接用户输入到 `cypher` 字符串易导致 Cypher 注入。
    /// 此方法仅接受无参数 Cypher，应优先使用
    /// [`execute_cypher_with_params`](Self::execute_cypher_with_params)。
    ///
    /// # 参数
    ///
    /// * `cypher` - Cypher 查询语句
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
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    #[deprecated(
        since = "0.4.2",
        note = "vuln-0005: Cypher injection risk; use execute_cypher_with_params instead"
    )]
    pub async fn execute_cypher(&self, cypher: &str) -> DbResult<crate::database::graph::GraphExecResult> {
        // vuln-0005 修复：注入防护检查（长度限制 + 危险模式检测）
        validate_cypher_safety(cypher)?;

        // 图权限检查（T035 stub：admin 绕过，非 admin deny）
        #[cfg(feature = "permission")]
        {
            let graph_perm_ctx =
                crate::access::permission::GraphPermissionContext::new(&self.role, &self.pool_inner.admin_role);
            graph_perm_ctx.check_graph_access(crate::access::permission::PermissionAction::Traverse)?;
        }

        // 取连接
        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Config("Connection not available - Session may have been invalidated".to_string())
        })?;

        // 获取图操作互斥锁（HIGH-001 修复）
        //
        // 串行化 `execute_cypher` 调用，确保图事务 take → await → put back 期间
        // 无并发调用看到 `graph_transaction` 为 `None` 而绕过事务隔离。
        let _graph_op_guard = self.graph_op_mutex.lock().await;

        // 检查是否在图事务中（短锁 take → 锁外执行 → 短锁 put back）
        let graph_txn = {
            let mut state = self.state.lock().await;
            // FM-3.1 修复：检查 poisoned 标记，防止 panic 后事务隔离被绕过
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
            // FM-3.1 修复：PoisonGuard 确保 panic 时设置 poisoned 标记
            //
            // 如果 `graph_txn.execute_cypher().await` panic，put back 不会执行，
            // 事务句柄丢失。guard 在 unwinding 时设置 poisoned 标记，
            // 后续 execute_cypher 返回错误而非绕过事务隔离。
            struct PoisonGuard<'a> {
                state: &'a Mutex<SessionState>,
                armed: bool,
            }
            impl<'a> Drop for PoisonGuard<'a> {
                fn drop(&mut self) {
                    if self.armed {
                        if let Ok(mut state) = self.state.try_lock() {
                            state.graph_txn_poisoned = true;
                        }
                    }
                }
            }

            let mut guard = PoisonGuard {
                state: &self.state,
                armed: true,
            };
            let result = graph_txn.execute_cypher(cypher).await;
            guard.armed = false; // 正常完成，解除 armed

            // 短锁：put back（无论成功失败都放回，由用户决定 commit/rollback）
            let mut state = self.state.lock().await;
            state.graph_transaction = Some(graph_txn);
            return result;
        }

        // 不在事务中，直接在连接上执行
        let graph = conn.as_graph()?;
        graph.execute_cypher(cypher).await
    }

    /// 执行参数化 Cypher 查询（vuln-0005 修复）
    ///
    /// 与 [`execute_cypher`](Self::execute_cypher) 相同的事务分发逻辑，
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
        // vuln-0005 修复：注入防护检查（长度限制 + 危险模式检测）
        validate_cypher_safety(cypher)?;

        // 图权限检查（与 execute_cypher 相同）
        #[cfg(feature = "permission")]
        {
            let graph_perm_ctx =
                crate::access::permission::GraphPermissionContext::new(&self.role, &self.pool_inner.admin_role);
            graph_perm_ctx.check_graph_access(crate::access::permission::PermissionAction::Traverse)?;
        }

        // 取连接
        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Config("Connection not available - Session may have been invalidated".to_string())
        })?;

        // 获取图操作互斥锁（与 execute_cypher 相同逻辑）
        let _graph_op_guard = self.graph_op_mutex.lock().await;

        // 检查是否在图事务中（短锁 take → 锁外执行 → 短锁 put back）
        let graph_txn = {
            let mut state = self.state.lock().await;
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
            // PoisonGuard（同 execute_cypher）
            struct PoisonGuard<'a> {
                state: &'a Mutex<SessionState>,
                armed: bool,
            }
            impl<'a> Drop for PoisonGuard<'a> {
                fn drop(&mut self) {
                    if self.armed {
                        if let Ok(mut state) = self.state.try_lock() {
                            state.graph_txn_poisoned = true;
                        }
                    }
                }
            }

            let mut guard = PoisonGuard {
                state: &self.state,
                armed: true,
            };
            let result = graph_txn.execute_cypher_with_params(cypher, params).await;
            guard.armed = false;

            let mut state = self.state.lock().await;
            state.graph_transaction = Some(graph_txn);
            return result;
        }

        // 不在事务中，直接在连接上执行参数化查询
        let graph = conn.as_graph()?;
        graph.execute_cypher_with_params(cypher, params).await
    }

    /// 执行 SQL（带权限检查和操作类型）
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self), fields(db.role = %self.role)))]
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
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, operation), fields(db.role = %self.role)))]
    pub async fn execute_with_operation(&self, sql: &str, operation: &PermissionAction) -> DbResult<ExecResult> {
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

        // 提取表名（vuln-0003 修复：使用 SqlParser 替代朴素字符串匹配）
        //
        // 旧实现 `extract_table_name(sql)` 使用 `contains("FROM ")` 朴素字符串匹配，
        // 可被 SQL 字符串字面量、注释、子查询混淆绕过权限检查。
        // 新实现 `extract_table_name_via_parser(sql)` 使用 sqlparser AST 解析，
        // 正确处理字符串字面量、注释、子查询等复杂 SQL 语法。
        //
        // 当 SqlParser 无法提取表名（None）时，跳过表级权限检查，
        // 由下游 `execute_raw` 的 SqlParser 检查提供防御纵深。
        #[cfg(feature = "sql-parser")]
        let table_name: String = extract_table_name_via_parser(sql).await.unwrap_or_default();
        #[cfg(not(feature = "sql-parser"))]
        let table_name: String = extract_table_name(sql);

        // 检查权限
        #[cfg(feature = "permission")]
        {
            if !table_name.is_empty() && !self.permission_ctx.check_table_access(&table_name, operation).await {
                return Err(permission_denied(operation, &table_name));
            }
        }

        // 执行 SQL
        let result = self.execute_raw(sql).await?;

        // 记录指标
        let duration = start.elapsed();
        self.record_query_metrics(&format!("{:?}", operation), duration, true);

        // 如果是写操作，标记
        #[cfg(feature = "permission")]
        {
            if is_write_action(operation) {
                self.mark_write().await;
            }
        }

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

        let mut results = Vec::new();
        let mut last_error = None;

        for sql in sqls {
            match self.execute_raw(sql).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    last_error = Some(e);
                    break;
                }
            }
        }

        if let Some(error) = last_error {
            self.rollback().await?;
            Err(error)
        } else {
            self.commit().await?;
            Ok(results)
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
    pub async fn check_table_permission(&self, _table_name: &str, _operation: &str) -> DbResult<()> {
        #[cfg(feature = "permission")]
        {
            let action = match _operation {
                "INSERT" => PermissionAction::Insert,
                "SELECT" => PermissionAction::Select,
                "UPDATE" => PermissionAction::Update,
                "DELETE" => PermissionAction::Delete,
                _ => return Err(DbError::Permission(format!("Unknown operation: {}", _operation))),
            };

            // Admin 角色绕过权限检查
            // vuln-0001 修复：admin bypass 仍记录审计日志
            if self.role == self.pool_inner.admin_role {
                audit_admin_bypass(&self.role, _table_name, &action);
            } else if !self.permission_ctx.check_table_access(_table_name, &action).await {
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
            metrics.record_query(operation, std::time::Duration::from_millis(0), success, bytes);
        }
    }
}

#[cfg(feature = "permission")]
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
fn permission_denied(action: &(impl std::fmt::Display + ?Sized), table: &(impl std::fmt::Display + ?Sized)) -> DbError {
    DbError::Permission(format!("Permission denied for {} on {}", action, table))
}

/// 记录 admin 权限绕过审计日志（vuln-0001 修复）
///
/// admin 角色绕过权限检查时调用此函数，记录审计日志以保留审计链。
/// 日志输出到 stderr（始终）和 tracing（当 tracing feature 启用时）。
///
/// # 参数
///
/// * `role` - 当前角色名称
/// * `table` - 被访问的表名
/// * `operation` - 权限操作类型
#[cfg(feature = "permission")]
fn audit_admin_bypass(role: &str, table: &str, operation: &PermissionAction) {
    let msg = format!(
        "[SECURITY AUDIT] Admin role '{}' bypassed permission check: operation={:?} table={}",
        role, operation, table
    );
    eprintln!("{}", msg);
    #[cfg(feature = "tracing")]
    {
        tracing::warn!(
            role = role,
            table = table,
            operation = ?operation,
            "Admin role bypassed permission check (vuln-0001 audit)"
        );
    }
}

/// 检查是否使用了默认 admin 角色并发出警告（vuln-0001 修复）
///
/// 当 `admin_role` 为 "admin"（默认值）时，记录安全警告。
/// 返回 `true` 表示使用了默认值（不安全），`false` 表示已自定义。
///
/// # 参数
///
/// * `admin_role` - 当前配置的 admin 角色名称
///
/// # 返回
///
/// `true` 表示使用了默认 "admin" 角色（不安全），`false` 表示已自定义
pub fn warn_if_default_admin_role_used(admin_role: &str) -> bool {
    if admin_role == "admin" {
        let msg = "[SECURITY WARNING] Using default admin_role 'admin' is insecure. \
                   Set a custom admin_role via DbConfig.admin_role or DbPoolBuilder::admin_role().";
        eprintln!("{}", msg);
        #[cfg(feature = "tracing")]
        {
            tracing::warn!(
                admin_role = admin_role,
                "Using default admin_role 'admin' is insecure (vuln-0001)"
            );
        }
        true
    } else {
        false
    }
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
            "Cypher query contains multiple statements (';' inside query) - potential injection".to_string(),
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

    // 4. 块注释检测：`/* */`
    if cypher.contains("/*") || cypher.contains("*/") {
        return Err(DbError::Permission(
            "Cypher query contains block comment '/* */' - potential injection".to_string(),
        ));
    }

    // 5. 危险过程调用检测：`CALL apoc.`（APOC 是 Neo4j 管理员过程库，可执行系统操作）
    //
    // 其他危险过程（如 `dbms.`、`db.`）也在黑名单中，防止提权 / 系统访问。
    let cypher_lower = cypher.to_ascii_lowercase();
    const DANGEROUS_CALLS: &[&str] = &["call apoc.", "call dbms.", "call db.", "call tx."];
    for &dangerous in DANGEROUS_CALLS {
        if cypher_lower.contains(dangerous) {
            return Err(DbError::Permission(format!(
                "Cypher query calls dangerous procedure ('{}') - potential privilege escalation",
                dangerous
            )));
        }
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
        // 归还连接到池
        if let Some(conn) = self.connection.take() {
            self.pool.release_connection(conn);
        }
    }
}

/// 简化的表名提取（用于权限检查）
///
/// # 弃用警告（vuln-0003 修复）
///
/// 此函数使用 `contains("FROM ")` 等朴素字符串匹配提取表名，
/// 可被以下 SQL 混淆绕过权限检查：
///
/// 1. **字符串字面量包含 "FROM "**：`SELECT 'from the depths' FROM users`
///    朴素解析器返回 "the" 而非 "users"。
/// 2. **SQL 注释包含 "FROM "**：`SELECT /* FROM fake_table */ * FROM users`
///    朴素解析器返回 "fake_table" 而非 "users"。
/// 3. **子查询的首个 FROM 在内层**：`SELECT * FROM (SELECT * FROM inner) AS sub`
///    朴素解析器返回 "(SELECT" 而非正确表名。
///
/// # 替代方案
///
/// 使用 [`extract_table_name_via_parser`] 替代，后者基于 sqlparser AST 解析，
/// 正确处理字符串字面量、注释、子查询等复杂 SQL 语法。
///
/// 当 `sql-parser` feature 启用时（`permission` feature 强制启用），
/// [`Session::execute_with_operation`] 已改用 [`extract_table_name_via_parser`]。
///
/// 此函数保留用于 `permission` feature 未启用 `sql-parser` 的边缘情况
/// （实际上 Cargo.toml 中 `permission = ["sql-parser", ...]` 已强制此依赖）。
#[deprecated(
    since = "0.4.2",
    note = "vuln-0003: 朴素字符串匹配可被 SQL 字符串字面量/注释/子查询绕过，请使用 `extract_table_name_via_parser` 替代"
)]
#[cfg(feature = "permission")]
#[allow(dead_code)] // 当 sql-parser 启用时（permission 强制启用），此函数被 extract_table_name_via_parser 替代
fn extract_table_name(sql: &str) -> String {
    // 这是一个简化的实现，实际应该使用 sqlparser
    let sql_upper = sql.to_uppercase();

    if sql_upper.contains("FROM ") {
        if let Some(start) = sql_upper.find("FROM ") {
            let rest = &sql[start + 5..];
            if let Some(end) = rest.find(|c| [' ', ',', ';', '(', ')'].contains(&c)) {
                return rest[..end].trim().to_string();
            } else {
                return rest.trim().to_string();
            }
        }
    }

    if sql_upper.contains("INTO ") {
        if let Some(start) = sql_upper.find("INTO ") {
            let rest = &sql[start + 5..];
            if let Some(end) = rest.find(|c| [' ', '(', ';'].contains(&c)) {
                return rest[..end].trim().to_string();
            } else {
                return rest.trim().to_string();
            }
        }
    }

    if sql_upper.contains("UPDATE ") {
        if let Some(start) = sql_upper.find("UPDATE ") {
            let rest = &sql[start + 7..];
            if let Some(end) = rest.find(|c| [' ', ';'].contains(&c)) {
                return rest[..end].trim().to_string();
            } else {
                return rest.trim().to_string();
            }
        }
    }

    String::new()
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

#[cfg(all(feature = "permission", not(feature = "sql-parser")))]
#[allow(deprecated)]
fn parse_table_and_action(sql: &str) -> (String, PermissionAction) {
    // 此函数仅在 permission 启用但 sql-parser 未启用时使用
    // （实际上 Cargo.toml 中 permission 强制依赖 sql-parser，此分支为死代码）
    // 当 sql-parser 不可用时，只能使用已弃用的 extract_table_name 作为 fallback
    let table_name = extract_table_name(sql);
    let sql_upper = sql.trim_start().to_uppercase();
    let action = if sql_upper.starts_with("INSERT") {
        PermissionAction::Insert
    } else if sql_upper.starts_with("UPDATE") {
        PermissionAction::Update
    } else if sql_upper.starts_with("DELETE") {
        PermissionAction::Delete
    } else {
        PermissionAction::Select
    };

    (table_name, action)
}

/// 解析 SQL 操作类型和表名用于权限检查
///
/// 统一 execute 流程中的 SQL 解析入口，消除 permission+sql-parser 与
/// permission+无 sql-parser 两个 cfg 分支的重复结构：
/// - sql-parser 启用：返回 None 表示不支持的语句或解析失败（execute 会跳过权限检查直接执行）
/// - sql-parser 未启用：始终返回 Some（使用简化解析器 parse_table_and_action）
#[cfg(feature = "permission")]
async fn parse_sql_for_permission(sql: &str) -> DbResult<Option<(String, PermissionAction)>> {
    #[cfg(feature = "sql-parser")]
    {
        let parser = SqlParser::shared().await;
        match parser.parse_operation_async(sql).await {
            Ok(Some((table, action))) => Ok(Some((table, action))),
            Ok(None) => Ok(None),
            Err(_) => Ok(None),
        }
    }
    #[cfg(not(feature = "sql-parser"))]
    {
        Ok(Some(parse_table_and_action(sql)))
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
#[allow(deprecated)] // vuln-0005: Session::execute_cypher 已 deprecated，但 graph_tests 仍需验证旧 API 行为
mod graph_tests {
    use super::*;
    use crate::database::graph::{GraphExecResult, GraphValue};

    /// 创建 Ladybug 内存连接池
    async fn make_ladybug_pool() -> DbPool {
        DbPool::new("ladybug::memory:")
            .await
            .expect("Failed to create Ladybug pool")
    }

    // ===== T032: is_in_transaction 图事务支持 =====

    /// TEST-GRAPH-TXN-001: 图连接初始 is_in_transaction 为 false
    #[tokio::test]
    async fn test_graph_session_is_in_transaction_initial_false() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        assert!(
            !session.is_in_transaction().await,
            "initial state should be no transaction"
        );
    }

    /// TEST-GRAPH-TXN-002: begin_transaction 后 is_in_transaction 为 true
    #[tokio::test]
    async fn test_graph_session_begin_sets_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        session.begin_transaction().await.expect("begin should succeed");
        assert!(
            session.is_in_transaction().await,
            "should be in transaction after begin"
        );
    }

    /// TEST-GRAPH-TXN-003: begin + commit 后 is_in_transaction 为 false
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

    /// TEST-GRAPH-TXN-004: begin + rollback 后 is_in_transaction 为 false
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

    // ===== T033: begin/commit/rollback 图事务分发 =====

    /// TEST-GRAPH-TXN-005: 图事务 begin → execute_cypher → commit 端到端
    #[tokio::test]
    async fn test_graph_transaction_commit_e2e() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 准备：创建 schema
        session
            .execute_cypher("CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))")
            .await
            .expect("create node table");

        // 事务：插入数据并提交
        session.begin_transaction().await.expect("begin");
        session
            .execute_cypher("CREATE (:Person {name: 'Alice'})")
            .await
            .expect("create in txn");
        session.commit().await.expect("commit");

        // 验证：提交后数据可见
        let result = session
            .execute_cypher("MATCH (p:Person) RETURN p.name AS name")
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

    /// TEST-GRAPH-TXN-006: 图事务 begin → execute_cypher → rollback 端到端
    #[tokio::test]
    async fn test_graph_transaction_rollback_e2e() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // 准备：创建 schema
        session
            .execute_cypher("CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))")
            .await
            .expect("create node table");

        // 事务：插入数据并回滚
        session.begin_transaction().await.expect("begin");
        session
            .execute_cypher("CREATE (:Person {name: 'Bob'})")
            .await
            .expect("create in txn");
        session.rollback().await.expect("rollback");

        // 验证：回滚后数据不可见
        let result = session
            .execute_cypher("MATCH (p:Person) RETURN p.name AS name")
            .await
            .expect("match after rollback");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 0, "should see 0 persons after rollback");
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    /// TEST-GRAPH-TXN-007: 重复 begin 应返回 Transaction 错误
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

    /// TEST-GRAPH-TXN-008: 无事务时 commit 应返回错误
    #[tokio::test]
    async fn test_graph_commit_without_transaction_fails() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session.commit().await;
        assert!(result.is_err(), "commit without transaction should fail");
    }

    /// TEST-GRAPH-TXN-009: 无事务时 rollback 应返回错误
    #[tokio::test]
    async fn test_graph_rollback_without_transaction_fails() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session.rollback().await;
        assert!(result.is_err(), "rollback without transaction should fail");
    }

    // ===== T034: execute_cypher 测试 =====

    /// TEST-GRAPH-EXEC-001: 不在事务中 execute_cypher("RETURN 1") 返回结果
    #[tokio::test]
    async fn test_execute_cypher_without_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session
            .execute_cypher("RETURN 1")
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

    /// TEST-GRAPH-EXEC-002: 在事务中 execute_cypher 委托给事务句柄
    #[tokio::test]
    async fn test_execute_cypher_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        session
            .execute_cypher("CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))")
            .await
            .expect("create table");

        session.begin_transaction().await.expect("begin");
        session
            .execute_cypher("CREATE (:Person {name: 'Alice', age: 25})")
            .await
            .expect("create in txn");

        // 事务内查询应看到数据
        let result = session
            .execute_cypher("MATCH (p:Person) RETURN p.name AS name, p.age AS age")
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

    /// TEST-GRAPH-EXEC-003: CREATE NODE TABLE + CREATE + MATCH 端到端
    #[tokio::test]
    async fn test_execute_cypher_e2e_create_match() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        // DDL
        session
            .execute_cypher("CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))")
            .await
            .expect("create node table");

        // 插入多条
        session
            .execute_cypher("CREATE (:Person {name: 'Alice', age: 25})")
            .await
            .expect("create alice");
        session
            .execute_cypher("CREATE (:Person {name: 'Bob', age: 30})")
            .await
            .expect("create bob");

        // 查询并验证
        let result = session
            .execute_cypher("MATCH (p:Person) RETURN p.name AS name, p.age AS age ORDER BY name")
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

    /// TEST-GRAPH-EXEC-004: 无效 Cypher 返回错误
    #[tokio::test]
    async fn test_execute_cypher_invalid_returns_error() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session.execute_cypher("INVALID CYPHER").await;
        assert!(result.is_err(), "invalid cypher should return error");
    }

    /// TEST-GRAPH-EXEC-005: 事务内多次 execute_cypher 使用同一事务句柄
    #[tokio::test]
    async fn test_execute_cypher_multiple_in_transaction() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");

        session
            .execute_cypher("CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))")
            .await
            .expect("create table");

        session.begin_transaction().await.expect("begin");

        // 多次 execute_cypher 都应在同一事务内
        session
            .execute_cypher("CREATE (:Person {name: 'A'})")
            .await
            .expect("create A");
        session
            .execute_cypher("CREATE (:Person {name: 'B'})")
            .await
            .expect("create B");
        session
            .execute_cypher("CREATE (:Person {name: 'C'})")
            .await
            .expect("create C");

        let result = session
            .execute_cypher("MATCH (p:Person) RETURN count(p) AS cnt")
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

    /// TEST-GRAPH-EXEC-006: 非 admin 角色调用 execute_cypher 应被拒绝（permission feature）
    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_execute_cypher_non_admin_denied() {
        let pool = make_ladybug_pool().await;
        // system 角色在无权限配置时也被允许获取 session
        let session = pool.get_session("system").await.expect("get_session");
        let result = session.execute_cypher("RETURN 1").await;
        assert!(result.is_err(), "non-admin role should be denied");
        let err = result.unwrap_err();
        assert!(
            matches!(err, DbError::Permission(ref msg) if msg.contains("Graph operation denied")),
            "expected Permission error, got {:?}",
            err
        );
    }

    /// TEST-GRAPH-EXEC-007: admin 角色 execute_cypher 成功（permission feature）
    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_execute_cypher_admin_allowed() {
        let pool = make_ladybug_pool().await;
        let session = pool.get_session("admin").await.expect("get_session");
        let result = session.execute_cypher("RETURN 42").await;
        assert!(result.is_ok(), "admin role should be allowed");
    }
}

// ============================================================================
// vuln-0001 安全审计测试
// ============================================================================

#[cfg(test)]
mod vuln_0001_tests {
    use super::*;

    /// vuln-0001 回归测试：warn_if_default_admin_role_used 对默认 "admin" 返回 true
    #[test]
    fn test_vuln_0001_warn_default_admin_role() {
        assert!(
            warn_if_default_admin_role_used("admin"),
            "default admin_role 'admin' should trigger warning"
        );
    }

    /// vuln-0001 回归测试：warn_if_default_admin_role_used 对自定义角色返回 false
    #[test]
    fn test_vuln_0001_custom_admin_role_no_warning() {
        assert!(
            !warn_if_default_admin_role_used("super-admin-2026"),
            "custom admin_role should not trigger warning"
        );
    }

    /// vuln-0001 回归测试：admin bypass 审计日志不 panic
    #[cfg(feature = "permission")]
    #[test]
    fn test_vuln_0001_audit_admin_bypass_no_panic() {
        // audit_admin_bypass 应该正常执行而不 panic
        // 它输出到 stderr，我们只验证不 panic
        audit_admin_bypass("admin", "users", &PermissionAction::Select);
        audit_admin_bypass("admin", "users", &PermissionAction::Insert);
        audit_admin_bypass("admin", "orders", &PermissionAction::Delete);
    }

    /// vuln-0001 集成测试：admin 角色绕过权限检查仍返回 Ok（带审计日志）
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_vuln_0001_admin_bypass_returns_ok_with_audit() {
        let pool = DbPool::new("sqlite::memory:").await.expect("Failed to create pool");
        let session = pool.get_session("admin").await.expect("get_session");

        // admin 角色绕过权限检查，应返回 Ok
        let result = session.check_permission("any_table", &PermissionAction::Select).await;
        assert!(result.is_ok(), "admin bypass should return Ok");

        // 也测试其他操作
        let result = session.check_permission("any_table", &PermissionAction::Insert).await;
        assert!(result.is_ok(), "admin bypass should return Ok for Insert");

        let result = session.check_permission("any_table", &PermissionAction::Delete).await;
        assert!(result.is_ok(), "admin bypass should return Ok for Delete");
    }

    /// vuln-0001 集成测试：非 admin 角色权限被拒绝
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_vuln_0001_non_admin_denied() {
        let pool = DbPool::new("sqlite::memory:").await.expect("Failed to create pool");
        // system 角色可获取 session 但不是 admin_role，无权限配置时 check_permission 应拒绝
        let session = pool.get_session("system").await.expect("get_session");

        // 非 admin 角色应被拒绝（无权限配置时默认拒绝）
        let result = session.check_permission("any_table", &PermissionAction::Select).await;
        assert!(result.is_err(), "non-admin should be denied");
    }

    /// vuln-0001 集成测试：check_table_permission admin bypass 带审计日志
    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_vuln_0001_check_table_permission_admin_bypass() {
        let pool = DbPool::new("sqlite::memory:").await.expect("Failed to create pool");
        let session = pool.get_session("admin").await.expect("get_session");

        // admin bypass check_table_permission
        let result = session.check_table_permission("users", "SELECT").await;
        assert!(result.is_ok(), "admin should bypass check_table_permission");

        let result = session.check_table_permission("users", "INSERT").await;
        assert!(result.is_ok(), "admin should bypass check_table_permission for INSERT");
    }
}

// ============================================================================
// vuln-0003 测试：extract_table_name 朴素字符串匹配绕过
// ============================================================================
//
// 漏洞描述：
//   `extract_table_name` 使用 `contains("FROM ")` 等朴素字符串匹配提取表名，
//   可被以下 SQL 混淆绕过权限检查：
//   1. 字符串字面量包含 "FROM " → 提取错误的表名
//   2. SQL 注释包含 "FROM " → 提取错误的表名
//   3. 子查询的首个 FROM 在内层 → 提取错误的表名
//
// 修复方案：
//   使用 SqlParser（基于 sqlparser AST 解析）替代朴素字符串匹配，
//   标记 `extract_table_name` 为 `#[deprecated]`。
// ============================================================================

#[cfg(test)]
#[cfg(all(feature = "permission", feature = "sql-parser"))]
#[allow(deprecated)]
mod vuln_0003_tests {
    use super::*;

    /// 辅助：通过 SqlParser 提取表名（修复后由 `extract_table_name_via_parser` 提供）
    ///
    /// 此函数在测试模块内独立实现，避免依赖尚未添加的内部函数。
    /// 修复后由 `extract_table_name_via_parser` 替代此测试辅助。
    async fn extract_table_name_via_parser_for_test(sql: &str) -> Option<String> {
        let parser = SqlParser::shared().await;
        parser
            .parse_operation_async(sql)
            .await
            .ok()
            .flatten()
            .map(|(table, _)| table)
    }

    /// vuln-0003 Red-1：朴素 `extract_table_name` 对字符串字面量内的 "FROM " 误匹配
    ///
    /// SQL: `SELECT 'from the depths' FROM users`
    /// 朴素解析器返回 "the"（来自字符串字面量 "from the depths"），
    /// 而 SqlParser 正确返回 "users"。
    #[tokio::test]
    async fn test_vuln_0003_naive_fails_on_string_literal_containing_from() {
        let sql = "SELECT 'from the depths' FROM users";

        // 朴素解析器返回错误结果（漏洞证据）
        let naive_result = extract_table_name(sql);
        assert_ne!(
            naive_result, "users",
            "naive extract_table_name should NOT return 'users' (demonstrating the bug)"
        );

        // SqlParser 正确提取表名
        let parser_result = extract_table_name_via_parser_for_test(sql).await;
        assert_eq!(
            parser_result.as_deref(),
            Some("users"),
            "SqlParser should correctly extract 'users' table name"
        );
    }

    /// vuln-0003 Red-2：朴素 `extract_table_name` 对 SQL 注释内的 "FROM " 误匹配
    ///
    /// SQL: `SELECT /* FROM fake_table */ * FROM users`
    /// 朴素解析器返回 "fake_table"（来自注释），权限检查针对错误表名。
    ///
    /// SqlParser 行为：
    /// - 将 `/* ... */` 块注释视为潜在注入向量并拒绝（安全行为）
    /// - 或正确提取 "users"（若注释被正常处理）
    /// 两种行为都是安全的，关键是不会返回错误表名让 SQL 绕过权限检查。
    #[tokio::test]
    async fn test_vuln_0003_naive_fails_on_comment_containing_from() {
        let sql = "SELECT /* FROM fake_table */ * FROM users";

        // 朴素解析器返回错误结果（漏洞证据）
        let naive_result = extract_table_name(sql);
        assert_ne!(
            naive_result, "users",
            "naive extract_table_name should NOT return 'users' when comment contains FROM (demonstrating the bug)"
        );

        // SqlParser 行为：拒绝 SQL（返回 None）或正确提取表名
        // 两种都是安全行为 — 关键是不会返回错误表名绕过权限检查
        let parser_result = extract_table_name_via_parser_for_test(sql).await;
        match parser_result {
            None => {
                // SqlParser 拒绝 SQL（检测到注释注入模式）— 安全行为
            }
            Some(table) => {
                assert_eq!(
                    table, "users",
                    "SqlParser should either reject or return correct table name 'users', got: {}",
                    table
                );
            }
        }
    }

    /// vuln-0003 Red-3：朴素 `extract_table_name` 对子查询的首个 FROM 误匹配
    ///
    /// SQL: `SELECT * FROM (SELECT * FROM inner_table) AS sub`
    /// 朴素解析器返回 "(SELECT"（来自子查询的 FROM），
    /// 而 SqlParser 正确返回 "inner_table"（最外层 FROM 的表名）。
    ///
    /// 注意：对于派生表（subquery in FROM），SqlParser 返回 None
    /// 因为派生表没有具名基表，朴素解析器返回无意义的 "(SELECT" 是错误的。
    #[tokio::test]
    async fn test_vuln_0003_naive_fails_on_subquery_from() {
        let sql = "SELECT * FROM (SELECT * FROM inner_table) AS sub";

        // 朴素解析器返回错误结果（漏洞证据）
        let naive_result = extract_table_name(sql);
        // 朴素解析器会返回 "(SELECT" 之类的无意义字符串
        assert_ne!(
            naive_result, "inner_table",
            "naive extract_table_name should NOT return 'inner_table' for subquery (demonstrating the bug)"
        );

        // SqlParser 应返回 None（派生表无具名基表）或正确表名，
        // 但绝不会返回朴素解析器那样的无意义字符串
        let parser_result = extract_table_name_via_parser_for_test(sql).await;
        // SqlParser 对派生表返回 None（无具名基表）
        // 这是正确行为：派生表的权限检查应在外层 SQL 上下文处理
        assert!(
            parser_result.is_none() || parser_result.as_deref() == Some("inner_table"),
            "SqlParser should return None or correct table name for derived table, got: {:?}",
            parser_result
        );
    }

    /// vuln-0003 Red-4：朴素 `extract_table_name` 对 INSERT INTO 字符串字面量误匹配
    ///
    /// SQL: `INSERT INTO users (name) VALUES ('from into values')`
    /// 朴素解析器应正确提取 "users"，但类似情况在其他 SQL 类型中可能出错。
    /// 此测试验证 SqlParser 对 INSERT 的正确处理。
    #[tokio::test]
    async fn test_vuln_0003_parser_correctly_handles_insert() {
        let sql = "INSERT INTO users (name) VALUES ('from into values')";

        // SqlParser 正确提取表名
        let parser_result = extract_table_name_via_parser_for_test(sql).await;
        assert_eq!(
            parser_result.as_deref(),
            Some("users"),
            "SqlParser should correctly extract 'users' for INSERT"
        );
    }

    /// vuln-0003 Red-5：朴素 `extract_table_name` 对 UPDATE 字符串字面量误匹配
    ///
    /// SQL: `UPDATE users SET name = 'from users' WHERE id = 1`
    /// 朴素解析器对 UPDATE 路径使用 `contains("UPDATE ")`，
    /// 此测试验证 SqlParser 对 UPDATE 的正确处理。
    #[tokio::test]
    async fn test_vuln_0003_parser_correctly_handles_update() {
        let sql = "UPDATE users SET name = 'from users' WHERE id = 1";

        // SqlParser 正确提取表名
        let parser_result = extract_table_name_via_parser_for_test(sql).await;
        assert_eq!(
            parser_result.as_deref(),
            Some("users"),
            "SqlParser should correctly extract 'users' for UPDATE"
        );
    }

    /// vuln-0003 Red-6：朴素 `extract_table_name` 对 DELETE 字符串字面量误匹配
    ///
    /// SQL: `DELETE FROM users WHERE name = 'from deleted'`
    /// 此测试验证 SqlParser 对 DELETE 的正确处理。
    #[tokio::test]
    async fn test_vuln_0003_parser_correctly_handles_delete() {
        let sql = "DELETE FROM users WHERE name = 'from deleted'";

        // SqlParser 正确提取表名
        let parser_result = extract_table_name_via_parser_for_test(sql).await;
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
        let parser_result = extract_table_name_via_parser_for_test(sql).await;
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
        assert!(result.is_err(), "Cypher with line comment '//' should be rejected");

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
        assert!(result.is_err(), "Cypher with block comment '/* */' should be rejected");

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
        assert!(result.is_err(), "Cypher calling APOC procedure should be rejected");

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
        session.begin_transaction().await.expect("begin transaction");

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
            .execute_cypher_with_params("MATCH (a:Account) RETURN a.id AS id ORDER BY a.id", HashMap::new())
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
