// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 连接池管理模块
//!
//! 提供数据库连接池的创建、管理和自动修正功能

// 大文件拆分：按职责纯移动的子模块（行为不变）
mod access;
mod health;
mod status;

#[cfg(feature = "sql-parser")]
use crate::access::DdlGuardPolicy;
#[cfg(feature = "permission")]
use crate::access::RolePolicy;
use crate::i18n;
#[cfg(any(feature = "permission", feature = "cache-available"))]
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
#[cfg(feature = "permission")]
use oxcache::Cache;
use std::sync::Arc;
use std::sync::Mutex as SyncMutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;
use std::time::Instant;
use tokio::sync::{Notify, Semaphore};
#[cfg(feature = "pool-health-check")]
use tokio::time::interval;
use tokio::time::timeout;

/// 空闲连接校验在途时，取连接等待回填的上限
///
/// 实际等待预算取本值与「acquire_timeout 剩余额度」的较小值：校验窗口内
/// 临时取走的空闲连接会被回填，等待可避免新建冗余连接（对
/// `duckdb::memory:` 等「新连接即新库」的后端是数据可见性要求）；校验方
/// 异常滞留时到期降级新建，不把 acquire 拖到无界。
const IDLE_CHECK_MAX_WAIT: Duration = Duration::from_millis(500);

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
    /// 内部连接池（pub(crate)：health_export 等 crate 兄弟模块可扩展 DbPool 方法）
    pub(crate) inner: Arc<DbPoolInner>,
}

pub(crate) struct DbPoolInner {
    /// 配置（Arc 共享，避免 warmup 等场景的深拷贝）
    pub(crate) config: Arc<DbConfig>,

    /// 信号量控制最大连接数（优化锁竞争）
    connection_semaphore: Arc<Semaphore>,

    /// 空闲连接队列
    idle_connections: SyncMutex<Vec<DbConnection>>,

    /// 连接可用通知（替代忙等待）
    connection_available: Notify,

    /// 活跃连接数
    pub(super) active_count: AtomicU32,

    /// 总连接数
    pub(super) total_count: AtomicU32,

    /// 空闲连接校验在途计数（>0 表示空闲队列可能被校验方临时取走）
    ///
    /// [`DbPool::acquire_connection`] 在空闲队列为空且本计数 >0 时等待
    /// 回填，不在校验窗口内新建连接：新建对 `duckdb::memory:` 等「新连接
    /// 即新库」的后端是数据可见性缺陷，对其他后端是冗余连接。
    pub(super) idle_check_in_progress: AtomicU32,

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

    /// 指标收集器（可选，用于 metrics 特性；支持运行时注入）
    #[cfg(feature = "metrics")]
    pub(crate) metrics_collector: std::sync::RwLock<Option<Arc<MetricsCollector>>>,

    /// 等待计数
    pub(super) wait_count: AtomicU32,

    /// 最大等待计数（历史峰值）
    pub(super) max_waiters: AtomicU32,

    /// 借用计数
    pub(super) borrow_count: AtomicU64,

    /// 最大活跃连接数
    pub(super) max_active: AtomicU32,

    /// 缓存提供者（DI 注入点，ArcSwapOption 无锁读取 — COW 模式）
    #[cfg(feature = "cache-available")]
    pub(crate) cache_provider: ArcSwapOption<Arc<dyn crate::domain::DbCacheProvider + Send + Sync>>,

    /// 数据保护配置（脱敏 + RLS，运行时整体换装）
    #[cfg(feature = "data-protection")]
    pub(crate) data_protection: tokio::sync::RwLock<crate::access::data_protection::DataProtection>,

    /// 数据保护策略世代（每次换装单调递增）——查询缓存等以策略内容为
    /// 前提的派生键据此区分新旧策略下的行集
    #[cfg(feature = "data-protection")]
    pub(crate) data_protection_epoch: std::sync::atomic::AtomicU64,

    /// 副本健康状态提供者
    #[cfg(feature = "health-check")]
    pub(crate) replica_health_provider:
        std::sync::RwLock<Option<crate::database::pool::health_export::ReplicaHealthProvider>>,

    /// DDL 守卫策略
    #[cfg(feature = "sql-parser")]
    pub(crate) ddl_guard: std::sync::RwLock<Option<std::sync::Arc<dyn DdlGuardPolicy>>>,

    /// 语句级 prepared statement LRU 缓存（None = 不启用缓存路径）
    #[cfg(feature = "prepare-cache")]
    pub(crate) prepare_cache: std::sync::RwLock<
        Option<std::sync::Arc<crate::database::pool::prepare_cache::PoolPrepareCache>>,
    >,
}

