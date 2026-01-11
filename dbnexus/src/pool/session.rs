// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Session 模块
//!
//! 提供数据库会话管理，包括事务、权限检查和读写分离

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::time::SystemTime;

use crate::config::{DbError, DbResult};
#[cfg(feature = "metrics")]
use crate::metrics::MetricsCollector;
use crate::permission::{PermissionAction, PermissionContext};
use crate::sql_parser::{SqlParser, is_ddl_operation};
use crate::pool::db_pool::{DbPool, DbPoolInner, DatabaseConnection};

// 导入 Sea-ORM 的事务 trait 和连接 trait
use sea_orm::{ConnectionTrait, DatabaseTransaction, ExecResult, Statement, TransactionTrait};

/// Session 结构
pub struct Session {
    /// 数据库连接
    connection: Option<DatabaseConnection>,

    /// 连接池（用于释放连接）
    pool: Arc<DbPool>,

    /// 连接池内部状态
    pool_inner: Arc<DbPoolInner>,

    /// 角色
    role: String,

    /// 最后写操作时间（用于读写分离）
    last_write: Option<Instant>,

    /// 权限上下文
    permission_ctx: PermissionContext,

    /// 事务对象（用于真实的事务管理）
    transaction: Option<DatabaseTransaction>,

    /// 指标收集器（可选，用于 metrics 特性）
    #[cfg(feature = "metrics")]
    metrics: Option<Arc<MetricsCollector>>,
}

impl Session {
    /// 创建新的 Session
    pub(crate) fn new(
        connection: DatabaseConnection,
        pool: Arc<DbPool>,
        pool_inner: Arc<DbPoolInner>,
        role: String,
    ) -> Self {
        let permission_ctx = PermissionContext::new(role.clone(), pool_inner.policy_cache.clone());

        #[cfg(feature = "metrics")]
        let metrics = pool_inner.metrics.clone();

        Session {
            connection: Some(connection),
            pool,
            pool_inner,
            role,
            last_write: None,
            permission_ctx,
            transaction: None,
            #[cfg(feature = "metrics")]
            metrics,
        }
    }

    /// 获取角色
    pub fn role(&self) -> &str {
        &self.role
    }

    /// 获取权限上下文
    pub fn permission_ctx(&self) -> &PermissionContext {
        &self.permission_ctx
    }

    /// 标记为写操作
    pub fn mark_write(&mut self) {
        self.last_write = Some(Instant::now());
    }

    /// 检查权限
    pub async fn check_permission(&self, table: &str, operation: &PermissionAction) -> Result<(), DbError> {
        if self.permission_ctx.check_table_access(table, operation).await {
            Ok(())
        } else {
            Err(DbError::Permission(format!(
                "Permission denied for {} on {}",
                operation, table
            )))
        }
    }

    /// 是否在事务中
    pub fn is_in_transaction(&self) -> bool {
        self.transaction.is_some()
    }

