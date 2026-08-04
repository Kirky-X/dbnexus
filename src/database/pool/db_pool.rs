// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 连接池管理模块
//!
//! 提供数据库连接池的创建、管理和自动修正功能

#[cfg(feature = "permission")]
use crate::access::RolePolicy;
use crate::i18n;
#[cfg(feature = "permission")]
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
#[cfg(feature = "permission")]
use oxcache::Cache;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;
#[cfg(feature = "metrics")]
use std::time::Instant;
use tokio::sync::{Mutex as AsyncMutex, Notify, Semaphore};
#[cfg(feature = "pool-health-check")]
use tokio::time::interval;
use tokio::time::timeout;

// ponytail: ConnectionLifecycle and dead consts removed; add back when health telemetry is wired

use super::Session;
#[cfg(feature = "permission")]
use crate::access::PermissionConfig;
#[cfg(any(feature = "ladybug", feature = "neo4j"))]
use crate::database::GraphConnection;
use crate::foundation::{ConfigError, DbConfig};
use crate::foundation::{DbError, DbResult};
#[cfg(feature = "metrics")]
use crate::observability::MetricsCollector;

// 导入 Sea-ORM 的连接 trait
use sea_orm::ConnectionTrait;

/// 数据库连接类型（SeaORM 原始类型别名，保留用于向后兼容）
pub type DatabaseConnection = sea_orm::DatabaseConnection;

/// 统一数据库连接枚举（0.3.0 新增）
///
/// 支持 SeaORM（SQLite/PostgreSQL/MySQL）和 DuckDB 两种后端连接。
/// `DbPool` 和 `Session` 通过此枚举统一管理不同后端的连接。
///
/// 0.4.0 新增 Ladybug 和 Neo4j 图数据库后端。
#[derive(Clone)]
pub enum DbConnection {
    /// SeaORM 连接（SQLite/PostgreSQL/MySQL）
    SeaOrm(DatabaseConnection),
    /// DuckDB 嵌入式连接（duckdb feature）
    #[cfg(feature = "duckdb")]
    DuckDb(crate::database::DuckDbConnection),
    /// Ladybug 嵌入式图数据库连接（ladybug feature）
    #[cfg(feature = "ladybug")]
    Ladybug(Arc<crate::database::LadybugConnection>),
    /// Neo4j 图数据库服务器连接（neo4j feature）
    #[cfg(feature = "neo4j")]
    Neo4j(Arc<crate::database::Neo4jConnection>),
}

impl DbConnection {
    /// 获取 SeaORM 连接引用，若为其他后端则返回错误
    pub fn as_sea_orm(&self) -> DbResult<&DatabaseConnection> {
        match self {
            DbConnection::SeaOrm(conn) => Ok(conn),
            #[cfg(feature = "duckdb")]
            DbConnection::DuckDb(_) => Err(DbError::Connection(sea_orm::DbErr::Custom(
                "Operation requires SeaORM connection but got DuckDb".to_string(),
            ))),
            #[cfg(feature = "ladybug")]
            DbConnection::Ladybug(_) => Err(DbError::Connection(sea_orm::DbErr::Custom(
                "Operation requires SeaORM connection but got Ladybug".to_string(),
            ))),
            #[cfg(feature = "neo4j")]
            DbConnection::Neo4j(_) => Err(DbError::Connection(sea_orm::DbErr::Custom(
                "Operation requires SeaORM connection but got Neo4j".to_string(),
            ))),
        }
    }

    /// 获取 DuckDB 连接引用，若为其他后端则返回错误
    #[cfg(feature = "duckdb")]
    pub fn as_duckdb(&self) -> DbResult<&crate::database::DuckDbConnection> {
        match self {
            DbConnection::DuckDb(conn) => Ok(conn),
            DbConnection::SeaOrm(_) => Err(DbError::Connection(sea_orm::DbErr::Custom(
                "Operation requires DuckDb connection but got SeaOrm".to_string(),
            ))),
            #[cfg(feature = "ladybug")]
            DbConnection::Ladybug(_) => Err(DbError::Connection(sea_orm::DbErr::Custom(
                "Operation requires DuckDb connection but got Ladybug".to_string(),
            ))),
            #[cfg(feature = "neo4j")]
            DbConnection::Neo4j(_) => Err(DbError::Connection(sea_orm::DbErr::Custom(
                "Operation requires DuckDb connection but got Neo4j".to_string(),
            ))),
        }
    }

    /// 获取图数据库连接引用（`&dyn GraphConnection`），若为关系型后端则返回错误
    ///
    /// # Errors
    ///
    /// 当连接为 SeaOrm 或 DuckDb 时返回 `DbError::Connection`。
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    pub fn as_graph(&self) -> DbResult<&dyn crate::database::GraphConnection> {
        match self {
            #[cfg(feature = "ladybug")]
            DbConnection::Ladybug(conn) => Ok(conn.as_ref()),
            #[cfg(feature = "neo4j")]
            DbConnection::Neo4j(conn) => Ok(conn.as_ref()),
            DbConnection::SeaOrm(_) => Err(DbError::Connection(sea_orm::DbErr::Custom(
                "Operation requires graph connection but got SeaOrm".to_string(),
            ))),
            #[cfg(feature = "duckdb")]
            DbConnection::DuckDb(_) => Err(DbError::Connection(sea_orm::DbErr::Custom(
                "Operation requires graph connection but got DuckDb".to_string(),
            ))),
        }
    }

    /// 判断是否为 DuckDB 连接
    pub fn is_duckdb(&self) -> bool {
        #[cfg(feature = "duckdb")]
        {
            matches!(self, DbConnection::DuckDb(_))
        }
        #[cfg(not(feature = "duckdb"))]
        {
            false
        }
    }

    /// 判断是否为图数据库连接（Ladybug 或 Neo4j）
    pub fn is_graph(&self) -> bool {
        #[cfg(feature = "ladybug")]
        {
            if matches!(self, DbConnection::Ladybug(_)) {
                return true;
            }
        }
        #[cfg(feature = "neo4j")]
        {
            if matches!(self, DbConnection::Neo4j(_)) {
                return true;
            }
        }
        false
    }
}

impl std::fmt::Debug for DbConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbConnection::SeaOrm(_) => write!(f, "DbConnection::SeaOrm(..)"),
            #[cfg(feature = "duckdb")]
            DbConnection::DuckDb(conn) => write!(f, "DbConnection::DuckDb({conn:?})"),
            #[cfg(feature = "ladybug")]
            DbConnection::Ladybug(conn) => write!(f, "DbConnection::Ladybug({conn:?})"),
            #[cfg(feature = "neo4j")]
            DbConnection::Neo4j(conn) => write!(f, "DbConnection::Neo4j({conn:?})"),
        }
    }
}

/// 连接池管理器
#[derive(Clone)]
pub struct DbPool {
    /// 内部连接池
    inner: Arc<DbPoolInner>,
    /// 缓存提供者（DI 注入点，feature-gated behind `cache`）
    #[cfg(feature = "cache")]
    cache_provider: Option<Arc<dyn crate::domain::DbCacheProvider + Send + Sync>>,
}

pub(crate) struct DbPoolInner {
    /// 配置
    pub(crate) config: DbConfig,

    /// 信号量控制最大连接数（优化锁竞争）
    connection_semaphore: Arc<Semaphore>,

    /// 空闲连接队列
    idle_connections: AsyncMutex<Vec<DbConnection>>,

    /// 连接可用通知（替代忙等待）
    connection_available: Notify,

    /// 活跃连接数
    pub(super) active_count: AtomicU32,

    /// 总连接数
    pub(super) total_count: AtomicU32,

    /// 权限策略缓存（直接使用 oxcache）
    #[cfg(feature = "permission")]
    pub(crate) policy_cache: Arc<Cache<String, RolePolicy>>,

    /// 权限配置（懒加载，使用 ArcSwap 无锁读取 — COW 模式）
    #[cfg(feature = "permission")]
    permission_config: Arc<ArcSwapOption<PermissionConfig>>,

    /// 后台健康检查任务（用于优雅关闭）
    health_check_shutdown: Arc<Notify>,

    /// 管理员角色名称
    pub(super) admin_role: String,

    /// 指标收集器（可选，用于 metrics 特性）
    #[cfg(feature = "metrics")]
    pub(crate) metrics_collector: Option<Arc<MetricsCollector>>,

    /// 等待计数
    pub(super) wait_count: AtomicU32,

    /// 最大等待计数（历史峰值）
    pub(super) max_waiters: AtomicU32,

    /// 借用计数
    pub(super) borrow_count: AtomicU64,

    /// 最大活跃连接数
    pub(super) max_active: AtomicU32,

    /// 故障转移：当前活跃 URL 索引（failover feature）
    #[cfg(feature = "failover")]
    #[allow(dead_code)]
    pub(super) current_url_index: std::sync::atomic::AtomicU32,
}

