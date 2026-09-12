// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 连接池管理模块
//!
//! 提供数据库连接池的创建、管理和自动修正功能

// audit 公开：admin bypass 审计环的观测 API（admin_bypass_count /
// take_admin_bypass_events）需供外部 crate 观测，模块必须公开可达
pub mod audit;
mod db_pool;
#[cfg(feature = "health-check")]
pub mod health_export;
mod pool_impl;
#[cfg(feature = "prepare-cache")]
pub mod prepare_cache;
mod session;

#[cfg(feature = "duckdb")]
pub mod duckdb_conn;

use crate::foundation::DbConfig;

pub use db_pool::{DatabaseConnection, DbConnection, DbPool, PoolStatus};
pub use session::Session;

#[cfg(feature = "duckdb")]
pub use duckdb_conn::{DuckDbConnection, DuckDbExecResult, DuckDbRow, DuckValue};
// 语句级 prepared statement LRU 缓存（池级就绪标记 + 命中率指标）
#[cfg(feature = "prepare-cache")]
pub use prepare_cache::{PoolPrepareCache, PrepareCacheStats, PreparedStatementCache};

// 导出迁移执行器供内部使用
#[cfg(feature = "migration")]
pub(crate) use crate::database::MigrationExecutor;

// 导入 Sea-ORM 的事务 trait 和连接 trait
pub use sea_orm::{ConnectionTrait, TransactionTrait};

use crate::foundation::DbResult;
use async_trait::async_trait;
use sea_orm::ExecResult;

#[cfg(any(
    feature = "metrics",
    feature = "cache",
    feature = "oxcache-integration"
))]
use std::sync::Arc;

#[cfg(any(feature = "cache", feature = "oxcache-integration"))]
use crate::domain::DbCacheProvider;

/// 连接池抽象 trait
///
/// 定义连接池的通用接口，便于测试和替换实现
#[async_trait]
pub trait ConnectionPool: Send + Sync {
    /// 获取会话
    ///
    /// # Arguments
    ///
    /// * `role` - 用户角色
    ///
    /// # Returns
    ///
    /// 返回数据库会话
    async fn get_session(&self, role: &str) -> DbResult<Session>;

    /// 获取连接池状态
    ///
    /// # Returns
    ///
    /// 返回连接池状态信息
    fn status(&self) -> PoolStatus;

    /// 获取配置
    ///
    /// # Returns
    ///
    /// 返回连接池配置
    fn config(&self) -> &DbConfig;
}

/// 数据库会话抽象 trait
///
/// 定义数据库会话的通用接口，便于测试和替换实现
#[async_trait]
pub trait DatabaseSession: Send + Sync {
    /// 执行 SQL（带权限检查）
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL 语句
    ///
    /// # Returns
    ///
    /// 返回执行结果
    async fn execute(&self, sql: &str) -> DbResult<ExecResult>;

    /// 执行原始 SQL（不带权限检查）
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL 语句
    ///
    /// # Returns
    ///
    /// 返回执行结果
    async fn execute_raw(&self, sql: &str) -> DbResult<ExecResult>;

    /// 执行原始 DDL（仅限管理员）
    ///
    /// # Arguments
    ///
    /// * `sql` - DDL SQL 语句
    ///
    /// # Returns
    ///
    /// 返回执行结果
    async fn execute_raw_ddl(&self, sql: &str) -> DbResult<ExecResult>;

    /// 开始事务
    ///
    /// # Returns
    ///
    /// 返回成功或错误
    async fn begin_transaction(&self) -> DbResult<()>;

    /// 提交事务
    ///
    /// # Returns
    ///
    /// 返回成功或错误
    async fn commit(&self) -> DbResult<()>;

    /// 回滚事务
    ///
    /// # Returns
    ///
    /// 返回成功或错误
    async fn rollback(&self) -> DbResult<()>;

    /// 获取角色
    ///
    /// # Returns
    ///
    /// 返回用户角色
    fn role(&self) -> &str;

    /// 是否在事务中
    ///
    /// # Returns
    ///
    /// 返回是否在事务中
    async fn is_in_transaction(&self) -> bool;
}

// ============================================================================
// DbPoolBuilder - 依赖注入构造器
// ============================================================================

/// DbPool 构造器
///
/// 支持部分依赖注入的流式 API 构建模式。
/// 未注入的依赖将使用默认值。
///
/// # Example
///
/// ```ignore
/// use dbnexus::DbPool;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let pool = DbPool::builder()
///         .url("sqlite://example.db")
///         .max_connections(10)
///         .build()
///         .await?;
///     Ok(())
/// }
/// ```
#[derive(Clone, Default)]
pub struct DbPoolBuilder {
    /// 数据库连接 URL（可选，如果未提供则需要 config）
    url: Option<String>,
    /// 数据库配置（可选，如果提供了 url 则自动创建）
    config: Option<DbConfig>,
    /// 管理员角色名称（可选，默认使用配置中的值）
    admin_role: Option<String>,
    /// 暂存的连接池参数（url/config 之前设置时先保存，build 时统一应用）
    pending_max_connections: Option<u32>,
    /// 暂存的最小连接数（同上）
    pending_min_connections: Option<u32>,
    /// 缓存提供者（DI 注入点，优先于内部缓存）
    #[cfg(any(feature = "cache", feature = "oxcache-integration"))]
    cache_provider: Option<Arc<dyn DbCacheProvider + Send + Sync>>,
    /// 统一 DDL 守卫策略
    #[cfg(feature = "sql-parser")]
    ddl_guard: Option<std::sync::Arc<dyn crate::access::DdlGuardPolicy>>,
    /// prepare 缓存容量（None = 不启用）
    #[cfg(feature = "prepare-cache")]
    prepare_cache_capacity: Option<usize>,
}
