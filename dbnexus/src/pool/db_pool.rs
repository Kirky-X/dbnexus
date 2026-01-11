// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池管理模块
//!
//! 提供数据库连接池的创建、管理和自动修正功能

use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as AsyncMutex, Notify};
use tokio::time::{interval, timeout};
use tracing::info;

use crate::config::{ConfigError, DbConfig, DbError, DbResult};
#[cfg(feature = "metrics")]
use crate::metrics::MetricsCollector;
use crate::permission::{PermissionConfig, RolePolicy};
use crate::sql_parser::{SqlParser, is_ddl_operation};
use super::Session;

// 导入 Sea-ORM 的事务 trait 和连接 trait
use sea_orm::ConnectionTrait;
use sea_orm::TransactionTrait;

/// 数据库连接类型
pub type DatabaseConnection = sea_orm::DatabaseConnection;

/// 连接池管理器
#[derive(Clone)]
pub struct DbPool {
    /// 内部连接池
    inner: Arc<DbPoolInner>,
}

pub(crate) struct DbPoolInner {
    /// 配置
    pub(crate) config: DbConfig,

    /// 空闲连接队列
    idle_connections: AsyncMutex<Vec<DatabaseConnection>>,

    /// 连接可用通知（替代忙等待）
    connection_available: Notify,

    /// 活跃连接数
    pub(crate) active_count: AtomicU32,

    /// 总连接数
    pub(crate) total_count: AtomicU32,

    /// 权限策略 LRU 缓存（使用 tokio 异步锁）
    pub(crate) policy_cache: Arc<AsyncMutex<LruCache<String, RolePolicy>>>,

    /// 权限配置（懒加载，使用 tokio 异步锁）
    permission_config: Arc<AsyncMutex<Option<PermissionConfig>>>,

    /// 后台健康检查任务（用于优雅关闭）
    health_check_shutdown: Arc<Notify>,

    /// 管理员角色名称
    pub(crate) admin_role: String,

    /// 指标收集器（可选，用于 metrics 特性）
    #[cfg(feature = "metrics")]
    pub(crate) metrics_collector: Option<Arc<MetricsCollector>>,
}