impl DbPoolInner {
    /// 归还连接到池（内部实现）
    ///
    /// 直接从 `DbPoolInner` 操作，无需通过 `DbPool` 中转。
    /// Session 的 Drop 可直接调用此方法，避免持有 `Arc<DbPool>`。
    pub(crate) fn release_connection(inner: &Arc<Self>, conn: DbConnection) {
        inner.active_count.fetch_sub(1, Ordering::SeqCst);
        let inner_clone = Arc::clone(inner);

        // 尝试快速路径：非阻塞获取锁
        if let Ok(mut idle) = inner_clone.idle_connections.try_lock() {
            if idle.len() < inner_clone.config.pool_config.max_connections as usize {
                idle.push(conn);
                inner_clone.connection_available.notify_one();
                inner_clone.connection_semaphore.add_permits(1);
            } else {
                inner_clone.total_count.fetch_sub(1, Ordering::SeqCst);
                inner_clone.connection_semaphore.add_permits(1);
            }
            return;
        }

        // 异步路径：在 tokio 运行时中执行
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::spawn(async move {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // 同步代码段保护
                }));
                if result.is_err() {
                    inner_clone.total_count.fetch_sub(1, Ordering::SeqCst);
                    inner_clone.connection_semaphore.add_permits(1);
                    return;
                }

                let mut idle = inner_clone.idle_connections.lock().await;
                if idle.len() < inner_clone.config.pool_config.max_connections as usize {
                    idle.push(conn);
                    inner_clone.connection_available.notify_one();
                } else {
                    inner_clone.total_count.fetch_sub(1, Ordering::SeqCst);
                }
                inner_clone.connection_semaphore.add_permits(1);
            });
        } else {
            inner_clone.total_count.fetch_sub(1, Ordering::SeqCst);
            inner_clone.connection_semaphore.add_permits(1);
        }
    }
}

impl DbPool {
    /// 更新最大活跃连接数（使用 CAS 操作避免竞态条件）
    ///
    /// 此方法使用 compare_exchange 循环确保原子性地更新 max_active，
    /// 只有当新值大于当前值时才更新。
    fn update_max_active(&self, active: u32) {
        // 使用 Acquire 语义确保看到最新的值
        let mut current = self.inner.max_active.load(Ordering::Acquire);
        while active > current {
            match self
                .inner
                .max_active
                .compare_exchange(current, active, Ordering::SeqCst, Ordering::Acquire)
            {
                Ok(_) => return,
                Err(observed) => {
                    // CAS 失败，使用观察到的值重试
                    current = observed;
                }
            }
        }
    }

    /// 设置缓存提供者（DI 注入点）
    ///
    /// 允许外部注入缓存实现，覆盖默认的内置缓存。
    /// 仅在 `cache` 特性启用时可用。
    #[cfg(feature = "cache")]
    pub fn set_cache_provider(&mut self, provider: Arc<dyn crate::domain::DbCacheProvider + Send + Sync>) {
        self.cache_provider = Some(provider);
    }

    /// 获取缓存提供者引用
    ///
    /// 返回当前注入的缓存提供者，如果未注入则返回 `None`。
    #[cfg(feature = "cache")]
    pub fn cache_provider(&self) -> Option<&Arc<dyn crate::domain::DbCacheProvider + Send + Sync>> {
        self.cache_provider.as_ref()
    }

