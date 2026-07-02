// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Session 模块
//!
//! 提供数据库会话管理，包括事务、权限检查和读写分离

use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "permission")]
use crate::access::permission::{PermissionAction, PermissionContext};
#[cfg(feature = "sql-parser")]
use crate::access::security::{DdlGuard, DdlValidationResult};
#[cfg(all(feature = "sql-parser", feature = "permission"))]
use crate::access::sql_parser::SqlParser;
#[cfg(feature = "sql-parser")]
use crate::access::sql_parser::is_ddl_operation;
use crate::database::pool::db_pool::{DatabaseConnection, DbConnection, DbPool, DbPoolInner};
use crate::foundation::error::{DbError, DbResult};
#[cfg(feature = "metrics")]
use crate::observability::metrics::MetricsCollector;
use async_trait::async_trait;

// 编译时检查：必须启用 permission 或 sql-parser feature 之一
#[cfg(not(any(feature = "permission", feature = "sql-parser")))]
compile_error!("Either 'permission' or 'sql-parser' feature must be enabled for Session functionality");

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
                last_write: None,
            }),
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
        if self.role == self.pool_inner.admin_role {
            return Ok(());
        }

        if self.permission_ctx.check_table_access(table, operation).await {
            Ok(())
        } else {
            Err(permission_denied(operation, table))
        }
    }

    /// 是否在事务中
    pub async fn is_in_transaction(&self) -> bool {
        let state = self.state.lock().await;
        state.transaction.is_some()
    }

    /// 开始事务
    ///
    /// v0.3.0 性能优化：短锁模式，避免持锁期间 async DB 调用。
    /// 流程：短锁检查 → 锁外 begin → 短锁写入（含并发冲突处理）
    pub async fn begin_transaction(&self) -> Result<(), DbError> {
        // 短锁：检查是否已在事务中
        {
            let state = self.state.lock().await;
            if state.transaction.is_some() {
                return Err(DbError::Transaction("Already in transaction".to_string()));
            }
        }

        // 锁外：执行 async DB 操作（避免持锁 await 阻塞其他调用者）
        let conn = self.connection()?;
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
    /// # 并发安全
    ///
    /// 如果在 commit 时有其他查询正在执行（持有 transaction 的 Arc clone），
    /// `Arc::try_unwrap` 会失败并返回错误。这是预期行为：用户不应在查询执行中提交事务。
    pub async fn commit(&self) -> Result<(), DbError> {
        // 短锁：take transaction
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
    /// # 并发安全
    ///
    /// 如果在 rollback 时有其他查询正在执行（持有 transaction 的 Arc clone），
    /// `Arc::try_unwrap` 会失败并返回错误。这是预期行为：用户不应在查询执行中回滚事务。
    pub async fn rollback(&self) -> Result<(), DbError> {
        // 短锁：take transaction
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
        db_type: crate::foundation::config::DatabaseType,
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
            return Err(DbError::Permission(
                "execute_raw requires the sql-parser feature to be enabled".to_string(),
            ));
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
    pub async fn execute_duckdb(&self, sql: &str) -> DbResult<Vec<crate::database::pool::DuckDbRow>> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
        let duck_conn = conn.as_duckdb()?;
        duck_conn.query(sql).await
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
    pub async fn execute_duckdb_raw(&self, sql: &str) -> DbResult<crate::database::pool::DuckDbExecResult> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| DbError::Config("Connection not available".to_string()))?;
        let duck_conn = conn.as_duckdb()?;
        duck_conn.execute(sql).await
    }

    /// 执行 SQL（带权限检查和操作类型）
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self), fields(db.role = %self.role)))]
    pub async fn execute(&self, sql: &str) -> DbResult<ExecResult> {
        let start = Instant::now();

        // DDL 检查（sql-parser 启用时）
        #[cfg(feature = "sql-parser")]
        check_ddl_operation(sql)?;

        #[cfg(feature = "permission")]
        {
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

        // 提取表名
        let table_name = extract_table_name(sql);

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
    #[cfg(feature = "metrics")]
    fn record_query_metrics(&self, query_type: &str, duration: Duration, success: bool) {
        if let Some(metrics) = &self.metrics_collector {
            metrics.record_query(query_type, duration, success, None);
        }
    }

    /// 记录查询指标（无 metrics 特性）
    #[cfg(not(feature = "metrics"))]
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
            if self.role == self.pool_inner.admin_role {
                // admin 有完全权限，跳过检查
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

/// 判断是否为写操作（Insert/Update/Delete）
#[cfg(feature = "permission")]
fn is_write_action(action: &PermissionAction) -> bool {
    matches!(
        action,
        PermissionAction::Insert | PermissionAction::Update | PermissionAction::Delete
    )
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
        // 归还连接到池
        if let Some(conn) = self.connection.take() {
            self.pool.release_connection(conn);
        }
    }
}

/// 简化的表名提取（用于权限检查）
#[cfg(feature = "permission")]
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

#[cfg(all(feature = "permission", not(feature = "sql-parser")))]
fn parse_table_and_action(sql: &str) -> (String, PermissionAction) {
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