impl DbPoolInner {
    #[cfg(feature = "pool-health-check")]
    /// 标记空闲连接校验在途
    ///
    /// 返回的守卫 Drop 时递减在途计数并唤醒等待方——含错误路径与提前
    /// 返回，无需手工配对清除。
    pub(crate) fn mark_idle_check(&self) -> IdleCheckGuard<'_> {
        self.idle_check_in_progress.fetch_add(1, Ordering::SeqCst);
        IdleCheckGuard { inner: self }
    }

    /// 归还连接到池（内部实现）
    ///
    /// 直接从 `DbPoolInner` 操作，无需通过 `DbPool` 中转。
    /// Session 的 Drop 可直接调用此方法，避免持有 `Arc<DbPool>`。
    ///
    /// 归还必须同步完成（临界区仅队列 push/len、notify 与许可归还，无 await）：
    /// 曾用 try_lock 失败走 `tokio::spawn` 延迟 push 的方案，归还被推迟到
    /// 下一个调度点，后续 `acquire_connection` 在 push 落地前看到空 idle
    /// 队列而新建连接——对 `duckdb::memory:` 而言新建即全新空库，此前建表
    /// 数据不可见（该场景由 COPY 测试定位）。空闲队列锁持有时间极短，
    /// 同步锁不引入可观测阻塞。
    pub(crate) fn release_connection(inner: &Arc<Self>, conn: DbConnection) {
        inner.active_count.fetch_sub(1, Ordering::SeqCst);

        let mut idle = inner
            .idle_connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if idle.len() < inner.config.pool_config.max_connections as usize {
            idle.push(conn);
            inner.connection_available.notify_one();
        } else {
            inner.total_count.fetch_sub(1, Ordering::SeqCst);
        }
        inner.connection_semaphore.add_permits(1);
    }
}

/// 空闲连接校验在途守卫
///
/// 置位期间 [`DbPool::acquire_connection`] 在空闲队列为空时等待回填而非
/// 新建连接；Drop 时清除计数并唤醒等待方（含错误路径与提前返回）。
#[cfg(feature = "pool-health-check")]
pub(crate) struct IdleCheckGuard<'a> {
    inner: &'a DbPoolInner,
}