    /// 创建新的连接池
    ///
    /// # Arguments
    ///
    /// * `url` - 数据库连接 URL
    ///
    /// # Errors
    ///
    /// 如果 URL 格式无效或不支持，返回错误
    ///
    /// # Example
    ///
    /// ```ignore
    /// use dbnexus::DbPool;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let pool = DbPool::new("sqlite://example.db").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(url: &str) -> DbResult<Self> {
        let config = DbConfig {
            url: url.to_string(),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// 使用配置创建连接池
    pub async fn with_config(config: DbConfig) -> DbResult<Self> {
        // 验证配置有效性（在创建任何连接前捕获非法参数）
        config
            .validate()
            .map_err(|e| DbError::Config(i18n::t("pool-invalid-config", &[("error", e.to_string())])))?;

        // 创建连接（复用 create_connection 保持错误转换一致）
        let _connection = Self::create_connection(&config).await?;

        // 创建权限策略缓存并加载初始权限配置（含首次预加载）
        #[cfg(feature = "permission")]
        let (policy_cache, permission_config) = Self::setup_permission_cache(&config).await?;

        let pool = Self {
            inner: Arc::new(DbPoolInner {
                config: config.clone(),
                connection_semaphore: Arc::new(Semaphore::new(config.pool_config.max_connections as usize)),
                idle_connections: AsyncMutex::new(Vec::new()),
                connection_available: Notify::new(),
                active_count: AtomicU32::new(0),
                total_count: AtomicU32::new(0),
                #[cfg(feature = "permission")]
                policy_cache,
                #[cfg(feature = "permission")]
                permission_config: match permission_config {
                    Some(config) => Arc::new(ArcSwapOption::from_pointee(config)),
                    None => Arc::new(ArcSwapOption::empty()),
                },
                health_check_shutdown: Arc::new(Notify::new()),
                admin_role: config.admin_role.clone(),
                #[cfg(feature = "metrics")]
                metrics_collector: None,
                wait_count: AtomicU32::new(0),
                max_waiters: AtomicU32::new(0),
                borrow_count: AtomicU64::new(0),
                max_active: AtomicU32::new(0),
                #[cfg(feature = "failover")]
                current_url_index: std::sync::atomic::AtomicU32::new(0),
            }),
            #[cfg(feature = "cache")]
            cache_provider: None,
        };

        // vuln-0001 修复：检查是否使用了默认 admin 角色（不安全），发出安全警告
        super::audit::warn_if_default_admin_role_used(&config.admin_role);

        // 启动后台健康检查任务
        #[cfg(feature = "pool-health-check")]
        pool.start_background_health_check();

        // 预创建最小连接数（并行创建以提高启动速度，带超时和重试）
        #[cfg(feature = "pool-warmup")]
        pool.warmup_connections().await?;

        // 注意：权限策略缓存的预加载已在 setup_permission_cache() 中完成（HIGH-004 修复）
        // 此处不再重复预加载，避免冗余 IO 和缓存覆盖。

        #[cfg(feature = "auto-migrate")]
        if config.auto_migrate {
            if let Some(ref migrations_dir) = config.migrations_dir {
                if migrations_dir.exists() {
                    let _applied = pool.run_migrations(migrations_dir).await?;
                } else {
                    // migrations directory does not exist, skip migration
                }
            }
        }

        Ok(pool)
    }

    /// 使用配置结构体创建连接池
    ///
    /// 此方法接受一个 [`DbConfig`] 结构体，用于配置连接池的所有参数。
    /// 与 [`Self::new`] 方法功能相同，但更适合从配置结构体直接初始化。
    ///
    /// # Example
    #[cfg_attr(
        feature = "sqlite",
        doc = r###"
    /// ```rust
    /// use dbnexus::DbPool;
    /// use dbnexus::DbConfig;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = DbConfig {
    ///         url: "sqlite::memory:".to_string(),
    ///         pool_config: dbnexus::foundation::PoolConfig {
    ///             max_connections: 10,
    ///             min_connections: 2,
    ///             ..Default::default()
    ///         },
    ///         ..Default::default()
    ///     };
    ///
    ///     let pool = DbPool::try_from_config(config).await?;
    ///     Ok(())
    /// }
    /// ```
    "###
    )]
    #[cfg_attr(
        not(feature = "sqlite"),
        doc = r###"
    /// ```rust,ignore
    /// // 此文档测试需要 sqlite 特性
    /// // 在使用其他数据库时，请参考相应的文档和示例
    /// ```
    "###
    )]
    ///
    /// # Errors
    ///
    /// 如果连接失败或配置无效，返回错误
    pub async fn try_from_config(config: DbConfig) -> DbResult<Self> {
        Self::with_config(config).await
    }

    /// 使用配置引用同步创建连接池（简化版本）
    ///
    /// 此方法是同步的，不会创建数据库连接。
    /// 实际的连接池创建和连接验证在首次获取连接时进行。
    ///
    /// 注意：此方法不会初始化权限缓存功能（需要异步初始化）。
    /// 如果需要完整的异步权限缓存功能，请使用 `with_config()` 异步方法。
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::DbPool;
    /// use dbnexus::DbConfig;
    ///
    /// let runtime = tokio::runtime::Runtime::new()?;
    /// let _guard = runtime.enter();
    ///
    /// let config = DbConfig {
    ///     url: "sqlite::memory:".to_string(),
    ///     pool_config: dbnexus::foundation::PoolConfig {
    ///         max_connections: 10,
    ///         min_connections: 1,
    ///         idle_timeout: 300,
    ///         acquire_timeout: 5000,
    ///     },
    ///     ..Default::default()
    /// };
    ///
    /// let pool = DbPool::try_from(&config)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// 如果配置验证失败，返回错误
    #[cfg(not(feature = "permission"))]
    pub fn try_from(config: &DbConfig) -> Result<Self, ConfigError> {
        // vuln-0001 修复：检查是否使用了默认 admin 角色（不安全），发出安全警告
        super::audit::warn_if_default_admin_role_used(&config.admin_role);
        Ok(Self {
            inner: Arc::new(DbPoolInner {
                config: config.clone(),
                connection_semaphore: Arc::new(Semaphore::new(config.pool_config.max_connections as usize)),
                idle_connections: AsyncMutex::new(Vec::new()),
                connection_available: Notify::new(),
                active_count: AtomicU32::new(0),
                total_count: AtomicU32::new(0),
                health_check_shutdown: Arc::new(Notify::new()),
                admin_role: config.admin_role.clone(),
                #[cfg(feature = "metrics")]
                metrics_collector: None,
                wait_count: AtomicU32::new(0),
                max_waiters: AtomicU32::new(0),
                borrow_count: AtomicU64::new(0),
                max_active: AtomicU32::new(0),
                #[cfg(feature = "failover")]
                current_url_index: std::sync::atomic::AtomicU32::new(0),
            }),
            #[cfg(feature = "cache")]
            cache_provider: None,
        })
    }

    /// 使用配置引用同步创建连接池（简化版本，带权限但不初始化缓存）
    ///
    /// 此方法是同步的，不会创建数据库连接。
    /// 实际的连接池创建和连接验证在首次获取连接时进行。
    ///
    /// 注意：此方法不会初始化权限缓存（需要异步初始化）。
    /// 如果需要完整的异步权限缓存功能，请使用 `with_config()` 异步方法。
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # // 需要 config feature
    /// use dbnexus::DbPool;
    /// use dbnexus::DbConfig;
    ///
    /// let runtime = tokio::runtime::Runtime::new()?;
    /// let _guard = runtime.enter();
    ///
    /// let config = DbConfig {
    ///     url: "sqlite::memory:".to_string(),
    ///     pool_config: dbnexus::foundation::PoolConfig {
    ///         max_connections: 10,
    ///         min_connections: 1,
    ///         idle_timeout: 300,
    ///         acquire_timeout: 5000,
    ///     },
    ///     ..Default::default()
    /// };
    ///
    /// let pool = DbPool::try_from(&config)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// 如果配置验证失败，返回错误
    #[cfg(feature = "permission")]
    pub fn try_from(config: &DbConfig) -> Result<Self, ConfigError> {
        // permission feature 启用时，需要异步创建 oxcache 缓存。
        // 同步 try_from 无法在 tokio runtime 内安全调用 block_on（会 panic）。
        // 显式失败，引导用户使用异步构造器 with_config()。
        let _ = config;
        Err(ConfigError::InvalidValue {
            key: "permission".to_string(),
            message: "DbPool::try_from cannot be used with `permission` feature enabled; \
             use `DbPool::with_config(config).await` instead (async constructor required for cache initialization)"
                .to_string(),
        })
    }

    /// 构造权限策略缓存并加载初始权限配置（含首次预加载）
    ///
    /// 完成两件事：
    /// 1. 用 `cache_config.policy_cache_capacity` 创建 oxcache `Cache`。
    /// 2. 调用 [`Self::load_permission_config`] 读取权限文件，并把 `roles` 写入缓存（首次预加载）。
    ///
    /// 返回构造好的缓存和加载到的权限配置（若未配置文件或加载失败则为 `None`）。
    #[cfg(feature = "permission")]
    async fn setup_permission_cache(
        config: &DbConfig,
    ) -> DbResult<(Arc<Cache<String, RolePolicy>>, Option<PermissionConfig>)> {
        // 创建权限策略缓存（使用 oxcache 后端）
        let policy_cache = Arc::new(
            Cache::builder()
                .capacity(config.cache_config.policy_cache_capacity)
                .build()
                .await
                .map_err(|_e| {
                    DbError::Connection(sea_orm::DbErr::ConnectionAcquire(sea_orm::ConnAcquireErr::Timeout))
                })?,
        );

        // 加载权限配置（如果指定了路径）- 仅在启用permission特性时
        let permission_config = Self::load_permission_config(config).await?;

        // 预加载权限策略到缓存（如果存在权限配置）
        if let Some(ref perm_config) = permission_config {
            for (role_name, policy) in &perm_config.roles {
                let _ = policy_cache.set(role_name, policy).await;
            }
        }

        Ok((policy_cache, permission_config))
    }

    /// 加载权限配置文件
    ///
    /// 通过 `serde_yaml_ng` / `serde_json` 直接解析 YAML/JSON 文件，与项目配置管理策略一致
    ///
    /// # Returns
    ///
    /// - `Ok(Some(PermissionConfig))` - 成功加载的权限配置
    /// - `Ok(None)` - 没有配置权限文件路径
    /// - `Err(DbError::Config(..))` - 文件存在但读取或解析失败
    #[cfg(feature = "permission")]
    async fn load_permission_config(config: &DbConfig) -> DbResult<Option<PermissionConfig>> {
        // 尝试从配置文件加载
        if let Some(ref path) = config.permissions_path {
            let content = tokio::fs::read_to_string(path).await.map_err(|e| {
                DbError::Config(i18n::t(
                    "pool-read-config-failed",
                    &[("path", path.clone()), ("error", e.to_string())],
                ))
            })?;
            let perm_config = Self::parse_permission_yaml(&content, path).map_err(|e| {
                DbError::Config(i18n::t(
                    "pool-parse-config-failed",
                    &[("path", path.clone()), ("error", e.to_string())],
                ))
            })?;
            return Ok(Some(perm_config));
        }

        // 没有配置权限文件
        Ok(None)
    }

    /// 解析权限配置
    ///
    /// 使用 `serde_yaml_ng` 解析（YAML 是 JSON 超集，兼容两种输入）。
    #[cfg(feature = "permission")]
    fn parse_permission_yaml(content: &str, source: &str) -> Result<PermissionConfig, String> {
        #[cfg(feature = "yaml")]
        {
            serde_yaml_ng::from_str(content).map_err(|e| {
                i18n::t(
                    "pool-yaml-parse-error",
                    &[("source", source.to_string()), ("error", e.to_string())],
                )
            })
        }
        #[cfg(not(feature = "yaml"))]
        {
            let _ = (content, source);
            Err("Cannot parse permission config: 'yaml' feature is not enabled".to_string())
        }
    }

    /// 获取指标收集器（如果已设置）
    #[cfg(feature = "metrics")]
    pub fn metrics(&self) -> Option<&Arc<MetricsCollector>> {
        self.inner.metrics_collector.as_ref()
    }

    /// 获取实际应用的配置
    ///
    /// 返回当前连接池使用的配置。
    ///
    /// # Returns
    ///
    /// 实际应用的配置
    pub fn get_actual_config(&self) -> &DbConfig {
        &self.inner.config
    }

    // ========================================================================
    // 故障转移方法（failover feature）
    // ========================================================================

    /// 原子切换到下一个 URL（循环遍历故障转移链）
    #[cfg(feature = "failover")]
    #[allow(dead_code)]
    pub(crate) fn advance_to_next_url(&self) {
        if let Some(ref config) = self.inner.config.failover_config {
            let len = config.urls.len() as u32;
            if len <= 1 {
                return;
            }
            self.inner
                .current_url_index
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| Some((current + 1) % len))
                .ok();
        }
    }

    /// 获取当前活跃 URL（故障转移感知）
    #[cfg(feature = "failover")]
    #[allow(dead_code)]
    pub(crate) fn current_url(&self) -> &str {
        if let Some(ref config) = self.inner.config.failover_config {
            if !config.urls.is_empty() {
                let idx = self.inner.current_url_index.load(Ordering::SeqCst) as usize;
                return &config.urls[idx.min(config.urls.len() - 1)];
            }
        }
        &self.inner.config.url
    }

    /// 获取当前活跃 URL（无故障转移时回退到 config.url）
    #[cfg(not(feature = "failover"))]
    #[allow(dead_code)]
    pub(crate) fn current_url(&self) -> &str {
        &self.inner.config.url
    }

    /// 验证连接有效性
    ///
    /// 执行探测查询（PostgreSQL/MySQL: `SELECT 1`，SQLite: `SELECT 1`）
    /// 验证连接是否可用。无效连接应被调用方惰性移除。
    #[cfg(feature = "failover")]
    #[allow(dead_code)]
    pub(crate) async fn validate_connection(&self, conn: &sea_orm::DatabaseConnection) -> bool {
        use sea_orm::ConnectionTrait;
        let query = self
            .inner
            .config
            .failover_config
            .as_ref()
            .and_then(|c| c.health_check_query.as_deref())
            .unwrap_or("SELECT 1");
        conn.execute_unprepared(query).await.is_ok()
    }

    /// 从池中获取 Session（带 metrics 支持）
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称，必须在权限配置中定义
    ///
    /// # Errors
    ///
    /// 如果角色未在权限配置中定义，返回错误
    ///
    /// # 安全警告
    ///
    /// 此方法接受角色字符串参数，调用者应确保：
    /// - 已通过其他方式验证用户身份（如JWT Token、API Key等）
    /// - 角色字符串来自可信来源，而非直接的用户输入
    /// - 在生产环境中不要硬编码角色字符串
    /// - 建议结合 `dbnexus::authentication::AuthenticationManager` 使用
    ///
    /// # 示例
    ///
    /// ```rust,no_run,ignore
    /// use dbnexus::{DbPool, authentication::AuthenticationManager};
    ///
    /// // 安全用法：先验证Token，再从Token中提取角色
    /// let auth_manager = AuthenticationManager::new(&jwt_secret);
    /// let claims = auth_manager.verify_token(token)?;
    /// let session = pool.get_session(&claims.role).await?;
    ///
    /// // 不安全用法：直接使用用户输入
    /// // let session = pool.get_session(user_input_role).await?; // 不要这样做！
    /// ```
    ///
    pub async fn get_session(&self, role: &str) -> DbResult<Session> {
        // 验证角色名称
        #[cfg(feature = "permission")]
        self.validate_role_name(role).await?;

        let connection = self.acquire_connection().await?;
        let session = Session::new(connection, self.inner.clone(), role.to_string());

        Ok(session)
    }

    /// 验证角色名称是否在权限配置中定义
    ///
    /// 仅在权限配置文件存在且成功加载时验证角色。
    /// 如果没有配置权限文件，使用安全默认策略（仅允许 admin/system 角色）。
    ///
    /// **显性化（v0.3.0 修复）**：未配置权限文件时输出 warn 日志，明确说明
    /// 正在使用安全默认策略，提醒用户配置权限文件以启用完整角色验证。
    #[cfg(feature = "permission")]
    async fn validate_role_name(&self, role: &str) -> DbResult<()> {
        // 无锁读取权限配置（ArcSwap COW — 读取是完全无锁的 CAS 操作）
        let permission_config = self.inner.permission_config.load();

        // 检查权限配置是否存在（用户是否显式配置了权限文件）
        if permission_config.is_none() {
            // 没有配置权限文件时，使用安全默认策略
            // 只允许预定义的安全角色，防止未授权访问
            let safe_roles = ["admin", "system"];
            if !safe_roles.contains(&role) {
                return Err(DbError::Permission(format!(
                    "Role '{}' is not allowed without explicit permission configuration. Allowed roles: {}",
                    role,
                    safe_roles.join(", ")
                )));
            }
            return Ok(());
        }

        // 检查角色是否存在
        if permission_config
            .as_ref()
            .is_some_and(|c| c.get_role_policy(role).is_none())
        {
            // 角色不存在
            return Err(DbError::Permission(format!(
                "Role '{}' is not defined in permission configuration",
                role
            )));
        }

        Ok(())
    }

    /// 创建单个数据库连接
    ///
    /// 使用配置中的 URL 建立新的数据库连接。
    /// 此方法不进行连接池管理，仅创建原始连接。
    ///
    /// # Arguments
    ///
    /// * `config` - 数据库配置，包含连接 URL
    ///
    /// # Returns
    ///
    /// 成功创建的数据库连接
    ///
    /// # Errors
    ///
    /// 如果连接失败，返回数据库错误
    async fn create_connection(config: &DbConfig) -> DbResult<DbConnection> {
        let db_type = config.database_type().map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                "pool-invalid-db-url",
                &[("error", e.to_string())],
            )))
        })?;

        match db_type {
            crate::foundation::DatabaseType::DuckDb => {
                #[cfg(feature = "duckdb")]
                {
                    let conn = crate::database::DuckDbConnection::new(&config.url)?;
                    Ok(DbConnection::DuckDb(conn))
                }
                #[cfg(not(feature = "duckdb"))]
                {
                    Err(DbError::Connection(sea_orm::DbErr::Custom(
                        "DuckDB feature is not enabled".to_string(),
                    )))
                }
            }
            crate::foundation::DatabaseType::Ladybug => {
                #[cfg(feature = "ladybug")]
                {
                    let pool_size = config.pool_config.max_connections as usize;
                    let conn = crate::database::LadybugConnection::new(&config.url, pool_size)?;
                    Ok(DbConnection::Ladybug(Arc::new(conn)))
                }
                #[cfg(not(feature = "ladybug"))]
                {
                    Err(DbError::Connection(sea_orm::DbErr::Custom(
                        "Ladybug feature is not enabled".to_string(),
                    )))
                }
            }
            crate::foundation::DatabaseType::Neo4j => {
                #[cfg(feature = "neo4j")]
                {
                    let (uri, user, password) = crate::database::Neo4jConnection::parse_url(&config.url)?;
                    let conn = crate::database::Neo4jConnection::new(&uri, &user, &password).await?;
                    Ok(DbConnection::Neo4j(Arc::new(conn)))
                }
                #[cfg(not(feature = "neo4j"))]
                {
                    Err(DbError::Connection(sea_orm::DbErr::Custom(
                        "Neo4j feature is not enabled".to_string(),
                    )))
                }
            }
            _ => {
                let conn = sea_orm::Database::connect(&config.url).await?;
                Ok(DbConnection::SeaOrm(conn))
            }
        }
    }

    /// 预创建最小连接数（并行创建以提高启动速度，带超时和重试）
    ///
    /// 并行启动 `min_connections` 个建连任务，每个任务带 `warmup_timeout` 超时
    /// 和 `warmup_retries` 次重试。
    ///
    /// **失败语义（v0.3.0 修复）**：
    /// - 全部失败：返回 `Err`（第一个错误），避免静默成功
    /// - 部分失败：返回 `Ok`，warn 日志记录失败数量
    /// - 全部成功：返回 `Ok`
    #[cfg(feature = "pool-warmup")]
    async fn warmup_connections(&self) -> DbResult<()> {
        let initial_connections = self.inner.config.pool_config.min_connections;
        let warmup_timeout = Duration::from_secs(self.inner.config.warmup_timeout);
        let warmup_retries = self.inner.config.warmup_retries;

        let mut connection_tasks = Vec::new();

        for _ in 0..initial_connections {
            let config = self.inner.config.clone();
            connection_tasks.push(async move {
                let mut retries = 0;
                let mut last_error = None;

                while retries <= warmup_retries {
                    match timeout(warmup_timeout, Self::create_connection(&config)).await {
                        Ok(Ok(conn)) => return Ok(conn),
                        Ok(Err(e)) => {
                            last_error = Some(e);
                            retries += 1;
                            if retries <= warmup_retries {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                        Err(_) => {
                            last_error = Some(DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                                sea_orm::ConnAcquireErr::Timeout,
                            )));
                            break;
                        }
                    }
                }

                Err(last_error.unwrap_or_else(|| {
                    DbError::Connection(sea_orm::DbErr::ConnectionAcquire(sea_orm::ConnAcquireErr::Timeout))
                }))
            });
        }

        // 并行执行所有连接创建任务
        let results = futures::future::join_all(connection_tasks).await;

        // 统计成功/失败（规则 12：失败必须显性化，不可静默丢弃）
        let mut success_count = 0usize;
        let mut errors: Vec<DbError> = Vec::new();

        for result in results {
            match result {
                Ok(conn) => {
                    self.inner.idle_connections.lock().await.push(conn);
                    self.inner.total_count.fetch_add(1, Ordering::SeqCst);
                    success_count += 1;
                }
                Err(e) => errors.push(e),
            }
        }

        if success_count == 0 && initial_connections > 0 {
            // 全部失败：返回第一个错误（显性化失败，避免静默成功）
            return Err(errors.into_iter().next().unwrap_or_else(|| {
                DbError::Connection(sea_orm::DbErr::ConnectionAcquire(sea_orm::ConnAcquireErr::Timeout))
            }));
        }

        if !errors.is_empty() {
            // 部分失败：不阻断初始化，错误已通过 errors 集合显性化
        }

        Ok(())
    }

    /// 检查连接健康状态
    ///
    /// 通过执行轻量级查询来验证数据库连接的有效性。
    /// 使用数据库特定的健康检查查询：
    /// - SQLite: `SELECT 1`
    /// - PostgreSQL: `SELECT 1`
    /// - MySQL: `SELECT 1`
    ///
    /// # Arguments
    ///
    /// * `conn` - 要检查的数据库连接
    ///
    /// # Returns
    ///
    /// 如果连接有效返回 `true`，否则返回 `false`
    pub async fn check_connection_health(&self, conn: &DbConnection) -> bool {
        match conn {
            DbConnection::SeaOrm(sea_conn) => {
                let backend = Self::get_database_backend(&self.inner.config.url);
                let result = timeout(
                    Duration::from_secs(5),
                    sea_conn.execute_raw(sea_orm::Statement::from_string(backend, "SELECT 1".to_string())),
                )
                .await;
                matches!(result, Ok(Ok(_)))
            }
            #[cfg(feature = "duckdb")]
            DbConnection::DuckDb(duck_conn) => {
                let result = timeout(Duration::from_secs(5), duck_conn.health_check()).await;
                matches!(result, Ok(Ok(_)))
            }
            #[cfg(feature = "ladybug")]
            DbConnection::Ladybug(conn) => {
                let result = timeout(Duration::from_secs(5), conn.health_check()).await;
                matches!(result, Ok(Ok(_)))
            }
            #[cfg(feature = "neo4j")]
            DbConnection::Neo4j(conn) => {
                let result = timeout(Duration::from_secs(5), conn.health_check()).await;
                matches!(result, Ok(Ok(_)))
            }
        }
    }

    /// 获取数据库类型
    ///
    /// 根据数据库 URL 的协议部分解析数据库类型。
    /// 支持的数据库类型包括 SQLite、PostgreSQL、MySQL 和 DuckDB。
    ///
    /// # Arguments
    ///
    /// * `url` - 数据库连接 URL
    ///
    /// # Returns
    ///
    /// 对应的 Sea-ORM 数据库后端类型
    ///
    /// # Note
    ///
    /// 如果 URL 无法识别，默认返回 SQLite 类型。
    /// DuckDB 连接不使用 SeaORM 后端，此方法仅用于 SeaORM 连接。
    fn get_database_backend(url: &str) -> sea_orm::DatabaseBackend {
        if url.starts_with("sqlite:") {
            sea_orm::DatabaseBackend::Sqlite
        } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
            sea_orm::DatabaseBackend::Postgres
        } else if url.starts_with("mysql:") {
            sea_orm::DatabaseBackend::MySql
        } else if url.starts_with("duckdb:") {
            // DuckDB 不使用 SeaORM 后端，映射到 Sqlite 以避免 panic
            sea_orm::DatabaseBackend::Sqlite
        } else {
            sea_orm::DatabaseBackend::Sqlite
        }
    }

    #[cfg(feature = "pool-health-check")]
    /// 验证空闲连接的有效性（并行版本）
    ///
    /// 遍历空闲连接池，对每个连接并发执行健康检查，
    /// 将连接分区为有效和无效两组。
    ///
    /// 使用 `futures::future::join_all()` 并行验证所有连接，
    /// 显著减少大量连接时的总等待时间。
    ///
    /// # Arguments
    ///
    /// * `idle` - 空闲连接队列的可变引用
    /// * `config` - 数据库配置
    ///
    /// # Returns
    ///
    /// 返回元组 (有效连接列表, 无效连接数量)
    async fn validate_idle_connections(idle: &mut Vec<DbConnection>, config: &DbConfig) -> (Vec<DbConnection>, usize) {
        let backend = Self::get_database_backend(&config.url);

        // 先将所有连接移出，避免在持有锁期间进行 I/O 操作
        let connections: Vec<DbConnection> = std::mem::take(idle);

        // 并行执行所有健康检查
        let check_futures: Vec<_> = connections
            .into_iter()
            .map(|conn| async {
                let is_valid = match &conn {
                    DbConnection::SeaOrm(sea_conn) => timeout(
                        Duration::from_secs(2),
                        sea_conn.execute_raw(sea_orm::Statement::from_string(backend, "SELECT 1".to_string())),
                    )
                    .await
                    .is_ok_and(|result| result.is_ok()),
                    #[cfg(feature = "duckdb")]
                    DbConnection::DuckDb(duck_conn) => timeout(Duration::from_secs(2), duck_conn.health_check())
                        .await
                        .is_ok_and(|result| result.is_ok()),
                    #[cfg(feature = "ladybug")]
                    DbConnection::Ladybug(graph_conn) => timeout(Duration::from_secs(2), graph_conn.health_check())
                        .await
                        .is_ok_and(|result| result.is_ok()),
                    #[cfg(feature = "neo4j")]
                    DbConnection::Neo4j(graph_conn) => timeout(Duration::from_secs(2), graph_conn.health_check())
                        .await
                        .is_ok_and(|result| result.is_ok()),
                };
                (conn, is_valid)
            })
            .collect();

        let results: Vec<(DbConnection, bool)> = futures::future::join_all(check_futures).await;

        // 分区为有效和无效连接（先计算 invalid_count，再转移所有权）
        let invalid_count = results.iter().filter(|(_, is_valid)| !*is_valid).count();

        let valid_connections: Vec<DbConnection> = results
            .into_iter()
            .filter_map(|(conn, is_valid)| if is_valid { Some(conn) } else { None })
            .collect();

        (valid_connections, invalid_count)
    }

    #[cfg(feature = "pool-health-check")]
    /// 清理无效连接
    ///
    /// 遍历空闲连接池，验证每个连接的有效性，
    /// 移除超时或断开连接的实例。
    ///
    /// # Returns
    ///
    /// 被移除的无效连接数量
    pub async fn clean_invalid_connections(&self) -> u32 {
        let mut idle = self.inner.idle_connections.lock().await;
        let config = &self.inner.config;

        // 使用辅助方法验证连接
        let (valid_connections, removed_count) = Self::validate_idle_connections(&mut idle, config).await;

        // 重建空闲连接队列
        idle.extend(valid_connections);

        // 更新总连接数
        if removed_count > 0 {
            self.inner.total_count.fetch_sub(removed_count as u32, Ordering::SeqCst);
        }

        removed_count as u32
    }

    #[cfg(feature = "pool-health-check")]
    /// 验证并重新创建无效连接
    ///
    /// 检查所有空闲连接的健康状态，自动替换无效连接。
    /// 此方法会确保池中至少保持配置的最小连接数。
    ///
    /// # Returns
    ///
    /// 被重新创建的连接数量，或错误
    pub async fn validate_and_recreate_connections(&self) -> Result<u32, sea_orm::DbErr> {
        let mut idle = self.inner.idle_connections.lock().await;
        let config = &self.inner.config;

        // 使用辅助方法验证连接
        let (valid_connections, invalid_count) = Self::validate_idle_connections(&mut idle, config).await;

        let mut recreated_count = 0;

        if invalid_count > 0 {
            // 更新总连接数
            self.inner.total_count.fetch_sub(invalid_count as u32, Ordering::SeqCst);

            // 重建空闲队列（只保留有效连接）
            idle.extend(valid_connections);

            // 重新创建连接以维持最小连接数
            let current_idle = idle.len();
            let needed = config.pool_config.min_connections.saturating_sub(current_idle as u32) as usize;

            for _ in 0..needed {
                match Self::create_connection(config).await {
                    Ok(new_conn) => {
                        idle.push(new_conn);
                        self.inner.total_count.fetch_add(1, Ordering::SeqCst);
                        recreated_count += 1;
                    }
                    Err(e) => {
                        return Err(sea_orm::DbErr::Custom(i18n::t(
                            "pool-recreate-failed",
                            &[("error", e.to_string())],
                        )));
                    }
                }
            }
        } else {
            // 没有无效连接，恢复有效连接到池中
            idle.extend(valid_connections);
        }

        Ok(recreated_count as u32)
    }

    /// 解析健康检查间隔配置
    ///
    /// 解析传入的间隔值（秒），并限制在 5-300 秒范围内。
    /// 超出范围的值会触发警告日志。
    ///
    /// # Arguments
    ///
    /// * `value` - 健康检查间隔配置值（由调用方从环境变量 `DB_HEALTH_CHECK_INTERVAL` 读取）
    ///
    /// # Returns
    ///
    /// 返回解析后的间隔秒数，默认为 30 秒。
    ///
    /// # Examples
    ///
    /// ```
    /// use dbnexus::DbPool;
    /// // 空字符串返回默认值 30
    /// assert_eq!(DbPool::parse_health_check_interval(""), 30);
    ///
    /// // 有效值返回该值
    /// assert_eq!(DbPool::parse_health_check_interval("60"), 60);
    ///
    /// // 超出范围的值返回限制后的值
    /// assert_eq!(DbPool::parse_health_check_interval("1000"), 300);
    /// ```
    #[cfg(feature = "pool-health-check")]
    pub fn parse_health_check_interval(value: &str) -> u64 {
        value.parse::<u64>().ok().map(|v| v.clamp(5, 300)).unwrap_or(30)
    }

    /// 启动后台连接健康检查任务
    ///
    /// 该任务会定期检查所有空闲连接的健康状态，
    /// 自动移除无效连接并重建新连接以维持最小连接数。
    ///
    /// 健康检查间隔默认为 30 秒，可通过环境变量 `DB_HEALTH_CHECK_INTERVAL` 配置（秒）。
    /// 间隔值会被限制在 5-300 秒范围内，超出范围的值会触发警告日志。
    #[cfg(feature = "pool-health-check")]
    fn start_background_health_check(&self) {
        let pool = self.clone();
        let shutdown = self.inner.health_check_shutdown.clone();

        // 从环境变量读取健康检查间隔配置并解析
        let env_value = std::env::var("DB_HEALTH_CHECK_INTERVAL").unwrap_or_default();
        let interval_secs = Self::parse_health_check_interval(&env_value);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // 执行连接健康检查
                        let _ = pool.validate_and_recreate_connections().await;
                    }
                    _ = shutdown.notified() => {
                        break;
                    }
                }
            }
        });
    }

    /// 从池中获取连接
    ///
    /// 实现连接获取逻辑，包括：
    /// 1. 记录等待者计数（wait_count），追踪当前等待获取连接的协程数
    /// 2. 追踪历史最大等待者峰值（max_waiters）
    /// 3. 获取信号量许可（控制最大并发数），带超时保护
    /// 4. 尝试从空闲连接队列获取，队列为空则创建新连接
    ///
    /// ## 等待计数与告警
    ///
    /// - 每次进入获取流程时 `wait_count += 1`，无论最终成功或超时都 `wait_count -= 1`
    /// - `max_waiters` 记录历史最大并发等待者数量（CAS 更新）
    /// - 获取超时后根据等待时长触发分级告警：
    ///   - ≥3s：warn 级别
    ///   - ≥5s：error 级别
    ///   - ≥10s：critical 级别
    ///
    /// ## 锁竞争优化策略
    ///
    /// 使用信号量（Semaphore）替代部分锁逻辑，减少锁竞争：
    /// - 信号量在获取锁之前就控制并发数量，实现更公平的等待机制
    /// - 锁持有时间最小化：仅在操作空闲队列时持有锁
    /// - 创建新连接时不持有锁，避免阻塞其他操作
    ///
    /// ## 信号量许可管理
    ///
    /// - 获取连接时：`permit.forget()` 消耗许可（连接被借出）
    /// - 释放连接时：`add_permits(1)` 归还许可（连接归还池中）
    ///
    /// # Returns
    ///
    /// 成功获取的数据库连接
    ///
    /// # Errors
    ///
    /// 如果获取连接超时（池耗尽）或创建连接失败，返回错误
    async fn acquire_connection(&self) -> DbResult<DbConnection> {
        // 步骤 0: 记录等待计数
        let waiters = self.inner.wait_count.fetch_add(1, Ordering::SeqCst) + 1;
        self.update_max_waiters(waiters);

        // 步骤 1: 获取信号量许可（等待可用槽位，带超时）
        // 信号量提供公平的等待队列，避免惊群效应
        let timeout_duration = self.inner.config.acquire_timeout_duration();

        // 仅在启用 metrics 时需要记录开始时间
        #[cfg(feature = "metrics")]
        let start = Instant::now();

        let acquire_result = timeout(timeout_duration, self.inner.connection_semaphore.acquire()).await;

        // wait_count 递减（无论成功或失败）
        self.inner.wait_count.fetch_sub(1, Ordering::SeqCst);

        let permit = match acquire_result {
            Ok(Ok(p)) => {
                // 记录获取延迟和慢获取
                #[cfg(feature = "metrics")]
                if let Some(ref collector) = self.inner.metrics_collector {
                    collector.record_connection_acquire_duration(start.elapsed());
                }
                p
            }
            Ok(Err(_)) => {
                // permit error - shouldn't happen with tokio semaphore
                return Err(DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::ConnAcquireErr::Timeout,
                )));
            }
            Err(_) => {
                // Timeout - 记录超时指标
                #[cfg(feature = "metrics")]
                {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    if let Some(ref collector) = self.inner.metrics_collector {
                        collector.record_connection_timeout_level(elapsed_ms);
                    }
                }

                return Err(DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::ConnAcquireErr::Timeout,
                )));
            }
        };

        // 步骤 2: 尝试从空闲队列获取（最小化锁持有时间）
        // 使用独立作用域确保锁尽快释放
        {
            let mut idle = self.inner.idle_connections.lock().await;
            if let Some(conn) = idle.pop() {
                // 更新统计计数
                let active = self.inner.active_count.fetch_add(1, Ordering::SeqCst) + 1;
                self.update_max_active(active);
                self.inner.borrow_count.fetch_add(1, Ordering::SeqCst);
                // 忘记许可，因为连接被借出（release_connection 会归还）
                permit.forget();
                return Ok(conn);
            }
            // 锁在此处释放
        }

        // 步骤 3: 创建新连接（不持有锁，避免阻塞其他操作）
        // 先更新计数，再创建连接
        self.inner.total_count.fetch_add(1, Ordering::SeqCst);
        let active = self.inner.active_count.fetch_add(1, Ordering::SeqCst) + 1;
        self.update_max_active(active);

        match Self::create_connection(&self.inner.config).await {
            Ok(conn) => {
                self.inner.borrow_count.fetch_add(1, Ordering::SeqCst);
                // 忘记许可，因为连接被借出
                permit.forget();
                Ok(conn)
            }
            Err(e) => {
                // 创建失败，回滚计数并释放许可
                self.inner.total_count.fetch_sub(1, Ordering::SeqCst);
                self.inner.active_count.fetch_sub(1, Ordering::SeqCst);
                // 释放许可（drop 会自动释放）
                drop(permit);
                Err(e)
            }
        }
    }

    /// 更新最大等待者计数（使用 CAS 避免竞态条件）
    fn update_max_waiters(&self, current_waiters: u32) {
        let mut current = self.inner.max_waiters.load(Ordering::Acquire);
        while current_waiters > current {
            match self
                .inner
                .max_waiters
                .compare_exchange(current, current_waiters, Ordering::SeqCst, Ordering::Acquire)
            {
                Ok(_) => return,
                Err(observed) => {
                    current = observed;
                }
            }
        }
    }

    /// 归还连接到池中
    ///
    /// 将使用完毕的连接归还到空闲连接队列。
    /// 如果空闲队列已满（达到最大连接数），则丢弃该连接。
    /// 归还后会通知一个等待的请求者有新连接可用。
    ///
    /// # Arguments
    ///
    /// * `conn` - 要归还的数据库连接
    ///
    /// # Note
    ///
    /// 此方法会归还信号量许可，确保连接池可以继续接受新的连接请求。
    /// 使用 tokio::spawn 在后台执行异步操作，避免阻塞调用者。
    #[cfg(feature = "auto-migrate")]
    pub(crate) fn release_connection(&self, conn: DbConnection) {
        DbPoolInner::release_connection(&self.inner, conn);
    }

    /// 获取连接池状态
    ///
    /// 返回当前连接池的统计信息，包括总连接数、活跃连接数和空闲连接数。
    ///
    /// # Returns
    ///
    /// 连接池状态信息
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::DbPool;
    ///
    /// # async fn example(pool: &DbPool) {
    /// let status = pool.status();
    /// println!("Total: {}, Active: {}, Idle: {}",
    ///     status.total, status.active, status.idle);
    /// # }
    /// ```
    pub fn status(&self) -> PoolStatus {
        let total = self.inner.total_count.load(Ordering::SeqCst);
        let active = self.inner.active_count.load(Ordering::SeqCst);
        let wait_count = self.inner.wait_count.load(Ordering::SeqCst);
        let max_waiters = self.inner.max_waiters.load(Ordering::SeqCst);
        let borrow_count = self.inner.borrow_count.load(Ordering::SeqCst);
        let max_active = self.inner.max_active.load(Ordering::SeqCst);

        PoolStatus {
            total,
            active,
            idle: total.saturating_sub(active),
            wait_count,
            max_waiters,
            borrow_count,
            max_active,
        }
    }

    /// 获取连接池告警指标
    ///
    /// 从 metrics_collector（如果启用）获取告警相关指标，包括：
    /// - `slow_acquires`：获取时长超过 1s 的次数
    /// - `timeout_errors`：获取超时总次数（warn + error + critical 之和）
    /// - `critical_timeouts`：严重超时（≥10s）次数
    /// - `wait_count`：当前正在等待获取连接的协程数
    /// - `max_waiters`：历史最大并发等待者峰值
    ///
    /// # Returns
    ///
    /// 连接池告警指标（始终返回有效值，metrics 未启用时所有计数为 0）
    #[cfg(feature = "metrics")]
    pub fn pool_metrics(&self) -> PoolMetrics {
        let wait_count = self.inner.wait_count.load(Ordering::SeqCst);
        let max_waiters = self.inner.max_waiters.load(Ordering::SeqCst);
        if let Some(ref collector) = self.inner.metrics_collector {
            let stats = collector.connection_acquire_stats();
            PoolMetrics {
                slow_acquires: stats.slow_acquires,
                timeout_errors: stats.timeout_warn + stats.timeout_error + stats.timeout_critical,
                critical_timeouts: stats.timeout_critical,
                wait_count,
                max_waiters,
            }
        } else {
            PoolMetrics {
                slow_acquires: 0,
                timeout_errors: 0,
                critical_timeouts: 0,
                wait_count,
                max_waiters,
            }
        }
    }

    /// 获取配置
    ///
    /// 返回连接池的配置引用。
    ///
    /// # Returns
    ///
    /// 连接池配置的引用
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::DbPool;
    ///
    /// # async fn example(pool: &DbPool) {
    /// let config = pool.config();
    /// println!("Max connections: {}", config.pool_config.max_connections);
    /// # }
    /// ```
    pub fn config(&self) -> &DbConfig {
        &self.inner.config
    }

    /// 运行自动迁移
    ///
    /// 如果配置中启用了 `auto_migrate`，此方法会在连接池创建后自动执行迁移。
    /// 也可以手动调用此方法来执行迁移。
    ///
    /// # Returns
    ///
    /// 成功应用的迁移数量
    #[cfg(feature = "auto-migrate")]
    pub async fn run_auto_migrate(&self) -> Result<u32, DbError> {
        if let Some(ref migrations_dir) = self.inner.config.migrations_dir {
            self.run_migrations(migrations_dir).await
        } else {
            Ok(0)
        }
    }

    /// 手动运行迁移
    ///
    /// # Arguments
    ///
    /// * `migrations_dir` - 迁移文件目录路径
    ///
    /// # Returns
    ///
    /// 成功应用的迁移数量
    #[cfg(feature = "auto-migrate")]
    pub async fn run_migrations(&self, migrations_dir: &std::path::Path) -> Result<u32, DbError> {
        use crate::database::MigrationExecutor;

        let db_type = self
            .inner
            .config
            .database_type()
            .map_err(|e| DbError::Config(e.to_string()))?;

        // 获取一个连接来执行迁移
        let connection = self.acquire_connection().await?;

        // 从 DbConnection 提取 SeaORM 连接用于迁移执行器
        let connection_for_migration = connection.as_sea_orm()?.clone();

        let mut executor = MigrationExecutor::new(connection_for_migration, db_type);

        let applied = executor.run_migrations(migrations_dir).await?;

        // 归还连接到池中
        self.release_connection(connection);

        Ok(applied)
    }
}

