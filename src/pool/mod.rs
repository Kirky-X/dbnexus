// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池管理模块
//!
//! 提供数据库连接池的创建、管理和自动修正功能

mod db_pool;
mod session;

pub use db_pool::{DatabaseConnection, DbPool, PoolStatus};
pub use session::Session;

// 导出迁移执行器供内部使用
#[cfg(feature = "migration")]
pub(crate) use crate::migration::MigrationExecutor;

// 导入 MetricsCollectorTrait
#[cfg(feature = "metrics")]
use crate::metrics::MetricsCollectorTrait;

// 导入 Sea-ORM 的事务 trait 和连接 trait
pub use sea_orm::{ConnectionTrait, TransactionTrait};

use crate::config::{DbConfig, DbResult};
use async_trait::async_trait;
use sea_orm::ExecResult;
use std::sync::Arc;

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
    async fn execute(&mut self, sql: &str) -> DbResult<ExecResult>;

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
    async fn begin_transaction(&mut self) -> DbResult<()>;

    /// 提交事务
    ///
    /// # Returns
    ///
    /// 返回成功或错误
    async fn commit(&mut self) -> DbResult<()>;

    /// 回滚事务
    ///
    /// # Returns
    ///
    /// 返回成功或错误
    async fn rollback(&mut self) -> DbResult<()>;

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
    fn is_in_transaction(&self) -> bool;
}

// ============================================================================
// DbPoolBuilder - 依赖注入构造器
// ============================================================================

#[cfg(feature = "metrics")]
use crate::metrics::MetricsCollector;
#[cfg(feature = "permission")]
use crate::permission::PermissionConfig;

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
    /// 指标收集器（可选）
    #[cfg(feature = "metrics")]
    metrics_collector: Option<Arc<dyn MetricsCollectorTrait>>,
    /// 权限配置（可选）
    #[cfg(feature = "permission")]
    permission_config: Option<PermissionConfig>,
    /// 管理员角色名称（可选，默认使用配置中的值）
    admin_role: Option<String>,
}