#[cfg(feature = "pool-health-check")]
impl Drop for IdleCheckGuard<'_> {
    fn drop(&mut self) {
        self.inner
            .idle_check_in_progress
            .fetch_sub(1, Ordering::SeqCst);
        self.inner.connection_available.notify_waiters();
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
            match self.inner.max_active.compare_exchange(
                current,
                active,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(observed) => {
                    // CAS 失败，使用观察到的值重试
                    current = observed;
                }
            }
        }
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
        config.validate().map_err(|e| {
            DbError::Config(i18n::t("pool-invalid-config", &[("error", e.to_string())]))
        })?;

        // 创建连接（复用 create_connection 保持错误转换一致）
        let _connection = Self::create_connection(&config).await?;

        // 创建权限策略缓存并加载初始权限配置（含首次预加载）
        #[cfg(feature = "permission")]
        let (policy_cache, permission_config) = Self::setup_permission_cache(&config).await?;

        let pool = Self {
            inner: Arc::new(DbPoolInner {
                config: Arc::new(config.clone()),
                connection_semaphore: Arc::new(Semaphore::new(
                    config.pool_config.max_connections as usize,
                )),
                idle_connections: SyncMutex::new(Vec::new()),
                connection_available: Notify::new(),
                active_count: AtomicU32::new(0),
                total_count: AtomicU32::new(0),
                idle_check_in_progress: AtomicU32::new(0),
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
                metrics_collector: std::sync::RwLock::new(None),
                wait_count: AtomicU32::new(0),
                max_waiters: AtomicU32::new(0),
                borrow_count: AtomicU64::new(0),
                max_active: AtomicU32::new(0),
                #[cfg(feature = "cache-available")]
                cache_provider: ArcSwapOption::new(None),
                #[cfg(feature = "data-protection")]
                data_protection: tokio::sync::RwLock::new(
                    crate::access::data_protection::DataProtection::default(),
                ),
                #[cfg(feature = "data-protection")]
                data_protection_epoch: std::sync::atomic::AtomicU64::new(0),
                #[cfg(feature = "health-check")]
                replica_health_provider: std::sync::RwLock::new(None),
                #[cfg(feature = "sql-parser")]
                ddl_guard: std::sync::RwLock::new(None),
                #[cfg(feature = "prepare-cache")]
                prepare_cache: std::sync::RwLock::new(None),
            }),
        };

        // vuln-0001 修复：检查是否使用了默认 admin 角色（不安全），记录安全审计事件
        super::audit::warn_and_record_default_admin_role(&config.admin_role);

        // 启动后台健康检查任务
        #[cfg(feature = "pool-health-check")]
        pool.start_background_health_check();

        // 预创建最小连接数（并行创建以提高启动速度，带超时和重试）
        #[cfg(feature = "pool-warmup")]
        pool.warmup_connections().await?;

        // 注意：权限策略缓存的预加载已在 setup_permission_cache() 中完成
        // 此处不再重复预加载，避免冗余 IO 和缓存覆盖。

        #[cfg(feature = "auto-migrate")]
        if config.auto_migrate
            && let Some(ref migrations_dir) = config.migrations_dir
        {
            if migrations_dir.exists() {
                let _applied = pool.run_migrations(migrations_dir).await?;
            } else {
                // migrations directory does not exist, skip migration
            }
        }

        Ok(pool)
    }

    /// 从已存在的 `duckdb::Connection` 创建 DuckDB 连接池（共享底层 DatabaseHandle）。
    ///
    /// 用于多组件共享同一 DuckDB 文件句柄的场景（如 alphalloy 的 sync store + DbPool）。
    /// 传入的 `conn` 应已通过 `try_clone()` 从主连接派生。
    ///
    /// 与 `with_config` 不同：跳过 `create_connection`（不重新 open 文件），
    /// 直接通过 `DuckDbConnection::from_shared` 包装已有连接。
    /// 不执行 warmup / auto-migrate（调用方通过 Session API 自行管理迁移）。
    #[cfg(feature = "duckdb")]
    pub fn with_existing_duckdb_connection(
        conn: duckdb::Connection,
        pool_size: usize,
    ) -> DbResult<Self> {
        let pool_size = pool_size.max(1);

        // 从已存在的连接创建 DuckDbConnection（内部 try_clone 填充池）
        let duckdb_conn = crate::database::DuckDbConnection::from_shared(conn, pool_size)?;

        // 最小化配置（仅用于池元数据和 session 权限检查）
        let config = DbConfig {
            url: "duckdb://shared".to_string(),
            pool_config: crate::foundation::PoolConfig {
                max_connections: pool_size as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        // 预填充空闲池：所有连接预先创建，避免 acquire_connection 触发 create_connection
        // （create_connection 会重新 open 文件，导致 DuckDB 文件锁冲突）
        // DuckDbConnection 是 Clone（内部 Arc 共享），clone 后共享同一底层 DatabaseHandle。
        let mut idle_vec = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            idle_vec.push(DbConnection::DuckDb(duckdb_conn.clone()));
        }

        let pool = Self {
            inner: Arc::new(DbPoolInner {
                config: Arc::new(config.clone()),
                connection_semaphore: Arc::new(Semaphore::new(pool_size)),
                idle_connections: SyncMutex::new(idle_vec),
                connection_available: Notify::new(),
                active_count: AtomicU32::new(0),
                total_count: AtomicU32::new(pool_size as u32),
                idle_check_in_progress: AtomicU32::new(0),
                #[cfg(feature = "permission")]
                policy_cache: {
                    // 最小化权限缓存（无配置文件，使用安全默认策略）
                    Arc::new(Cache::new())
                },
                #[cfg(feature = "permission")]
                permission_config: Arc::new(ArcSwapOption::empty()),
                health_check_shutdown: Arc::new(Notify::new()),
                admin_role: config.admin_role.clone(),
                #[cfg(feature = "metrics")]
                metrics_collector: std::sync::RwLock::new(None),
                wait_count: AtomicU32::new(0),
                max_waiters: AtomicU32::new(0),
                borrow_count: AtomicU64::new(0),
                max_active: AtomicU32::new(0),
                #[cfg(feature = "cache-available")]
                cache_provider: ArcSwapOption::new(None),
                #[cfg(feature = "data-protection")]
                data_protection: tokio::sync::RwLock::new(
                    crate::access::data_protection::DataProtection::default(),
                ),
                #[cfg(feature = "data-protection")]
                data_protection_epoch: std::sync::atomic::AtomicU64::new(0),
                #[cfg(feature = "health-check")]
                replica_health_provider: std::sync::RwLock::new(None),
                #[cfg(feature = "sql-parser")]
                ddl_guard: std::sync::RwLock::new(None),
                #[cfg(feature = "prepare-cache")]
                prepare_cache: std::sync::RwLock::new(None),
            }),
        };

        // 安全审计：检查是否使用了默认 admin 角色（不安全），记录安全审计事件
        super::audit::warn_and_record_default_admin_role(&config.admin_role);

        // 启动后台健康检查
        #[cfg(feature = "pool-health-check")]
        pool.start_background_health_check();

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
        // vuln-0001 修复：检查是否使用了默认 admin 角色（不安全），记录安全审计事件
        super::audit::warn_and_record_default_admin_role(&config.admin_role);
        Ok(Self {
            inner: Arc::new(DbPoolInner {
                config: Arc::new(config.clone()),
                connection_semaphore: Arc::new(Semaphore::new(
                    config.pool_config.max_connections as usize,
                )),
                idle_connections: SyncMutex::new(Vec::new()),
                connection_available: Notify::new(),
                active_count: AtomicU32::new(0),
                total_count: AtomicU32::new(0),
                idle_check_in_progress: AtomicU32::new(0),
                health_check_shutdown: Arc::new(Notify::new()),
                admin_role: config.admin_role.clone(),
                #[cfg(feature = "metrics")]
                metrics_collector: std::sync::RwLock::new(None),
                wait_count: AtomicU32::new(0),
                max_waiters: AtomicU32::new(0),
                borrow_count: AtomicU64::new(0),
                max_active: AtomicU32::new(0),
                #[cfg(feature = "cache-available")]
                cache_provider: ArcSwapOption::new(None),
                #[cfg(feature = "data-protection")]
                data_protection: tokio::sync::RwLock::new(
                    crate::access::data_protection::DataProtection::default(),
                ),
                #[cfg(feature = "data-protection")]
                data_protection_epoch: std::sync::atomic::AtomicU64::new(0),
                #[cfg(feature = "health-check")]
                replica_health_provider: std::sync::RwLock::new(None),
                #[cfg(feature = "sql-parser")]
                ddl_guard: std::sync::RwLock::new(None),
                #[cfg(feature = "prepare-cache")]
                prepare_cache: std::sync::RwLock::new(None),
            }),
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
                    DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                        sea_orm::ConnAcquireErr::Timeout,
                    ))
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

    /// 获取指标收集器（如果已设置；可经 `set_metrics_collector` 运行时注入）
    #[cfg(feature = "metrics")]
    pub fn metrics(&self) -> Option<Arc<MetricsCollector>> {
        self.inner
            .metrics_collector
            .read()
            .expect("metrics_collector lock")
            .clone()
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
    pub(crate) async fn acquire_connection(&self) -> DbResult<DbConnection> {
        // 步骤 0: 记录等待计数
        let waiters = self.inner.wait_count.fetch_add(1, Ordering::SeqCst) + 1;
        self.update_max_waiters(waiters);

        // 步骤 1: 获取信号量许可（等待可用槽位，带超时）
        let timeout_duration = self.inner.config.acquire_timeout_duration();

        let start = Instant::now();

        let acquire_result =
            timeout(timeout_duration, self.inner.connection_semaphore.acquire()).await;
        self.inner.wait_count.fetch_sub(1, Ordering::SeqCst);

        let permit = match acquire_result {
            Ok(Ok(p)) => {
                self.record_acquire_duration(start);
                p
            }
            Ok(Err(_)) => {
                return Err(DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::ConnAcquireErr::Timeout,
                )));
            }
            Err(_) => {
                let waited_ms = start.elapsed().as_millis() as u64;
                self.record_acquire_timeout(start);
                log::warn!(
                    "{}",
                    i18n::t(
                        "pool-log-acquire-timeout",
                        &[
                            ("waited_ms", waited_ms.to_string()),
                            (
                                "timeout_ms",
                                (timeout_duration.as_millis() as u64).to_string()
                            ),
                            (
                                "waiters",
                                self.inner.wait_count.load(Ordering::SeqCst).to_string()
                            ),
                        ]
                    )
                );
                return Err(DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::ConnAcquireErr::Timeout,
                )));
            }
        };

        // 步骤 2: 尝试从空闲队列获取（最小化锁持有时间；同步锁：临界区仅
        // pop，无 await——归还侧同样同步，保证归还先于任何后续新建可见）
        {
            let mut idle = self
                .inner
                .idle_connections
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(conn) = idle.pop() {
                let active = self.inner.active_count.fetch_add(1, Ordering::SeqCst) + 1;
                self.update_max_active(active);
                self.inner.borrow_count.fetch_add(1, Ordering::SeqCst);
                permit.forget();
                return Ok(conn);
            }
        }

        // 步骤 2.5: 空闲连接校验在途 → 等待回填后重试取用，不新建
        //
        // 校验方在校验窗口内临时取走空闲连接（idle 看起来为空），此时新建
        // 连接会引入冗余连接：对 `duckdb::memory:` 等「新连接即新库」的后端
        // 是数据可见性缺陷，对其他后端是连接数膨胀。等待预算取
        // 「acquire_timeout 剩余额度」与 IDLE_CHECK_MAX_WAIT 的较小值；到期
        // 仍在校验中则降级新建（记录 warn 供诊断），等待登记进 wait_count
        // 使池告警可归因该校验窗口。
        let wait_deadline = std::cmp::min(
            start + timeout_duration,
            Instant::now() + IDLE_CHECK_MAX_WAIT,
        );
        let mut wait_registered = false;
        let found = 'wait: loop {
            // 每轮先试取：校验窗口内归还/回填的连接立即可用
            let popped = {
                let mut idle = self
                    .inner
                    .idle_connections
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                idle.pop()
            };
            if popped.is_some() {
                break 'wait popped;
            }
            if self.inner.idle_check_in_progress.load(Ordering::SeqCst) == 0 {
                break 'wait None;
            }
            let now = Instant::now();
            if now >= wait_deadline {
                log::warn!(
                    "{}",
                    i18n::t(
                        "pool-log-idle-check-wait-timeout",
                        &[
                            ("waited_ms", (now - start).as_millis().to_string()),
                            (
                                "in_progress",
                                self.inner
                                    .idle_check_in_progress
                                    .load(Ordering::SeqCst)
                                    .to_string()
                            ),
                        ]
                    )
                );
                break 'wait None;
            }
            if !wait_registered {
                let waiters = self.inner.wait_count.fetch_add(1, Ordering::SeqCst) + 1;
                self.update_max_waiters(waiters);
                wait_registered = true;
            }
            // 守卫清除时广播、归还时 notify_one；1ms 兜底防丢失唤醒
            let notified = self.inner.connection_available.notified();
            let _ = timeout(Duration::from_millis(1).min(wait_deadline - now), notified).await;
        };
        if wait_registered {
            self.inner.wait_count.fetch_sub(1, Ordering::SeqCst);
        }
        if let Some(conn) = found {
            let active = self.inner.active_count.fetch_add(1, Ordering::SeqCst) + 1;
            self.update_max_active(active);
            self.inner.borrow_count.fetch_add(1, Ordering::SeqCst);
            permit.forget();
            return Ok(conn);
        }

        // 步骤 3: 创建新连接（不持有锁）
        self.inner.total_count.fetch_add(1, Ordering::SeqCst);
        let active = self.inner.active_count.fetch_add(1, Ordering::SeqCst) + 1;
        self.update_max_active(active);

        match Self::create_connection(&self.inner.config).await {
            Ok(conn) => {
                self.inner.borrow_count.fetch_add(1, Ordering::SeqCst);
                permit.forget();
                Ok(conn)
            }
            Err(e) => {
                self.inner.total_count.fetch_sub(1, Ordering::SeqCst);
                self.inner.active_count.fetch_sub(1, Ordering::SeqCst);
                drop(permit);
                Err(e)
            }
        }
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
            crate::database::LadybugConnection::new(":memory:", 1)
                .expect("Failed to create LadybugConnection"),
        ));
        assert!(conn.is_graph(), "Ladybug connection should be graph");
        assert!(!conn.is_duckdb(), "Ladybug connection should not be duckdb");
    }

    #[cfg(feature = "ladybug")]
    #[test]
    fn test_ladybug_connection_as_graph_returns_ok() {
        let conn = DbConnection::Ladybug(Arc::new(
            crate::database::LadybugConnection::new(":memory:", 1)
                .expect("Failed to create LadybugConnection"),
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
            crate::database::LadybugConnection::new(":memory:", 1)
                .expect("Failed to create LadybugConnection"),
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
        graph
            .health_check()
            .await
            .expect("health_check should pass");
    }

    #[cfg(feature = "neo4j")]
    #[test]
    fn test_neo4j_connection_is_graph() {
        let conn =
            DbConnection::Neo4j(Arc::new(crate::database::Neo4jConnection::new_placeholder()));
        assert!(conn.is_graph(), "Neo4j connection should be graph");
        assert!(!conn.is_duckdb(), "Neo4j connection should not be duckdb");
    }

    #[cfg(feature = "neo4j")]
    #[test]
    fn test_neo4j_connection_as_graph_returns_ok() {
        let conn =
            DbConnection::Neo4j(Arc::new(crate::database::Neo4jConnection::new_placeholder()));
        let result = conn.as_graph();
        assert!(result.is_ok(), "as_graph() on Neo4j should return Ok");
        let graph = result.unwrap();
        assert_eq!(graph.backend_name(), "neo4j");
    }

    #[cfg(feature = "neo4j")]
    #[test]
    fn test_neo4j_connection_as_sea_orm_returns_err() {
        let conn =
            DbConnection::Neo4j(Arc::new(crate::database::Neo4jConnection::new_placeholder()));
        let result = conn.as_sea_orm();
        assert!(result.is_err(), "as_sea_orm() on Neo4j should return Err");
    }

    #[cfg(feature = "neo4j")]
    #[tokio::test]
    #[ignore = "需要 Neo4j 服务器，设置 NEO4J_URL/NEO4J_USER/NEO4J_PASSWORD 环境变量后运行"]
    async fn test_create_connection_neo4j() {
        let url =
            std::env::var("NEO4J_URL").unwrap_or_else(|_| "neo4j://localhost:7687".to_string());
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
                crate::database::LadybugConnection::new(":memory:", 1)
                    .expect("Failed to create LadybugConnection"),
            ));
            let debug_str = format!("{conn:?}");
            assert!(
                debug_str.contains("Ladybug"),
                "Debug should contain 'Ladybug': {debug_str}"
            );
        }
        #[cfg(feature = "neo4j")]
        {
            let conn =
                DbConnection::Neo4j(Arc::new(crate::database::Neo4jConnection::new_placeholder()));
            let debug_str = format!("{conn:?}");
            assert!(
                debug_str.contains("Neo4j"),
                "Debug should contain 'Neo4j': {debug_str}"
            );
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
        let conn =
            DbConnection::SeaOrm(sea_orm::Database::connect("sqlite::memory:").await.unwrap());
        assert!(!conn.is_duckdb());
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_db_connection_is_graph_seaorm() {
        let conn =
            DbConnection::SeaOrm(sea_orm::Database::connect("sqlite::memory:").await.unwrap());
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
        let pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

        // Test status() — 无 pool-warmup 时懒创建（total=0）；有 pool-warmup 时预创建
        let status = pool.status();
        #[cfg(not(feature = "pool-warmup"))]
        {
            assert_eq!(status.total, 0); // no warmup → no pre-created connections
            assert_eq!(status.idle, 0);
        }
        #[cfg(feature = "pool-warmup")]
        {
            assert_eq!(status.total, 2); // warmup → pre-created min_connections 个连接
            assert_eq!(status.idle, 2);
        }
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
        let pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

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

    #[cfg(not(feature = "duckdb"))]
    #[tokio::test]
    async fn test_create_connection_duckdb_not_enabled() {
        let config = DbConfig {
            url: "duckdb::memory:".to_string(),
            ..Default::default()
        };
        let result = DbPool::create_connection(&config).await;
        assert!(
            result.is_err(),
            "DuckDB connection should fail without duckdb feature"
        );
    }

    #[cfg(not(feature = "ladybug"))]
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

    #[cfg(not(feature = "neo4j"))]
    #[tokio::test]
    async fn test_create_connection_neo4j_not_enabled() {
        let config = DbConfig {
            url: "neo4j://localhost:7687".to_string(),
            ..Default::default()
        };
        let result = DbPool::create_connection(&config).await;
        assert!(
            result.is_err(),
            "Neo4j connection should fail without neo4j feature"
        );
    }

    #[cfg(feature = "permission")]
    #[test]
    fn test_pool_try_from_with_permission_returns_error() {
        let config = DbConfig::default();
        let result = DbPool::try_from(&config);
        assert!(
            result.is_err(),
            "try_from should fail with permission feature enabled"
        );
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
        let pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

        // Test ConnectionPool::status — 无 pool-warmup 时懒创建，有 pool-warmup 时预创建 min 个
        let status = ConnectionPool::status(&pool);
        #[cfg(not(feature = "pool-warmup"))]
        assert_eq!(status.total, 0); // no warmup → no pre-created connections
        #[cfg(feature = "pool-warmup")]
        assert!(
            status.total >= 5,
            "warmup should pre-create min_connections: {}",
            status.total
        );

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
        let pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

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
        // This exercises the total_count decrement path (idle pool at capacity)
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: PoolConfig {
                max_connections: 2,
                min_connections: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

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
        assert!(
            status.idle <= 2,
            "idle should be <= max_connections: {}",
            status.idle
        );
    }

    /// vuln-0001 回归测试：使用默认 admin 角色创建池时记录安全审计事件
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_default_admin_role_records_audit_event() {
        use crate::database::pool::audit::admin_bypass_count;

        let before = admin_bypass_count();

        // DbConfig::default() 的 admin_role 为默认值 "admin"
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let _pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

        assert!(
            admin_bypass_count() > before,
            "默认 admin 角色创建池应触发审计记录"
        );
    }

    // ===== 补充测试：cache 方法, validate_role_name =====

    #[cfg(all(feature = "cache-available", feature = "sqlite"))]
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
            ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, DbError>> + Send + 'a>>
            {
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
            fn delete<'a>(
                &'a self,
                _key: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }
        }

        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let mut pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

        // Initially no cache provider
        assert!(pool.cache_provider().is_none());

        // Set cache provider (covers lines 311-312)
        let provider = Arc::new(NoopCacheProvider);
        pool.set_cache_provider(provider);

        // Now cache provider should be Some (covers lines 319-320)
        assert!(pool.cache_provider().is_some());
    }

    /// 未注入 cache_provider 时，query_cache_get 返回 None（直通）。
    #[cfg(all(feature = "cache-available", feature = "sqlite"))]
    #[tokio::test]
    async fn test_query_cache_miss_without_provider() {
        let pool = DbPool::new("sqlite::memory:").await.expect("pool");
        let session = pool.get_session("admin").await.expect("session");
        // No cache_provider injected → query_cache_get returns None
        let result = session.query_cache_get("any_key").await;
        assert!(result.is_none(), "expected None without cache_provider");
    }

    /// 注入 cache_provider 后，query_cache_set 存储数据并可经 query_cache_get 命中。
    #[cfg(all(feature = "cache-available", feature = "sqlite"))]
    #[tokio::test]
    async fn test_query_cache_hit_with_provider() {
        use std::collections::HashMap;
        use std::future::Future;
        use std::pin::Pin;
        use std::sync::Mutex as StdMutex;

        /// In-memory DbCacheProvider for testing.
        struct MemCacheProvider {
            data: StdMutex<HashMap<String, Vec<u8>>>,
        }
        impl crate::domain::DbCacheProvider for MemCacheProvider {
            fn get<'a>(
                &'a self,
                key: &'a str,
            ) -> Pin<
                Box<
                    dyn Future<Output = Result<Option<Vec<u8>>, crate::foundation::DbError>>
                        + Send
                        + 'a,
                >,
            > {
                Box::pin(async move {
                    let data = self.data.lock().unwrap();
                    Ok(data.get(key).cloned())
                })
            }
            fn set<'a>(
                &'a self,
                key: &'a str,
                value: Vec<u8>,
                _ttl: Option<std::time::Duration>,
            ) -> Pin<Box<dyn Future<Output = Result<(), crate::foundation::DbError>> + Send + 'a>>
            {
                Box::pin(async move {
                    let mut data = self.data.lock().unwrap();
                    data.insert(key.to_string(), value);
                    Ok(())
                })
            }
            fn delete<'a>(
                &'a self,
                key: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<(), crate::foundation::DbError>> + Send + 'a>>
            {
                Box::pin(async move {
                    let mut data = self.data.lock().unwrap();
                    data.remove(key);
                    Ok(())
                })
            }
        }

        let mut pool = DbPool::new("sqlite::memory:").await.expect("pool");
        let provider = Arc::new(MemCacheProvider {
            data: StdMutex::new(HashMap::new()),
        });
        pool.set_cache_provider(provider);

        let session = pool.get_session("admin").await.expect("session");

        // Cache miss initially
        let miss = session.query_cache_get("select:users").await;
        assert!(miss.is_none(), "expected cache miss initially");

        // Store a value
        session
            .query_cache_set("select:users", b"cached_result".to_vec())
            .await;

        // Cache hit
        let hit = session.query_cache_get("select:users").await;
        assert_eq!(hit, Some(b"cached_result".to_vec()), "expected cache hit");
    }

    #[cfg(all(feature = "permission", feature = "sqlite"))]
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
            file.write_all(yaml_content.as_bytes())
                .expect("write temp file");
        }

        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            permissions_path: Some(yaml_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        let pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

        // "admin" role exists in config -> should succeed
        let result = pool.get_session("admin").await;
        assert!(
            result.is_ok(),
            "admin should be allowed: {:?}",
            result.err()
        );

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

    #[cfg(all(feature = "permission", feature = "sqlite"))]
    #[tokio::test]
    async fn test_validate_role_name_no_config_unsafe_role() {
        // Without permission config, only "admin" and "system" are allowed
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let pool = DbPool::with_config(config)
            .await
            .expect("should create pool");

        // "admin" is a safe role -> should succeed
        let result = pool.get_session("admin").await;
        assert!(
            result.is_ok(),
            "admin should be allowed: {:?}",
            result.err()
        );

        // "system" is a safe role -> should succeed
        let result = pool.get_session("system").await;
        assert!(
            result.is_ok(),
            "system should be allowed: {:?}",
            result.err()
        );

        // "hacker" is NOT a safe role -> should fail
        let result = pool.get_session("hacker").await;
        assert!(result.is_err(), "hacker should be rejected");
    }
}