    /// 开始事务
    pub async fn begin_transaction(&mut self) -> Result<(), DbError> {
        if self.is_in_transaction() {
            return Err(DbError::Transaction("Already in transaction".to_string()));
        }

        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Config("Connection not available".to_string())
        })?;

        let transaction = conn.begin().await.map_err(|e| {
            DbError::Transaction(format!("Failed to begin transaction: {}", e))
        })?;

        self.transaction = Some(transaction);
        Ok(())
    }

    /// 提交事务
    pub async fn commit(&mut self) -> Result<(), DbError> {
        let transaction = self.transaction.take().ok_or_else(|| {
            DbError::Transaction("No active transaction to commit".to_string())
        })?;
        transaction.commit().await.map_err(|e| DbError::Transaction(e.to_string()))?;
        self.last_write = None;
        Ok(())
    }

    /// 回滚事务
    pub async fn rollback(&mut self) -> Result<(), DbError> {
        if !self.is_in_transaction() {
            return Err(DbError::Transaction("Not in transaction".to_string()));
        }

        let transaction = self.transaction.take().unwrap();
        transaction.rollback().await.map_err(|e| {
            DbError::Transaction(format!("Failed to rollback transaction: {}", e))
        })?;

        Ok(())
    }

    /// 是否应该使用主库（基于读写分离配置）
    pub fn should_use_master(&self) -> bool {
        // 如果在事务中，必须使用主库
        if self.is_in_transaction() {
            return true;
        }

        // 如果配置了读写分离且有写操作，使用主库
        self.last_write
            .map(|t| t.elapsed() < Duration::from_secs(5))
            .unwrap_or(false)
    }

    /// 获取连接引用
    pub fn connection(&mut self) -> Result<&mut DatabaseConnection, DbError> {
        self.connection.as_mut().ok_or_else(|| {
            DbError::Config("Connection not available".to_string())
        })
    }

    /// 执行原始 SQL（带权限检查）
    pub async fn execute_raw(&self, sql: &str) -> DbResult<ExecResult> {
        // 检查是否为 DDL 操作
        if is_ddl_operation(sql) {
            return Err(DbError::Permission(
                "DDL operations are not allowed in this context".to_string(),
            ));
        }

        // 解析 SQL 操作类型和表名
        let parser = SqlParser::new();
        if let Some((_, action)) = parser.parse_operation(sql) {
            // 提取表名（简化实现）
            let table_name = extract_table_name(sql);

            // 检查权限
            if !self.permission_ctx.check_table_access(&table_name, &action).await {
                return Err(DbError::Permission(format!(
                    "Permission denied for {} on {}",
                    action, table_name
                )));
            }
        }

        // 执行 SQL
        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Config("Connection not available".to_string())
        })?;

        conn.execute_unprepared(sql).await.map_err(|e| {
            DbError::Connection(e)
        })
    }

    /// 执行 SQL（带权限检查和操作类型）
    pub async fn execute(&mut self, sql: &str) -> DbResult<ExecResult> {
        let start = Instant::now();

        // 检查是否为 DDL 操作
        if is_ddl_operation(sql) {
            return Err(DbError::Permission(
                "DDL operations are not allowed in this context".to_string(),
            ));
        }

        // 解析 SQL 操作类型和表名
        let parser = SqlParser::new();
        let (table_name, action) = parser
            .parse_operation(sql)
            .unwrap_or_else(|| (String::new(), PermissionAction::Select));

        // 检查权限
        if !table_name.is_empty() {
            if !self.permission_ctx.check_table_access(&table_name, &action).await {
                return Err(DbError::Permission(format!(
                    "Permission denied for {} on {}",
                    action, table_name
                )));
            }
        }

        // 执行 SQL
        let result = self.execute_raw(sql).await?;

        // 记录指标
        let duration = start.elapsed();
        self.record_query_metrics(&format!("{:?}", action), duration, true);

        // 如果是写操作，标记
        if matches!(action, PermissionAction::Insert | PermissionAction::Update | PermissionAction::Delete) {
            self.mark_write();
        }

        Ok(result)
    }

    /// 执行 SQL 并指定操作类型
    pub async fn execute_with_operation(
        &mut self,
        sql: &str,
        operation: &PermissionAction,
    ) -> DbResult<ExecResult> {
        let start = Instant::now();

        // 检查是否为 DDL 操作
        if is_ddl_operation(sql) {
            return Err(DbError::Permission(
                "DDL operations are not allowed in this context".to_string(),
            ));
        }

        // 提取表名
        let table_name = extract_table_name(sql);

        // 检查权限
        if !table_name.is_empty() {
            if !self.permission_ctx.check_table_access(&table_name, operation).await {
                return Err(DbError::Permission(format!(
                    "Permission denied for {} on {}",
                    operation, table_name
                )));
            }
        }

        // 执行 SQL
        let result = self.execute_raw(sql).await?;

        // 记录指标
        let duration = start.elapsed();
        self.record_query_metrics(&format!("{:?}", operation), duration, true);

        // 如果是写操作，标记
        if matches!(
            operation,
            PermissionAction::Insert | PermissionAction::Update | PermissionAction::Delete
        ) {
            self.mark_write();
        }

        Ok(result)
    }

    /// 记录查询指标
    #[cfg(feature = "metrics")]
    fn record_query_metrics(&self, query_type: &str, duration: Duration, success: bool) {
        if let Some(metrics) = &self.metrics {
            metrics.record_query(query_type, duration, success);
        }
    }

    /// 记录查询指标（无 metrics 特性）
    #[cfg(not(feature = "metrics"))]
    fn record_query_metrics(&self, _query_type: &str, _duration: Duration, _success: bool) {
        // No-op when metrics feature is disabled
    }

    /// 记录连接错误
    #[cfg(feature = "metrics")]
    fn record_connection_error(&self) {
        if let Some(metrics) = &self.metrics {
            metrics.record_connection_error();
        }
    }

    /// 记录连接错误（无 metrics 特性）
    #[cfg(not(feature = "metrics"))]
    fn record_connection_error(&self) {
        // No-op when metrics feature is disabled
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // 归还连接到池
        if let Some(conn) = self.connection.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                pool.release_connection(conn);
            });
        }
    }
}

/// 简化的表名提取（用于权限检查）
fn extract_table_name(sql: &str) -> String {
    // 这是一个简化的实现，实际应该使用 sqlparser
    let sql_upper = sql.to_uppercase();

    if sql_upper.contains("FROM ") {
        if let Some(start) = sql_upper.find("FROM ") {
            let rest = &sql[start + 5..];
            if let Some(end) = rest.find(|c| c == ' ' || c == ',' || c == ';') {
                return rest[..end].trim().to_string();
            }
        }
    }

    if sql_upper.contains("INTO ") {
        if let Some(start) = sql_upper.find("INTO ") {
            let rest = &sql[start + 5..];
            if let Some(end) = rest.find(|c| c == ' ' || c == '(' || c == ';') {
                return rest[..end].trim().to_string();
            }
        }
    }

    if sql_upper.starts_with("UPDATE ") {
        if let Some(start) = sql_upper.find("UPDATE ") {
            let rest = &sql[start + 7..];
            if let Some(end) = rest.find(|c| c == ' ' || c == ';') {
                return rest[..end].trim().to_string();
            }
        }
    }

    String::new()
}