impl DbPool {
    /// 创建新的连接池
    pub async fn new(url: &str) -> DbResult<Self> {
        let config = DbConfig {
            url: url.to_string(),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// 使用配置创建连接池（带自动修正）
    pub async fn with_config(config: DbConfig) -> DbResult<Self> {
        // 使用配置修正器自动修正配置
        let corrected_config = crate::config::ConfigCorrector::auto_correct(config);

        // 创建初始连接以查询数据库能力
        let db_type = crate::config::DatabaseType::parse_database_type(&corrected_config.url);

        // 创建连接并应用数据库能力修正
        let connection = sea_orm::Database::connect(&corrected_config.url)
            .await
            .map_err(DbError::Connection)?;

        // 应用数据库能力修正（如果需要）
        let corrected_config = crate::config::ConfigCorrector::auto_correct_with_database_capability(
            corrected_config,
            &connection,
            db_type, // DatabaseType implements Copy, no need to clone
        )
        .await;

        // 输出配置修正信息
        if corrected_config.max_connections < 100 && db_type.is_real_database() {
            info!(
                "Database connection limit: 80% of {} = {} connections",
                corrected_config.max_connections, corrected_config.max_connections
            );
        }

        let policy_cache = Arc::new(AsyncMutex::new(LruCache::new(
            NonZeroUsize::new(4096).expect("LRU cache size must be non-zero"),
        )));

        // 加载权限配置（如果指定了路径）
        let permission_config = Self::load_permission_config(&corrected_config).await;

        let pool = Self {
            inner: Arc::new(DbPoolInner {
                config: corrected_config.clone(),
                idle_connections: AsyncMutex::new(Vec::new()),
                connection_available: Notify::new(),
                active_count: AtomicU32::new(0),
                total_count: AtomicU32::new(0),
                policy_cache,
                permission_config: Arc::new(AsyncMutex::new(permission_config)),
                health_check_shutdown: Arc::new(Notify::new()),
                admin_role: corrected_config.admin_role.clone(),
                #[cfg(feature = "metrics")]
                metrics_collector: None,
            }),
        };

        // 启动后台健康检查任务
        pool.start_background_health_check();

        // 预创建最小连接数（并行创建以提高启动速度）
        let initial_connections = pool.inner.config.min_connections;
        let mut connection_tasks = Vec::new();

        for _ in 0..initial_connections {
            let config = corrected_config.clone();
            connection_tasks.push(async move { Self::create_connection(&config).await });
        }

        // 并行执行所有连接创建任务
        let results = futures::future::join_all(connection_tasks).await;

        for result in results {
            match result {
                Ok(conn) => {
                    pool.inner.idle_connections.lock().await.push(conn);
                    pool.inner.total_count.fetch_add(1, Ordering::SeqCst);
                }
                Err(e) => {
                    tracing::error!("Failed to create initial connection: {}", e);
                    // 继续创建其他连接，即使有部分失败
                }
            }
        }

        info!(
            "Connection pool initialized: {} connections (min: {}, max: {})",
            initial_connections, corrected_config.min_connections, corrected_config.max_connections
        );

        // 加载权限策略到缓存
        let permission_config_guard = pool.inner.permission_config.lock().await;

        if let Some(ref config) = *permission_config_guard {
            let mut cache = pool.inner.policy_cache.lock().await;
            for (role, policy) in &config.roles {
                cache.put(role.clone(), policy.clone());
            }
            info!("Loaded permission policies for {} roles", config.roles.len());
        }
        drop(permission_config_guard);

        #[cfg(feature = "auto-migrate")]
        if corrected_config.auto_migrate {
            if let Some(ref migrations_dir) = corrected_config.migrations_dir {
                if migrations_dir.exists() {
                    info!(
                        "Auto-migrate enabled, running migrations from: {}",
                        migrations_dir.display()
                    );
                    let applied = pool.run_migrations(migrations_dir).await?;
                    info!("Auto-migrate completed: {} migrations applied", applied);
                } else {
                    tracing::warn!(
                        "Auto-migrate enabled but migrations directory does not exist: {}",
                        migrations_dir.display()
                    );
                }
            } else {
                tracing::warn!("Auto-migrate enabled but migrations_dir not configured");
            }
        }

        Ok(pool)
    }

    /// 使用配置结构体创建连接池
    ///
    /// 此方法接受一个 [`DbConfig`] 结构体，用于配置连接池的所有参数。
    /// 与 [`with_config`] 方法功能相同，但更适合从配置结构体直接初始化。
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::{DbPool, DbConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = DbConfig {
    ///         url: "sqlite::memory:".to_string(),
    ///         max_connections: 10,
    ///         min_connections: 2,
    ///         ..Default::default()
    ///     };
    ///
    ///     let pool = DbPool::try_from_config(config).await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// 如果连接失败或配置无效，返回错误
    pub async fn try_from_config(config: DbConfig) -> DbResult<Self> {
        Self::with_config(config).await
    }

    /// 使用配置引用同步创建连接池
    ///
    /// 此方法是同步的，不会创建数据库连接。
    /// 实际的连接池创建和连接验证在首次获取连接时进行。
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::{DbPool, DbConfig};
    ///
    /// let config = DbConfig {
    ///     url: "sqlite::memory:".to_string(),
    ///     max_connections: 10,
    ///     min_connections: 1,
    ///     idle_timeout: 300,
    ///     acquire_timeout: 5000,
    ///     permissions_path: None,
    ///     migrations_dir: None,
    ///     auto_migrate: false,
    ///     migration_timeout: 60,
    ///     admin_role: "admin".to_string(),
    /// };
    ///
    /// let pool = DbPool::try_from(&config)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// 如果配置验证失败，返回错误
    pub fn try_from(config: &DbConfig) -> Result<Self, ConfigError> {
        config.validate()?;
        Ok(Self {
            inner: Arc::new(DbPoolInner {
                config: config.clone(),
                idle_connections: AsyncMutex::new(Vec::new()),
                connection_available: Notify::new(),
                active_count: AtomicU32::new(0),
                total_count: AtomicU32::new(0),
                policy_cache: Arc::new(AsyncMutex::new(LruCache::new(
                    NonZeroUsize::new(4096).expect("LRU cache size must be non-zero"),
                ))),
                permission_config: Arc::new(AsyncMutex::new(None)),
                health_check_shutdown: Arc::new(Notify::new()),
                admin_role: config.admin_role.clone(),
                #[cfg(feature = "metrics")]
                metrics_collector: None,
            }),
        })
    }

    /// 加载权限配置文件
    ///
    /// # Returns
    ///
    /// - `Some(PermissionConfig)` - 成功加载的权限配置
    /// - `None` - 没有配置权限文件或加载失败，使用默认的 deny_all 策略
    async fn load_permission_config(config: &DbConfig) -> Option<PermissionConfig> {
        // 尝试从配置文件加载
        if let Some(ref path) = config.permissions_path {
            tracing::info!("Loading permission config from: {}", path);
            match std::fs::read_to_string(path) {
                Ok(content) => match PermissionConfig::from_yaml(&content) {
                    Ok(perm_config) => {
                        tracing::info!("Successfully loaded permission config from: {}", path);
                        return Some(perm_config);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse permission config from '{}': {}", path, e);
                        // 配置文件存在但解析失败，不验证角色
                        return None;
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read permission config from '{}': {}", path, e);
                    // 配置文件存在但读取失败，不验证角色
                    return None;
                }
            }
        }

        // 没有配置权限文件
        tracing::debug!("No permission config path specified");
        None
    }

    /// 获取指标收集器（如果已设置）
    #[cfg(feature = "metrics")]
    pub fn metrics(&self) -> Option<&Arc<MetricsCollector>> {
        self.inner.metrics_collector.as_ref()
    }

    /// 获取当前应用的实际配置
    ///
    /// 返回经过自动修正后的配置副本。
    /// 如果配置从未被修正过，则返回传入的配置。
    ///
    /// # Returns
    ///
    /// 实际应用的配置（可能已被自动修正）
    pub fn get_actual_config(&self) -> DbConfig {
        crate::config::ConfigCorrector::get_actual_config(&self.inner.config)
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
    pub async fn get_session(&self, role: &str) -> DbResult<Session> {
        // 验证角色名称
        self.validate_role_name(role).await?;

        let connection = self.acquire_connection().await?;
        let pool_ref = Arc::new(self.clone());
        #[allow(unused_mut)]
        let mut session = Session::new(connection, pool_ref, self.inner.clone(), role.to_string());

        // 设置 metrics（如果有）
        #[cfg(feature = "metrics")]
        if let Some(ref metrics) = self.inner.metrics_collector {
            session.set_metrics(metrics.clone());
        }

        Ok(session)
    }

    /// 验证角色名称是否在权限配置中定义
    ///
    /// 仅在权限配置文件存在且成功加载时验证角色。
    /// 如果没有配置权限文件，使用 deny_all 策略，不进行角色验证。
    async fn validate_role_name(&self, role: &str) -> DbResult<()> {
        // 获取权限配置锁
        let permission_config = self.inner.permission_config.lock().await;

        // 检查权限配置是否存在（用户是否显式配置了权限文件）
        if permission_config.is_none() {
            // 没有配置权限文件时，使用安全默认策略
            // 只允许预定义的安全角色，防止未授权访问
            let safe_roles = ["admin", "system"];
            if !safe_roles.contains(&role) {
                tracing::warn!(
                    "Role '{}' is not allowed without explicit permission configuration",
                    role
                );
                return Err(DbError::Permission(format!(
                    "Role '{}' is not allowed without explicit permission configuration. Allowed roles: {}",
                    role,
                    safe_roles.join(", ")
                )));
            }
            tracing::debug!(
                "No permission config configured, allowing safe role '{}'",
                role
            );
            return Ok(());
        }

        tracing::debug!("Permission config present, checking role '{}'", role);

        // 检查角色是否存在
        if permission_config
            .as_ref()
            .is_some_and(|c| c.get_role_policy(role).is_none())
        {
            // 角色不存在
            tracing::warn!("Unknown role '{}' requested, falling back to deny_all policy", role);
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
    async fn create_connection(config: &DbConfig) -> DbResult<DatabaseConnection> {
        let conn = sea_orm::Database::connect(&config.url).await?;
        Ok(conn)
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
    pub async fn check_connection_health(&self, conn: &DatabaseConnection) -> bool {
        let health_query = Self::get_health_check_query(&self.inner.config.url);

        // 创建带超时的健康检查
        let result = timeout(
            Duration::from_secs(5),
            conn.execute_raw(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                health_query.to_string(),
            )),
        )
        .await;

        match result {
            Ok(Ok(_)) => {
                tracing::debug!("Connection health check passed");
                true
            }
            Ok(Err(e)) => {
                tracing::warn!("Connection health check failed: {}", e);
                false
            }
            Err(_) => {
                tracing::warn!("Connection health check timed out");
                false
            }
        }
    }

    /// 获取数据库类型
    ///
    /// 根据数据库 URL 的协议部分解析数据库类型。
    /// 支持的数据库类型包括 SQLite、PostgreSQL 和 MySQL。
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
    /// 如果 URL 无法识别，默认返回 SQLite 类型
    fn get_database_backend(url: &str) -> sea_orm::DatabaseBackend {
        if url.starts_with("sqlite:") {
            sea_orm::DatabaseBackend::Sqlite
        } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
            sea_orm::DatabaseBackend::Postgres
        } else if url.starts_with("mysql:") {
            sea_orm::DatabaseBackend::MySql
        } else {
            sea_orm::DatabaseBackend::Sqlite
        }
    }

    /// 获取健康检查查询语句
    ///
    /// 根据数据库类型返回对应的健康检查 SQL 语句。
    /// 所有支持的数据库类型都使用简单的 `SELECT 1` 查询，
    /// 这是一个轻量级的查询，用于验证连接是否仍然有效。
    ///
    /// # Arguments
    ///
    /// * `url` - 数据库连接 URL
    ///
    /// # Returns
    ///
    /// 对应数据库类型的健康检查 SQL 语句
    fn get_health_check_query(url: &str) -> &'static str {
        match Self::get_database_backend(url) {
            sea_orm::DatabaseBackend::Sqlite => "SELECT 1",
            sea_orm::DatabaseBackend::Postgres => "SELECT 1",
            sea_orm::DatabaseBackend::MySql => "SELECT 1",
            // 处理未来可能新增的数据库类型
            _ => "SELECT 1",
        }
    }

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

        let health_query = Self::get_health_check_query(&config.url);
        let backend = Self::get_database_backend(&config.url);
        let mut removed_count = 0;

        // 保留有效连接
        let mut valid_connections: Vec<DatabaseConnection> = Vec::with_capacity(idle.len());

        for conn in idle.drain(..) {
            // 执行健康检查（带超时）
            let is_valid = timeout(
                Duration::from_secs(2),
                conn.execute_raw(sea_orm::Statement::from_string(backend, health_query.to_string())),
            )
            .await
            .is_ok_and(|result| result.is_ok());

            if is_valid {
                valid_connections.push(conn);
            } else {
                removed_count += 1;
            }
        }

        // 重建空闲连接队列
        idle.extend(valid_connections);

        // 更新总连接数
        self.inner.total_count.fetch_sub(removed_count as u32, Ordering::SeqCst);

        if removed_count > 0 {
            tracing::info!(
                "Cleaned {} invalid connections from pool (remaining idle: {})",
                removed_count,
                idle.len()
            );
        }

        removed_count as u32
    }

    /// 验证并重新创建无效连接
    ///
    /// 检查所有空闲连接的健康状态，自动替换无效连接。
    /// 此方法会确保池中至少保持配置的最小连接数。
    ///
    /// # Returns
    ///
    /// 被重新创建的连接数量
    pub async fn validate_and_recreate_connections(&self) -> u32 {
        let mut idle = self.inner.idle_connections.lock().await;
        let config = &self.inner.config;
        let mut recreated_count = 0;

        let health_query = Self::get_health_check_query(&config.url);
        let backend = Self::get_database_backend(&config.url);

        // 手动分区连接为有效和无效两组
        let mut valid_connections: Vec<DatabaseConnection> = Vec::new();
        let mut invalid_connections: Vec<DatabaseConnection> = Vec::new();

        for conn in idle.drain(..) {
            let is_valid = timeout(
                Duration::from_secs(2),
                conn.execute_raw(sea_orm::Statement::from_string(backend, health_query.to_string())),
            )
            .await
            .is_ok_and(|result| result.is_ok());

            if is_valid {
                valid_connections.push(conn);
            } else {
                invalid_connections.push(conn);
            }
        }

        let invalid_count = invalid_connections.len();
        if invalid_count > 0 {
            // 更新总连接数
            self.inner.total_count.fetch_sub(invalid_count as u32, Ordering::SeqCst);

            // 重建空闲队列（只保留有效连接）
            idle.clear();
            idle.extend(valid_connections);

            tracing::warn!("Found {} invalid connections, removed from pool", invalid_count);

            // 重新创建连接以维持最小连接数
            let current_idle = idle.len();
            let needed = config.min_connections.saturating_sub(current_idle as u32) as usize;

            for _ in 0..needed {
                match Self::create_connection(config).await {
                    Ok(new_conn) => {
                        idle.push(new_conn);
                        self.inner.total_count.fetch_add(1, Ordering::SeqCst);
                        recreated_count += 1;
                    }
                    Err(e) => {
                        tracing::error!("Failed to recreate connection: {}", e);
                    }
                }
            }

            if recreated_count > 0 {
                tracing::info!(
                    "Recreated {} connections to maintain minimum pool size",
                    recreated_count
                );
            }
        } else {
            // 没有无效连接，恢复有效连接到池中
            idle.extend(valid_connections);
        }

        recreated_count as u32
    }

    /// 启动后台连接健康检查任务
    ///
    /// 该任务会定期检查所有空闲连接的健康状态，
    /// 自动移除无效连接并重建新连接以维持最小连接数。
    ///
    /// 健康检查间隔默认为 30 秒，可通过环境变量 `DB_HEALTH_CHECK_INTERVAL` 配置（秒）。
    fn start_background_health_check(&self) {
        let pool = self.clone();
        let shutdown = self.inner.health_check_shutdown.clone();

        // 从环境变量获取健康检查间隔，默认为 30 秒
        let interval_secs = std::env::var("DB_HEALTH_CHECK_INTERVAL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        tracing::info!(
            "Starting background health check task with interval: {} seconds",
            interval_secs
        );

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // 执行连接健康检查
                        let recreated = pool.validate_and_recreate_connections().await;
                        if recreated > 0 {
                            tracing::info!(
                                "Background health check: recreated {} connections",
                                recreated
                            );
                        } else {
                            tracing::debug!("Background health check: all connections healthy");
                        }
                    }
                    _ = shutdown.notified() => {
                        tracing::info!("Background health check task shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// 从池中获取连接
    ///
    /// 实现连接获取逻辑，包括：
    /// 1. 尝试从空闲连接队列获取
    /// 2. 如果队列为空且未达到最大连接数，创建新连接
    /// 3. 如果已达到最大连接数，等待其他连接释放（带超时）
    ///
    /// 使用异步条件变量（Notify）实现高效的等待机制，避免忙等待。
    ///
    /// # Returns
    ///
    /// 成功获取的数据库连接
    ///
    /// # Errors
    ///
    /// 如果获取连接超时或创建连接失败，返回错误
    async fn acquire_connection(&self) -> DbResult<DatabaseConnection> {
        // 使用锁保护整个连接获取流程，避免竞争条件
        let mut idle = self.inner.idle_connections.lock().await;

        // 尝试从空闲队列获取
        if !idle.is_empty() {
            self.inner.active_count.fetch_add(1, Ordering::SeqCst);
            return idle.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::ConnectionAcquire(sea_orm::ConnAcquireErr::Timeout))
            });
        }

        // 检查是否达到最大连接数（在持有锁的情况下）
        if self.inner.total_count.load(Ordering::SeqCst) >= self.inner.config.max_connections {
            // 等待空闲连接（使用条件变量替代忙等待）
            let timeout_duration = self.inner.config.acquire_timeout_duration();

            // 释放锁并等待通知
            drop(idle);

            let result = timeout(timeout_duration, async {
                let mut idle = self.inner.idle_connections.lock().await;
                while idle.is_empty() {
                    // 释放锁并等待通知
                    drop(idle);
                    self.inner.connection_available.notified().await;
                    idle = self.inner.idle_connections.lock().await;
                }
                idle.pop()
            })
            .await;

            return match result {
                Ok(Some(conn)) => {
                    self.inner.active_count.fetch_add(1, Ordering::SeqCst);
                    Ok(conn)
                }
                Ok(None) => Err(DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::ConnAcquireErr::Timeout,
                ))),
                Err(_) => Err(DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                    sea_orm::ConnAcquireErr::Timeout,
                ))),
            };
        }

        // 创建新连接（在持有锁的情况下，确保不会超过最大连接数）
        // 先增加 total_count，确保原子性
        self.inner.total_count.fetch_add(1, Ordering::SeqCst);
        self.inner.active_count.fetch_add(1, Ordering::SeqCst);

        // 释放锁后再创建连接（避免阻塞其他操作）
        drop(idle);

        Self::create_connection(&self.inner.config).await
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
    /// 此方法是异步的，使用 tokio::spawn 在后台执行，
    /// 避免阻塞调用者。
    #[allow(dead_code)]
    pub(crate) fn release_connection(&self, conn: DatabaseConnection) {
        self.inner.active_count.fetch_sub(1, Ordering::SeqCst);
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let mut idle = inner.idle_connections.lock().await;
            if idle.len() < inner.config.max_connections as usize {
                idle.push(conn);
                // 通知等待的请求者有新连接可用
                inner.connection_available.notify_one();
            }
        });
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
        PoolStatus {
            total,
            active,
            idle: total.saturating_sub(active),
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
    /// println!("Max connections: {}", config.max_connections);
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
            tracing::info!("Running auto-migrate from directory: {}", migrations_dir.display());
            self.run_migrations(migrations_dir).await
        } else {
            tracing::warn!("Auto-migrate enabled but migrations_dir not configured");
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
        use crate::migration::{DatabaseType, MigrationExecutor};

        let db_type = DatabaseType::parse_database_type(&self.inner.config.url);

        // 获取一个连接来执行迁移
        let connection = self.acquire_connection().await?;

        // 克隆连接，因为执行器需要拥有连接
        let connection_for_migration = connection.clone();

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
        tracing::info!("DbPool dropped, shutdown signal sent to background health check task");
    }
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
}