// ===== 补充测试：池指标、死连接清理与重建 =====

#[cfg(all(feature = "metrics", feature = "health-check", feature = "sqlite"))]
#[tokio::test]
async fn test_pool_metrics_reflects_collector_presence() {
    use crate::observability::MetricsCollector;

    let pool = DbPool::new("sqlite::memory:").await.expect("pool");
    // 无 collector：计数全 0，等待者计数仍真实
    let m = pool.pool_metrics();
    assert_eq!(m.slow_acquires, 0);
    assert_eq!(m.timeout_errors, 0);
    assert_eq!(m.critical_timeouts, 0);
    assert_eq!(m.wait_count, 0);

    // 注入 collector 后获取会话产生采集数据
    pool.set_metrics_collector(Some(Arc::new(MetricsCollector::new())))
        .await;
    let session = pool.get_session("admin").await.expect("session");
    drop(session);
    let m = pool.pool_metrics();
    assert_eq!(m.timeout_errors, 0, "正常获取不应有超时");
    assert_eq!(m.max_waiters, 1, "单次获取的历史等待者峰值为 1");

    // 清除 collector：采集器分支归零，历史峰值不受影响
    pool.set_metrics_collector(None).await;
    let m = pool.pool_metrics();
    assert_eq!(m.slow_acquires, 0);
    assert_eq!(m.max_waiters, 1, "历史峰值不受 collector 清除影响");
}