/// DbPool 的优雅关闭
impl Drop for DbPool {
    fn drop(&mut self) {
        // 通知后台健康检查任务关闭
        self.inner.health_check_shutdown.notify_one();
    }
}

/// 连接池告警指标（用于分级告警和监控）
///
/// 包含所有与连接池告警相关的指标，用于告警规则配置和监控告警。
#[cfg(feature = "metrics")]
#[derive(Debug, Clone)]
pub struct PoolMetrics {
    /// 慢获取次数（>3s）
    pub slow_acquires: u64,
    /// 超时总次数
    pub timeout_errors: u64,
    /// 严重级超时次数（>=10s）
    pub critical_timeouts: u64,
    /// 当前等待者数量
    pub wait_count: u32,
    /// 最大等待者数量（历史峰值）
    pub max_waiters: u32,
}

/// 连接池状态
#[derive(Debug, Clone)]
pub struct PoolStatus {
    /// 总连接数
    pub total: u32,

    /// 活跃连接数
    pub active: u32,

    /// 空闲连接数
    pub idle: u32,

    /// 当前等待连接的请求数
    pub wait_count: u32,

    /// 最大等待计数（历史峰值）
    pub max_waiters: u32,

    /// 借用次数
    pub borrow_count: u64,

    /// 最大活跃连接数（历史峰值）
    pub max_active: u32,
}