impl DbPoolBuilder {
    /// 创建新的构造器
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置数据库连接 URL
    ///
    /// # Arguments
    ///
    /// * `url` - 数据库连接 URL 字符串
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    pub fn url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    /// 设置数据库配置
    ///
    /// # Arguments
    ///
    /// * `config` - 数据库配置
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    pub fn config(mut self, config: DbConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// 设置指标收集器
    ///
    /// # Arguments
    ///
    /// * `metrics_collector` - 指标收集器实例
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    #[cfg(feature = "metrics")]
    pub fn metrics_collector(mut self, metrics_collector: Arc<MetricsCollector>) -> Self {
        self.metrics_collector = Some(metrics_collector);
        self
    }

    /// 设置权限配置
    ///
    /// # Arguments
    ///
    /// * `permission_config` - 权限配置
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    #[cfg(feature = "permission")]
    pub fn permission_config(mut self, permission_config: PermissionConfig) -> Self {
        self.permission_config = Some(permission_config);
        self
    }

    /// 设置管理员角色名称
    ///
    /// # Arguments
    ///
    /// * `admin_role` - 管理员角色名称
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    pub fn admin_role(mut self, admin_role: &str) -> Self {
        self.admin_role = Some(admin_role.to_string());
        self
    }

    /// 设置最大连接数
    ///
    /// # Arguments
    ///
    /// * `max_connections` - 最大连接数
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    pub fn max_connections(mut self, max_connections: u32) -> Self {
        if let Some(ref mut config) = self.config {
            config.set_max_connections(max_connections);
        } else if let Some(ref url) = self.url {
            // 如果只有 url，创建一个默认配置然后修改
            let config = crate::config::DbConfigBuilder::new()
                .url(url)
                .max_connections(max_connections)
                .build()
                .unwrap_or_default();
            self.config = Some(config);
        }
        self
    }

    /// 设置最小连接数
    ///
    /// # Arguments
    ///
    /// * `min_connections` - 最小连接数
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    pub fn min_connections(mut self, min_connections: u32) -> Self {
        if let Some(ref mut config) = self.config {
            config.set_min_connections(min_connections);
        } else if let Some(ref url) = self.url {
            let config = crate::config::DbConfigBuilder::new()
                .url(url)
                .min_connections(min_connections)
                .build()
                .unwrap_or_default();
            self.config = Some(config);
        }
        self
    }

    /// 构建 DbPool
    ///
    /// # Errors
    ///
    /// 如果配置无效或无法连接数据库，返回错误
    ///
    /// # Returns
    ///
    /// 返回新创建的 DbPool 实例
    pub async fn build(self) -> DbResult<DbPool> {
        // 确定最终配置
        let config = if let Some(config) = self.config {
            config
        } else if let Some(url) = self.url {
            // 从 url 创建默认配置
            crate::config::DbConfigBuilder::new()
                .url(&url)
                .max_connections(20)
                .min_connections(5)
                .idle_timeout(300)
                .acquire_timeout(5000)
                .admin_role(self.admin_role.as_deref().unwrap_or("admin"))
                .build()
                .map_err(|e| {
                    crate::config::DbError::Connection(sea_orm::DbErr::Custom(format!("Config error: {:?}", e)))
                })?
        } else {
            return Err(crate::config::DbError::Connection(sea_orm::DbErr::Custom(
                "Either url or config must be provided".to_string(),
            )));
        };

        // 创建 pool
        let pool = DbPool::with_config(config).await?;

        // 注意：以下值已通过 config 设置，不需要额外调用 setter 方法
        // - admin_role: 在 config 创建时已设置（line 327）
        // - metrics_collector: 通过 config 或其他方式设置
        // - permission_config: 在 config 创建时已设置

        Ok(pool)
    }
}

impl std::fmt::Debug for DbPoolBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbPoolBuilder")
            .field("url", &self.url)
            .field("config", &self.config.is_some())
            .field("admin_role", &self.admin_role)
            .finish()
    }
}

// ============================================================================
// DbPoolDependencies - 完全依赖注入结构体
// ============================================================================

/// DbPool 完全依赖注入结构体
///
/// 用于应用层统一管理所有依赖项，支持完全依赖注入模式。
///
/// # Example
///
/// ```ignore
/// use dbnexus::pool::DbPoolDependencies;
/// use std::sync::Arc;
///
/// let deps = DbPoolDependencies {
///     config: my_config,
///     metrics_collector: Some(Arc::new(my_collector)),
///     permission_config: Some(my_perm_config),
///     ..Default::default()
/// };
///
/// let pool = DbPool::with_dependencies(deps).await?;
/// ```
#[derive(Clone, Default)]
pub struct DbPoolDependencies {
    /// 数据库配置（必需）
    pub config: DbConfig,
    /// 指标收集器（可选）
    #[cfg(feature = "metrics")]
    pub metrics_collector: Option<Arc<dyn MetricsCollectorTrait>>,
    /// 权限配置（可选）
    #[cfg(feature = "permission")]
    pub permission_config: Option<PermissionConfig>,
    /// 管理员角色（可选）
    pub admin_role: Option<String>,
}

impl DbPoolDependencies {
    /// 设置数据库配置
    pub fn with_config(mut self, config: DbConfig) -> Self {
        self.config = config;
        self
    }

    /// 设置指标收集器
    #[cfg(feature = "metrics")]
    pub fn with_metrics_collector(mut self, collector: Arc<MetricsCollector>) -> Self {
        self.metrics_collector = Some(collector);
        self
    }

    /// 设置权限配置
    #[cfg(feature = "permission")]
    pub fn with_permission_config(mut self, config: PermissionConfig) -> Self {
        self.permission_config = Some(config);
        self
    }

    /// 设置管理员角色
    pub fn with_admin_role(mut self, role: &str) -> Self {
        self.admin_role = Some(role.to_string());
        self
    }
}