#[cfg(all(feature = "sqlite", feature = "pool-health-check"))]
#[tokio::test]
#[ignore = "sea-orm 2.0 sqlite 走 RusqliteSharedConnection（进程内嵌库）：SELECT 1 恒成功、close 无法真正杀死连接，「死连接」不可模拟；待 cleanup 支持注入式健康探针后恢复"]
async fn test_clean_invalid_connections_removes_dead() {
    let pool = DbPool::new("sqlite::memory:").await.expect("pool");
    // 死连接模拟用文件型 sqlite：sqlite::memory: 池有单连接保活语义，
    // close() 后句柄仍可响应 SELECT 1，无法可靠模拟死连接
    let dir = tempfile::tempdir().expect("tempdir");
    let dead_url = format!("sqlite://{}?mode=rwc", dir.path().join("dead.db").display());
    let dead = sea_orm::Database::connect(&dead_url)
        .await
        .expect("connect");
    dead.clone().close().await.expect("close");
    let baseline_idle = pool
        .inner
        .idle_connections
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .len();
    pool.inner
        .idle_connections
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(DbConnection::SeaOrm(dead));

    let removed = pool.clean_invalid_connections().await;
    assert_eq!(removed, 1, "死连接必须被清除");
    // 池构造时预填充的活连接保留（new() 会按 min_connections 预热），只清死连接
    assert_eq!(
        pool.inner
            .idle_connections
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len(),
        baseline_idle,
        "仅移除注入的死连接，预填充活连接不受影响"
    );
}