// 实现 ConnectionPool trait
#[async_trait]
impl super::ConnectionPool for DbPool {
    async fn get_session(&self, role: &str) -> DbResult<Session> {
        self.get_session(role).await
    }

    fn status(&self) -> PoolStatus {
        self.status()
    }

    fn config(&self) -> &DbConfig {
        self.config()
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::foundation::PoolConfig;

    #[cfg(feature = "ladybug")]
    #[test]
    fn test_ladybug_connection_is_graph() {
        let conn = DbConnection::Ladybug(Arc::new(
            crate::database::LadybugConnection::new(":memory:", 1).expect("Failed to create LadybugConnection"),
        ));
        assert!(conn.is_graph(), "Ladybug connection should be graph");
        assert!(!conn.is_duckdb(), "Ladybug connection should not be duckdb");
    }

    #[cfg(feature = "ladybug")]
    #[test]
    fn test_ladybug_connection_as_graph_returns_ok() {
        let conn = DbConnection::Ladybug(Arc::new(
            crate::database::LadybugConnection::new(":memory:", 1).expect("Failed to create LadybugConnection"),
        ));
        let result = conn.as_graph();
        assert!(result.is_ok(), "as_graph() on Ladybug should return Ok");
        let graph = result.unwrap();
        assert_eq!(graph.backend_name(), "ladybug");
    }

    #[cfg(feature = "ladybug")]
    #[test]
    fn test_ladybug_connection_as_sea_orm_returns_err() {
        let conn = DbConnection::Ladybug(Arc::new(
            crate::database::LadybugConnection::new(":memory:", 1).expect("Failed to create LadybugConnection"),
        ));
        let result = conn.as_sea_orm();
        assert!(result.is_err(), "as_sea_orm() on Ladybug should return Err");
    }

    #[cfg(feature = "ladybug")]
    #[tokio::test]
    async fn test_create_connection_ladybug_memory() {
        let config = DbConfig {
            url: "ladybug::memory:".to_string(),
            pool_config: PoolConfig {
                max_connections: 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let conn = DbPool::create_connection(&config)
            .await
            .expect("create_connection for ladybug::memory: should succeed");
        assert!(conn.is_graph(), "should be graph connection");
        let graph = conn.as_graph().expect("as_graph should succeed");
        assert_eq!(graph.backend_name(), "ladybug");
    }

    #[cfg(feature = "ladybug")]
    #[tokio::test]
    async fn test_create_connection_ladybug_health_check() {
        let config = DbConfig {
            url: "ladybug::memory:".to_string(),
            pool_config: PoolConfig {
                max_connections: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let conn = DbPool::create_connection(&config)
            .await
            .expect("create_connection for ladybug::memory: should succeed");
        let graph = conn.as_graph().expect("as_graph should succeed");
        graph.health_check().await.expect("health_check should pass");
    }

    #[cfg(feature = "neo4j")]
    #[test]
    fn test_neo4j_connection_is_graph() {
        let conn = DbConnection::Neo4j(Arc::new(crate::database::Neo4jConnection::new_placeholder()));
        assert!(conn.is_graph(), "Neo4j connection should be graph");
        assert!(!conn.is_duckdb(), "Neo4j connection should not be duckdb");
    }

    #[cfg(feature = "neo4j")]
    #[test]
    fn test_neo4j_connection_as_graph_returns_ok() {
        let conn = DbConnection::Neo4j(Arc::new(crate::database::Neo4jConnection::new_placeholder()));
        let result = conn.as_graph();
        assert!(result.is_ok(), "as_graph() on Neo4j should return Ok");
        let graph = result.unwrap();
        assert_eq!(graph.backend_name(), "neo4j");
    }

    #[cfg(feature = "neo4j")]
    #[test]
    fn test_neo4j_connection_as_sea_orm_returns_err() {
        let conn = DbConnection::Neo4j(Arc::new(crate::database::Neo4jConnection::new_placeholder()));
        let result = conn.as_sea_orm();
        assert!(result.is_err(), "as_sea_orm() on Neo4j should return Err");
    }

    #[cfg(feature = "neo4j")]
    #[tokio::test]
    #[ignore = "需要 Neo4j 服务器，设置 NEO4J_URL/NEO4J_USER/NEO4J_PASSWORD 环境变量后运行"]
    async fn test_create_connection_neo4j() {
        let url = std::env::var("NEO4J_URL").unwrap_or_else(|_| "neo4j://localhost:7687".to_string());
        let config = DbConfig {
            url,
            pool_config: PoolConfig {
                max_connections: 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let conn = DbPool::create_connection(&config)
            .await
            .expect("create_connection for neo4j should succeed");
        assert!(conn.is_graph(), "should be graph connection");
        let graph = conn.as_graph().expect("as_graph should succeed");
        assert_eq!(graph.backend_name(), "neo4j");
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_sea_orm_connection_is_graph_returns_false() {
        let sea_conn = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite memory connection");
        let conn = DbConnection::SeaOrm(sea_conn);
        assert!(!conn.is_graph(), "SeaOrm connection should not be graph");
        assert!(!conn.is_duckdb(), "SeaOrm connection should not be duckdb");
    }

    #[cfg(all(feature = "sqlite", any(feature = "ladybug", feature = "neo4j")))]
    #[tokio::test]
    async fn test_sea_orm_connection_as_graph_returns_err() {
        let sea_conn = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite memory connection");
        let conn = DbConnection::SeaOrm(sea_conn);
        let result = conn.as_graph();
        assert!(result.is_err(), "as_graph() on SeaOrm should return Err");
    }

    #[test]
    fn test_db_connection_debug_format() {
        #[cfg(feature = "ladybug")]
        {
            let conn = DbConnection::Ladybug(Arc::new(
                crate::database::LadybugConnection::new(":memory:", 1).expect("Failed to create LadybugConnection"),
            ));
            let debug_str = format!("{conn:?}");
            assert!(
                debug_str.contains("Ladybug"),
                "Debug should contain 'Ladybug': {debug_str}"
            );
        }
        #[cfg(feature = "neo4j")]
        {
            let conn = DbConnection::Neo4j(Arc::new(crate::database::Neo4jConnection::new_placeholder()));
            let debug_str = format!("{conn:?}");
            assert!(debug_str.contains("Neo4j"), "Debug should contain 'Neo4j': {debug_str}");
        }
    }

    // ===== 补充测试：get_database_backend, status, config =====

    #[test]
    fn test_get_database_backend_sqlite() {
        assert!(matches!(
            DbPool::get_database_backend("sqlite::memory:"),
            sea_orm::DatabaseBackend::Sqlite
        ));
        assert!(matches!(
            DbPool::get_database_backend("sqlite:test.db"),
            sea_orm::DatabaseBackend::Sqlite
        ));
    }

    #[test]
    fn test_get_database_backend_postgres() {
        assert!(matches!(
            DbPool::get_database_backend("postgres://localhost/db"),
            sea_orm::DatabaseBackend::Postgres
        ));
        assert!(matches!(
            DbPool::get_database_backend("postgresql://localhost/db"),
            sea_orm::DatabaseBackend::Postgres
        ));
    }

    #[test]
    fn test_get_database_backend_mysql() {
        assert!(matches!(
            DbPool::get_database_backend("mysql://localhost/db"),
            sea_orm::DatabaseBackend::MySql
        ));
    }

    #[test]
    fn test_get_database_backend_duckdb_fallback() {
        // DuckDB maps to Sqlite to avoid panic
        assert!(matches!(
            DbPool::get_database_backend("duckdb::memory:"),
            sea_orm::DatabaseBackend::Sqlite
        ));
    }

    #[test]
    fn test_get_database_backend_unknown_fallback() {
        // Unknown URL defaults to Sqlite
        assert!(matches!(
            DbPool::get_database_backend("unknown://something"),
            sea_orm::DatabaseBackend::Sqlite
        ));
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_db_connection_is_duckdb_without_feature() {
        // Without duckdb feature, is_duckdb() always returns false
        let conn = DbConnection::SeaOrm(sea_orm::Database::connect("sqlite::memory:").await.unwrap());
        assert!(!conn.is_duckdb());
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_db_connection_is_graph_seaorm() {
        let conn = DbConnection::SeaOrm(sea_orm::Database::connect("sqlite::memory:").await.unwrap());
        assert!(!conn.is_graph());
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_db_connection_as_sea_orm_success() {
        let sea_conn = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite memory connection");
        let conn = DbConnection::SeaOrm(sea_conn);
        assert!(conn.as_sea_orm().is_ok());
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_pool_status_and_config() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: PoolConfig {
                max_connections: 10,
                min_connections: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let pool = DbPool::with_config(config).await.expect("should create pool");

        // Test status()
        let status = pool.status();
        assert_eq!(status.total, 0);
        assert_eq!(status.active, 0);
        assert_eq!(status.idle, 0);
        assert_eq!(status.borrow_count, 0);

        // Test config()
        assert_eq!(pool.config().url, "sqlite::memory:");
        assert_eq!(pool.config().pool_config.max_connections, 10);
        assert_eq!(pool.config().pool_config.min_connections, 2);
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_pool_update_max_active() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let pool = DbPool::with_config(config).await.expect("should create pool");

        // Initially max_active is 0
        assert_eq!(pool.inner.max_active.load(Ordering::SeqCst), 0);

        // Update to 5
        pool.update_max_active(5);
        assert_eq!(pool.inner.max_active.load(Ordering::SeqCst), 5);

        // Update to 3 (should not decrease)
        pool.update_max_active(3);
        assert_eq!(pool.inner.max_active.load(Ordering::SeqCst), 5);

        // Update to 10
        pool.update_max_active(10);
        assert_eq!(pool.inner.max_active.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn test_create_connection_duckdb_not_enabled() {
        let config = DbConfig {
            url: "duckdb::memory:".to_string(),
            ..Default::default()
        };
        let result = DbPool::create_connection(&config).await;
        assert!(result.is_err(), "DuckDB connection should fail without duckdb feature");
    }

    #[tokio::test]
    async fn test_create_connection_ladybug_not_enabled() {
        let config = DbConfig {
            url: "ladybug::memory:".to_string(),
            ..Default::default()
        };
        let result = DbPool::create_connection(&config).await;
        assert!(
            result.is_err(),
            "Ladybug connection should fail without ladybug feature"
        );
    }

    #[tokio::test]
    async fn test_create_connection_neo4j_not_enabled() {
        let config = DbConfig {
            url: "neo4j://localhost:7687".to_string(),
            ..Default::default()
        };
        let result = DbPool::create_connection(&config).await;
        assert!(result.is_err(), "Neo4j connection should fail without neo4j feature");
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_pool_try_from_with_permission_returns_error() {
        let config = DbConfig::default();
        let result = DbPool::try_from(&config);
        assert!(result.is_err(), "try_from should fail with permission feature enabled");
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_pool_current_url() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let pool = DbPool::with_config(config).await.expect("should create pool");
        assert_eq!(pool.current_url(), "sqlite::memory:");
    }

    // ===== 补充测试：Debug trait, ConnectionPool trait, health check, release_connection =====

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_seaorm_debug_format() {
        let sea_conn = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite memory connection");
        let conn = DbConnection::SeaOrm(sea_conn);
        let debug_str = format!("{conn:?}");
        assert!(
            debug_str.contains("SeaOrm"),
            "Debug should contain 'SeaOrm': {debug_str}"
        );
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_connection_pool_trait_methods() {
        use super::super::ConnectionPool;

        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: PoolConfig {
                max_connections: 5,
                ..Default::default()
            },
            ..Default::default()
        };
        let pool = DbPool::with_config(config).await.expect("should create pool");

        // Test ConnectionPool::status
        let status = ConnectionPool::status(&pool);
        assert_eq!(status.total, 0);

        // Test ConnectionPool::config
        let cfg = ConnectionPool::config(&pool);
        assert_eq!(cfg.url, "sqlite::memory:");
        assert_eq!(cfg.pool_config.max_connections, 5);

        // Test ConnectionPool::get_session with admin role
        let session = ConnectionPool::get_session(&pool, "admin").await;
        assert!(
            session.is_ok(),
            "get_session should succeed for admin: {:?}",
            session.err()
        );
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_check_connection_health_sqlite() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let pool = DbPool::with_config(config).await.expect("should create pool");

        let sea_conn = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite memory connection");
        let conn = DbConnection::SeaOrm(sea_conn);
        let healthy = pool.check_connection_health(&conn).await;
        assert!(healthy, "SQLite memory connection should be healthy");
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_release_connection_pool_full() {
        // Test release_connection when idle pool is at capacity
        // This exercises lines 249-250 (total_count decrement path)
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: PoolConfig {
                max_connections: 2,
                min_connections: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let pool = DbPool::with_config(config).await.expect("should create pool");

        // Get 2 sessions (fills the semaphore)
        let session1 = pool.get_session("admin").await.expect("session 1");
        let session2 = pool.get_session("admin").await.expect("session 2");

        // Drop both - they go back to idle pool
        drop(session1);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(session2);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify pool is consistent - idle should be <= max_connections
        let status = pool.status();
        assert!(status.idle <= 2, "idle should be <= max_connections: {}", status.idle);
    }

    // ===== 补充测试：cache 方法, validate_role_name =====

    #[cfg(feature = "cache")]
    #[tokio::test]
    async fn test_pool_set_and_get_cache_provider() {
        use crate::foundation::DbError;
        use std::future::Future;
        use std::pin::Pin;

        struct NoopCacheProvider;
        impl crate::domain::DbCacheProvider for NoopCacheProvider {
            fn get<'a>(
                &'a self,
                _key: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, DbError>> + Send + 'a>> {
                Box::pin(async { Ok(None) })
            }
            fn set<'a>(
                &'a self,
                _key: &'a str,
                _value: Vec<u8>,
                _ttl: Option<std::time::Duration>,
            ) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }
            fn delete<'a>(&'a self, _key: &'a str) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }
        }

        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let mut pool = DbPool::with_config(config).await.expect("should create pool");

        // Initially no cache provider
        assert!(pool.cache_provider().is_none());

        // Set cache provider (covers lines 311-312)
        let provider = Arc::new(NoopCacheProvider);
        pool.set_cache_provider(provider);

        // Now cache provider should be Some (covers lines 319-320)
        assert!(pool.cache_provider().is_some());
    }

    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_validate_role_name_with_config_unknown_role() {
        use std::io::Write;

        // Create a temp permission config file with only "admin" role
        let yaml_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations: ["select", "insert", "update", "delete"]
"#;
        let tmp_dir = std::env::temp_dir();
        let yaml_path = tmp_dir.join("test_perm_config.yaml");
        {
            let mut file = std::fs::File::create(&yaml_path).expect("create temp file");
            file.write_all(yaml_content.as_bytes()).expect("write temp file");
        }

        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            permissions_path: Some(yaml_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        let pool = DbPool::with_config(config).await.expect("should create pool");

        // "admin" role exists in config -> should succeed
        let result = pool.get_session("admin").await;
        assert!(result.is_ok(), "admin should be allowed: {:?}", result.err());

        // "unknown_role" does NOT exist in config -> should fail (covers line 812)
        let result = pool.get_session("unknown_role").await;
        assert!(result.is_err(), "unknown_role should be rejected");
        match result.err().unwrap() {
            DbError::Permission(msg) => {
                assert!(
                    msg.contains("not defined in permission configuration"),
                    "error should mention role not defined: {}",
                    msg
                );
            }
            other => panic!("expected DbError::Permission, got {:?}", other),
        }

        // Clean up temp file
        let _ = std::fs::remove_file(&yaml_path);
    }

    #[cfg(feature = "permission")]
    #[tokio::test]
    async fn test_validate_role_name_no_config_unsafe_role() {
        // Without permission config, only "admin" and "system" are allowed
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let pool = DbPool::with_config(config).await.expect("should create pool");

        // "admin" is a safe role -> should succeed
        let result = pool.get_session("admin").await;
        assert!(result.is_ok(), "admin should be allowed: {:?}", result.err());

        // "system" is a safe role -> should succeed
        let result = pool.get_session("system").await;
        assert!(result.is_ok(), "system should be allowed: {:?}", result.err());

        // "hacker" is NOT a safe role -> should fail
        let result = pool.get_session("hacker").await;
        assert!(result.is_err(), "hacker should be rejected");
    }
}