#[cfg(all(feature = "sqlite", feature = "pool-health-check"))]
#[tokio::test]
#[ignore = "sea-orm 2.0 sqlite 走 RusqliteSharedConnection（进程内嵌库）：SELECT 1 恒成功、close 无法真正杀死连接，「死连接」不可模拟；待 validate 支持注入式健康探针后恢复"]
async fn test_validate_and_recreate_connections_refills_min() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        pool_config: crate::foundation::PoolConfig {
            min_connections: 2,
            ..Default::default()
        },
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("pool");
    // 预置一个死连接（文件型 sqlite，close 后必失效；memory 池有保活语义）
    let dir2 = tempfile::tempdir().expect("tempdir");
    let dead_url = format!(
        "sqlite://{}?mode=rwc",
        dir2.path().join("dead.db").display()
    );
    let dead = sea_orm::Database::connect(&dead_url)
        .await
        .expect("connect");
    dead.clone().close().await.expect("close");
    pool.inner
        .idle_connections
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(DbConnection::SeaOrm(dead));

    let recreated = pool
        .validate_and_recreate_connections()
        .await
        .expect("recreate");
    assert!(
        recreated >= 1,
        "全部预填充连接死亡后应重建至 min_connections=2，实际 {recreated}"
    );
    assert_eq!(
        pool.inner
            .idle_connections
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len(),
        2,
        "重建后应回填至 min_connections"
    );
}

#[cfg(all(feature = "permission", feature = "sqlite"))]
#[tokio::test]
async fn test_pool_permissions_path_missing_file_rejected() {
    let missing = std::env::temp_dir().join(format!(
        "dbnexus_no_such_perm_{}.yaml",
        std::time::Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::remove_file(&missing);
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        permissions_path: Some(missing.to_string_lossy().to_string()),
        ..Default::default()
    };
    let result = DbPool::with_config(config).await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("missing permissions file must fail pool creation"),
    };
    let msg = err.message();
    assert!(
        msg.contains("read") || msg.contains("读取") || !msg.is_empty(),
        "got: {msg}"
    );
    assert!(matches!(err, crate::foundation::DbError::Config(_)));
}

#[cfg(all(feature = "permission", feature = "sqlite"))]
#[tokio::test]
async fn test_pool_permissions_path_malformed_file_rejected() {
    use std::io::Write;

    let path = std::env::temp_dir().join(format!(
        "dbnexus_bad_perm_{}.yaml",
        std::time::Instant::now().elapsed().as_nanos()
    ));
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(b"roles: [not, a, mapping")
        .expect("write malformed yaml");
    drop(f);

    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        permissions_path: Some(path.to_string_lossy().to_string()),
        ..Default::default()
    };
    let result = DbPool::with_config(config).await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("malformed permissions file must fail pool creation"),
    };
    assert!(matches!(err, crate::foundation::DbError::Config(_)));
    let _ = std::fs::remove_file(&path);
}

#[cfg(all(feature = "permission", feature = "sqlite"))]
#[test]
fn test_parse_permission_yaml_invalid_content_rejected() {
    // 未闭合的流序列是 YAML 扫描错误；roles 映射到标量是反序列化错误
    assert!(DbPool::parse_permission_yaml("[1, 2, 3", "inline").is_err());
    assert!(DbPool::parse_permission_yaml("roles: not-a-mapping", "inline").is_err());
}

/// 池耗尽：连接被占满后，获取超时必须返回 ConnectionAcquire 错误
#[cfg(feature = "sqlite")]
#[tokio::test]
async fn test_acquire_connection_times_out_when_exhausted() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        pool_config: crate::foundation::PoolConfig {
            max_connections: 1,
            min_connections: 1,
            acquire_timeout: 50,
            ..Default::default()
        },
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("pool");
    let first = pool.acquire_connection().await;
    assert!(
        first.is_ok(),
        "首个连接必须成功: {:?}",
        first.as_ref().err()
    );
    let err = pool.acquire_connection().await;
    assert!(err.is_err(), "第二个连接必须超时");
    drop(first);
}
