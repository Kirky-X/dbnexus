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

use crate::config::{DbConfig, DbError, DbResult};
#[cfg(feature = "metrics")]
use crate::metrics::MetricsCollector;
use crate::permission::{PermissionAction, PermissionConfig, PermissionContext, RolePolicy};

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
        #[allow(unused_mut)]
        let mut session = Session::new(connection, self.inner.clone(), role.to_string());

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
            // 没有配置权限文件，使用 deny_all 策略，不进行角色验证
            tracing::debug!("No permission config configured, skipping role validation");
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
    async fn acquire_connection(&self) -> DbResult<DatabaseConnection> {
        // 尝试从空闲队列获取
        {
            let mut idle = self.inner.idle_connections.lock().await;
            if !idle.is_empty() {
                self.inner.active_count.fetch_add(1, Ordering::SeqCst);
                return idle.pop().ok_or_else(|| {
                    DbError::Connection(sea_orm::DbErr::ConnectionAcquire(sea_orm::ConnAcquireErr::Timeout))
                });
            }
        }

        // 检查是否达到最大连接数
        if self.inner.total_count.load(Ordering::SeqCst) >= self.inner.config.max_connections {
            // 等待空闲连接（使用条件变量替代忙等待）
            let timeout_duration = self.inner.config.acquire_timeout_duration();
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

            match result {
                Ok(Some(conn)) => {
                    self.inner.active_count.fetch_add(1, Ordering::SeqCst);
                    return Ok(conn);
                }
                Ok(None) => {
                    return Err(DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                        sea_orm::ConnAcquireErr::Timeout,
                    )));
                }
                Err(_) => {
                    return Err(DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                        sea_orm::ConnAcquireErr::Timeout,
                    )));
                }
            }
        }

        // 创建新连接
        let conn = Self::create_connection(&self.inner.config).await?;
        self.inner.total_count.fetch_add(1, Ordering::SeqCst);
        self.inner.active_count.fetch_add(1, Ordering::SeqCst);
        Ok(conn)
    }

    /// 归还连接到池中
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

/// Session 结构
pub struct Session {
    /// 数据库连接
    connection: Option<DatabaseConnection>,

    /// 连接池内部状态
    pool: Arc<DbPoolInner>,

    /// 角色
    role: String,

    /// 最后写操作时间（用于读写分离）
    last_write: Option<Instant>,

    /// 权限上下文
    permission_ctx: PermissionContext,

    /// 事务对象（用于真实的事务管理）
    transaction: Option<sea_orm::DatabaseTransaction>,

    /// 指标收集器（可选，用于 metrics 特性）
    #[cfg(feature = "metrics")]
    metrics: Option<Arc<MetricsCollector>>,
}

impl Session {
    fn new(connection: DatabaseConnection, pool: Arc<DbPoolInner>, role: String) -> Self {
        let permission_ctx = PermissionContext::new(role.clone(), pool.policy_cache.clone());

        Self {
            connection: Some(connection),
            pool,
            role,
            last_write: None,
            permission_ctx,
            transaction: None,
            #[cfg(feature = "metrics")]
            metrics: None,
        }
    }

    /// 设置指标收集器
    ///
    /// # Arguments
    ///
    /// * `metrics` - 指标收集器实例
    #[cfg(feature = "metrics")]
    pub fn set_metrics(&mut self, metrics: Arc<MetricsCollector>) {
        self.metrics = Some(metrics);
    }

    /// 获取角色
    pub fn role(&self) -> &str {
        &self.role
    }

    /// 获取权限上下文
    pub fn permission_ctx(&self) -> &PermissionContext {
        &self.permission_ctx
    }

    /// 标记写操作（用于读写分离）
    pub fn mark_write(&mut self) {
        self.last_write = Some(Instant::now());
    }

    /// 检查权限
    pub async fn check_permission(&self, table: &str, operation: &PermissionAction) -> Result<(), DbError> {
        if self.permission_ctx.check_table_access(table, operation).await {
            Ok(())
        } else {
            Err(DbError::Permission(format!(
                "Role '{}' does not have {} permission on table '{}'",
                self.role, operation, table
            )))
        }
    }

    /// 检查是否在事务中
    pub fn is_in_transaction(&self) -> bool {
        self.transaction.is_some()
    }

    /// 在事务中执行操作
    ///
    /// 这是推荐的事务使用方式。事务会在闭包执行完成后自动提交，
    /// 如果闭包返回错误则自动回滚。
    ///
    /// # Errors
    ///
    /// 如果已经在事务中，或闭包执行出错，返回错误
    pub async fn transaction<F, T, E>(&mut self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&mut Session) -> Result<T, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        if self.transaction.is_some() {
            return Err(DbError::Transaction("Transaction already in progress".to_string()));
        }

        // 获取底层连接并开始事务
        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                sea_orm::ConnAcquireErr::ConnectionClosed,
            ))
        })?;

        // 使用 Sea-ORM 的事务 API
        let txn = conn.begin().await?;

        // 保存事务对象
        self.transaction = Some(txn);

        // 执行用户代码
        match f(self) {
            Ok(res) => {
                // 提交事务
                if let Some(txn) = self.transaction.take() {
                    txn.commit().await?;
                }
                Ok(res)
            }
            Err(e) => {
                // 回滚事务
                if let Some(txn) = self.transaction.take() {
                    let _ = txn.rollback().await;
                }
                Err(DbError::Transaction(format!("Transaction failed: {}", e)))
            }
        }
    }

    /// 开始事务
    ///
    /// # Errors
    ///
    /// 如果已经在事务中，返回错误
    ///
    /// 注意：此方法会创建一个真实的数据库事务。
    /// 使用完毕后必须调用 commit() 或 rollback() 来结束事务。
    pub async fn begin_transaction(&mut self) -> Result<(), DbError> {
        if self.transaction.is_some() {
            return Err(DbError::Transaction("Transaction already in progress".to_string()));
        }

        // 获取底层连接并开始事务
        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                sea_orm::ConnAcquireErr::ConnectionClosed,
            ))
        })?;

        // 使用 Sea-ORM 的事务 API
        let txn = conn.begin().await?;

        // 保存事务对象
        self.transaction = Some(txn);

        Ok(())
    }

    /// 提交事务
    ///
    /// # Errors
    ///
    /// 如果没有活跃的事务，返回错误
    pub async fn commit(&mut self) -> Result<(), DbError> {
        let txn = self
            .transaction
            .take()
            .ok_or_else(|| DbError::Transaction("No active transaction to commit".to_string()))?;

        txn.commit().await?;

        Ok(())
    }

    /// 回滚事务
    ///
    /// # Errors
    ///
    /// 如果没有活跃的事务，返回错误
    pub async fn rollback(&mut self) -> Result<(), DbError> {
        let txn = self
            .transaction
            .take()
            .ok_or_else(|| DbError::Transaction("No active transaction to rollback".to_string()))?;

        txn.rollback().await?;

        Ok(())
    }

    /// 检查是否应该使用主库（写后读场景）
    pub fn should_use_master(&self) -> bool {
        self.last_write
            .map(|t| t.elapsed() < Duration::from_secs(5))
            .unwrap_or(false)
    }

    /// 获取数据库连接
    pub fn connection(&mut self) -> Result<&mut DatabaseConnection, DbError> {
        self.connection.as_mut().ok_or_else(|| {
            DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                sea_orm::ConnAcquireErr::ConnectionClosed,
            ))
        })
    }

    /// 执行原始 SQL 语句（带权限检查）
    ///
    /// # 安全说明
    ///
    /// 此方法会自动进行权限检查，确保用户有权限执行相应的操作。
    /// 如果无法解析 SQL 语句（例如复杂查询），则会拒绝执行以确保安全。
    ///
    /// 对于 DDL 操作（CREATE、DROP、ALTER 等），只允许管理员角色执行。
    /// 对于系统表（如 sqlite_master、information_schema 等），跳过权限检查。
    ///
    /// # Arguments
    ///
    /// * `sql` - 要执行的 SQL 语句
    ///
    /// # Returns
    ///
    /// 执行结果
    ///
    /// # Errors
    ///
    /// 如果 SQL 执行失败或权限不足，返回错误
    pub async fn execute_raw(&self, sql: &str) -> DbResult<sea_orm::ExecResult> {
        // 检查是否为 DDL 操作（使用更安全的检测方法）
        if self.is_ddl_operation(sql) {
            // DDL 操作只允许管理员角色执行
            if self.role() != "admin" {
                return Err(DbError::Permission(
                    "Permission denied: only 'admin' role can execute DDL operations".to_string(),
                ));
            }
        } else if let Some((table_name, action)) = self.parse_sql_operation(sql) {
            // 检查是否为系统表，系统表跳过权限检查
            let is_system_table = table_name.to_uppercase() == "SQLITE_MASTER"
                || table_name.to_uppercase() == "INFORMATION_SCHEMA"
                || table_name.to_uppercase().starts_with("PG_")
                || table_name.to_uppercase().starts_with("MYSQL.");

            if !is_system_table {
                // 检查是否配置了权限文件
                let permission_config = self.pool.permission_config.lock().await;
                let has_permission_config = permission_config.is_some();
                drop(permission_config);

                // 只有配置了权限文件才进行权限检查
                if has_permission_config {
                    // DML 操作：检查表级权限
                    if !self.permission_ctx.check_table_access(&table_name, &action).await {
                        return Err(DbError::Permission(format!(
                            "Permission denied: role '{}' does not have permission to '{}' on table '{}'",
                            self.role(),
                            action,
                            table_name
                        )));
                    }
                }
            }
        } else {
            // 如果无法解析 SQL 且不是 DDL，拒绝执行以确保安全
            return Err(DbError::Permission(
                "Unable to parse SQL statement for permission check, please use explicit methods".to_string(),
            ));
        }

        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Connection(sea_orm::DbErr::ConnectionAcquire(
                sea_orm::ConnAcquireErr::ConnectionClosed,
            ))
        })?;

        // 从 URL 解析数据库类型
        let backend = match self.pool.config.url.as_str() {
            url if url.starts_with("postgres") => sea_orm::DatabaseBackend::Postgres,
            url if url.starts_with("mysql") => sea_orm::DatabaseBackend::MySql,
            _ => sea_orm::DatabaseBackend::Sqlite,
        };

        let stmt = sea_orm::Statement::from_string(backend, sql.to_string());

        conn.execute_raw(stmt).await.map_err(DbError::Connection)
    }

    /// 检查 SQL 是否为 DDL 操作
    ///
    /// DDL 操作包括：CREATE, DROP, ALTER, TRUNCATE, RENAME, INDEX, TRIGGER 等
    /// 使用规范化后的 SQL 进行检查，防止通过空白字符或注释绕过
    fn is_ddl_operation(&self, sql: &str) -> bool {
        // 完整的 DDL 关键字列表
        const DDL_KEYWORDS: &[&str] = &[
            "CREATE",
            "DROP",
            "ALTER",
            "TRUNCATE",
            "RENAME",
            "INDEX",
            "TRIGGER",
            "VIEW",
            "FUNCTION",
            "PROCEDURE",
            "SEQUENCE",
            "SYNONYM",
            "DATABASE",
            "SCHEMA",
            "TEMPORARY",
            "EXTERNAL",
            "MATERIALIZED",
        ];

        // 去除前导空白和注释
        let normalized = Self::normalize_sql_statement(sql);

        // 检查是否以任何 DDL 关键字开头（不区分大小写）
        DDL_KEYWORDS.iter().any(|kw| {
            let upper_kw = kw.to_uppercase();
            normalized.to_uppercase().starts_with(&upper_kw)
        })
    }

    /// 规范化 SQL 语句（去除前导空白、注释等）
    fn normalize_sql_statement(sql: &str) -> String {
        let mut result = String::with_capacity(sql.len());
        let mut chars = sql.chars().peekable();

        while let Some(c) = chars.next() {
            // 跳过空白
            if c.is_whitespace() {
                continue;
            }

            // 处理 SQL 注释
            if c == '-' && chars.peek() == Some(&'-') {
                // 单行注释，跳到行尾
                while let Some(&next_c) = chars.peek() {
                    if next_c == '\n' {
                        break;
                    }
                    chars.next();
                }
                continue;
            }

            // 处理 C 风格注释
            if c == '/' && chars.peek() == Some(&'*') {
                chars.next(); // 消耗 '*'
                while let Some(c1) = chars.next() {
                    if c1 == '*' && chars.peek() == Some(&'/') {
                        chars.next(); // 消耗 '/'
                        break;
                    }
                }
                continue;
            }

            result.push(c);
        }

        result
    }

    /// 内部方法：解析 SQL 语句类型（用于权限检查）
    ///
    /// 使用 sqlparser 解析 SQL 语句类型（SELECT/INSERT/UPDATE/DELETE）
    /// 表名提取使用正则作为后备方案
    pub(super) fn parse_sql_operation(&self, sql: &str) -> Option<(String, PermissionAction)> {
        use sqlparser::dialect::GenericDialect;
        use sqlparser::parser::Parser;

        let _dialect = GenericDialect {};
        let _ast = Parser::parse_sql(&_dialect, sql).ok()?;

        let _statement = _ast.first()?;
        let sql_upper = sql.trim_start().to_uppercase();

        // 确定操作类型
        let action = if sql_upper.starts_with("SELECT") {
            PermissionAction::Select
        } else if sql_upper.starts_with("INSERT") {
            PermissionAction::Insert
        } else if sql_upper.starts_with("UPDATE") {
            PermissionAction::Update
        } else if sql_upper.starts_with("DELETE") {
            PermissionAction::Delete
        } else {
            return None;
        };

        // 提取表名
        let table_name = self.extract_table_name_fallback(sql, &action);
        table_name.map(|name| (name, action))
    }

    /// 从 SQL 中提取表名（正则后备方案）
    fn extract_table_name_fallback(&self, sql: &str, action: &PermissionAction) -> Option<String> {
        use std::sync::OnceLock;
        static INSERT_RE: OnceLock<regex::Regex> = OnceLock::new();
        static SELECT_RE: OnceLock<regex::Regex> = OnceLock::new();
        static UPDATE_RE: OnceLock<regex::Regex> = OnceLock::new();
        static DELETE_RE: OnceLock<regex::Regex> = OnceLock::new();
        static TABLE_RE: OnceLock<regex::Regex> = OnceLock::new();

        let insert_pattern = INSERT_RE.get_or_init(|| {
            regex::Regex::new(r"(?i)INSERT\s+INTO\s+([a-zA-Z_][a-zA-Z0-9_]*)")
                .expect("Failed to compile INSERT regex pattern")
        });

        let select_pattern = SELECT_RE.get_or_init(|| {
            regex::Regex::new(r"(?i)\bFROM\s+([a-zA-Z_][a-zA-Z0-9_]*)").expect("Failed to compile SELECT regex pattern")
        });

        let update_pattern = UPDATE_RE.get_or_init(|| {
            regex::Regex::new(r"(?i)\bUPDATE\s+([a-zA-Z_][a-zA-Z0-9_]*)")
                .expect("Failed to compile UPDATE regex pattern")
        });

        let delete_pattern = DELETE_RE.get_or_init(|| {
            regex::Regex::new(r"(?i)\bDELETE\s+FROM\s+([a-zA-Z_][a-zA-Z0-9_]*)")
                .expect("Failed to compile DELETE regex pattern")
        });

        let table_pattern = TABLE_RE.get_or_init(|| {
            regex::Regex::new(r##"(?:(?:([a-zA-Z_][a-zA-Z0-9_]*)\.)?([a-zA-Z_][a-zA-Z0-9_]*)|"([^"]+)"|`([^`]+)`)"##)
                .expect("Failed to compile table regex pattern")
        });

        // 根据操作类型选择对应的正则模式
        match action {
            PermissionAction::Select => {
                if let Some(caps) = select_pattern.captures(sql) {
                    if let Some(table) = caps.get(1) {
                        return Some(table.as_str().to_string());
                    }
                }
            }
            PermissionAction::Insert => {
                if let Some(caps) = insert_pattern.captures(sql) {
                    if let Some(table) = caps.get(1) {
                        return Some(table.as_str().to_string());
                    }
                }
            }
            PermissionAction::Update => {
                if let Some(caps) = update_pattern.captures(sql) {
                    if let Some(table) = caps.get(1) {
                        return Some(table.as_str().to_string());
                    }
                }
            }
            PermissionAction::Delete => {
                if let Some(caps) = delete_pattern.captures(sql) {
                    if let Some(table) = caps.get(1) {
                        return Some(table.as_str().to_string());
                    }
                }
            }
        }

        // 尝试通用表名模式作为后备
        if let Some(caps) = table_pattern.captures(sql) {
            if let (Some(schema), Some(table)) = (caps.get(1), caps.get(2)) {
                return Some(format!("{}.{}", schema.as_str(), table.as_str()));
            }
            if let Some(table) = caps.get(3).or(caps.get(4)) {
                return Some(table.as_str().to_string());
            }
        }

        None
    }

    /// 执行 SQL 语句的统一入口，集成权限检查和指标收集（自动解析操作类型）
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL 语句
    ///
    /// # Errors
    ///
    /// 如果权限检查失败或 SQL 执行失败，返回错误
    pub async fn execute(&mut self, sql: &str) -> DbResult<sea_orm::ExecResult> {
        use std::time::Instant;

        // 首先检查是否是 DDL 操作
        if self.is_ddl_operation(sql) {
            // DDL 操作只允许管理员角色执行
            if self.role() != "admin" {
                return Err(DbError::Permission(
                    "Permission denied: only 'admin' role can execute DDL operations".to_string(),
                ));
            }

            // 管理员执行 DDL，标记为写操作
            self.mark_write();

            // 执行 SQL（不进行权限检查）
            let result = self.execute_raw(sql).await;

            // 记录指标
            #[cfg(feature = "metrics")]
            {
                let _start_time = Instant::now();
                let duration = _start_time.elapsed();
                self.record_query_metrics("DDL", duration, result.is_ok());
            }

            return result;
        }

        // 尝试自动解析 SQL 操作类型和表名
        if let Some((table_name, operation)) = self.parse_sql_operation(sql) {
            // 权限检查
            self.check_permission(&table_name, &operation).await?;

            // 标记写操作（如果需要）
            if matches!(
                operation,
                PermissionAction::Insert | PermissionAction::Update | PermissionAction::Delete
            ) {
                self.mark_write();
            }

            // 记录开始时间用于指标收集
            let _start_time = Instant::now();

            // 执行 SQL
            let result = self.execute_raw(sql).await;

            // 记录指标
            #[cfg(feature = "metrics")]
            {
                let duration = _start_time.elapsed();
                let query_type = operation.to_string();
                self.record_query_metrics(&query_type, duration, result.is_ok());
            }

            result
        } else {
            // 如果无法解析 SQL，则执行原始 SQL（不进行权限检查）
            // 注意：这可能是一个安全风险，所以更好的做法是拒绝无法解析的语句
            Err(DbError::Permission(
                "Unable to parse SQL statement for permission check".to_string(),
            ))
        }
    }

    /// 执行 SQL 语句的统一入口，集成权限检查和指标收集（指定操作类型）
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL 语句
    /// * `table` - 操作的表名（用于权限检查）
    /// * `operation` - 操作类型（用于权限检查）
    ///
    /// # Errors
    ///
    /// 如果权限检查失败或 SQL 执行失败，返回错误
    pub async fn execute_with_operation(
        &mut self,
        sql: &str,
        table: &str,
        operation: PermissionAction,
    ) -> DbResult<sea_orm::ExecResult> {
        use std::time::Instant;

        // 权限检查
        self.check_permission(table, &operation).await?;

        // 标记写操作（如果需要）
        if matches!(
            operation,
            PermissionAction::Insert | PermissionAction::Update | PermissionAction::Delete
        ) {
            self.mark_write();
        }

        // 记录开始时间用于指标收集
        let _start_time = Instant::now();

        // 执行 SQL
        let result = self.execute_raw(sql).await;

        // 记录指标
        #[cfg(feature = "metrics")]
        {
            let duration = _start_time.elapsed();
            let query_type = operation.to_string();
            self.record_query_metrics(&query_type, duration, result.is_ok());
        }

        result
    }

    /// 记录查询指标
    ///
    /// # Arguments
    ///
    /// * `query_type` - 查询类型（如 "SELECT", "INSERT"）
    /// * `duration` - 查询耗时
    /// * `success` - 是否成功
    #[cfg(feature = "metrics")]
    pub fn record_query_metrics(&self, query_type: &str, duration: Duration, success: bool) {
        if let Some(ref metrics) = self.metrics {
            metrics.record_query(query_type, duration, success, None);
        }
    }

    /// 记录连接错误
    ///
    /// 用于在连接获取失败时记录错误指标
    #[cfg(feature = "metrics")]
    pub fn record_connection_error(&self) {
        if let Some(ref metrics) = self.metrics {
            metrics.record_connection_error();
        }
    }
}

/// 自动回滚未提交的事务
impl Drop for Session {
    fn drop(&mut self) {
        // 如果有未提交的事务，自动回滚
        if self.transaction.is_some() {
            tracing::warn!("Session dropped with uncommitted transaction, auto-rollback triggered");
            // 注意：由于 Drop 是同步的，无法进行异步回滚
            // 在实际应用中，开发者应该在使用完事务后显式调用 commit 或 rollback
            // 这里仅记录警告，事务会在连接关闭时由数据库自动回滚
        }

        // 归还连接到池
        if let Some(conn) = self.connection.take() {
            // 使用 fetch_update 防止计数变成负数
            let prev_count = self
                .pool
                .active_count
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |c| Some(c.saturating_sub(1)))
                .unwrap_or(0);

            // 如果之前计数已经是0，不再重复减
            if prev_count == 0 {
                tracing::warn!("Active count was already 0, skipping decrement");
            }

            let inner = self.pool.clone();

            // 更新指标（如果有 metrics 特性）
            #[cfg(feature = "metrics")]
            if let Some(ref metrics) = self.metrics {
                let status = PoolStatus {
                    total: inner.total_count.load(Ordering::SeqCst),
                    active: inner.active_count.load(Ordering::SeqCst).saturating_sub(1),
                    idle: (inner.total_count.load(Ordering::SeqCst) - inner.active_count.load(Ordering::SeqCst) + 1),
                };
                metrics.update_pool_status(status.total, status.active, status.idle);
            }

            // 注意：在 Drop 中启动异步任务可能不可靠（如果 Runtime 正在关闭）。
            // 建议显式调用 release_connection() 方法。
            // 这里使用 tokio::spawn 是为了向后兼容，但最好在业务代码中管理 Session 生命周期。
            #[allow(clippy::let_underscore_future)]
            let _ = tokio::spawn(async move {
                let mut idle = inner.idle_connections.lock().await;
                if idle.len() < inner.config.max_connections as usize {
                    idle.push(conn);
                    // 通知等待的请求者有新连接可用
                    inner.connection_available.notify_one();
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-020: 池状态创建测试
    #[test]
    fn test_pool_status_creation() {
        let status = PoolStatus {
            total: 10,
            active: 3,
            idle: 7,
        };

        assert_eq!(status.total, 10);
        assert_eq!(status.active, 3);
        assert_eq!(status.idle, 7);
    }

    /// TEST-U-021: 配置自动修正测试 - min > max
    #[test]
    fn test_auto_correct_min_greater_than_max() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 5,
            min_connections: 10,
            idle_timeout: 300,
            acquire_timeout: 5000,
            permissions_path: None,
            migrations_dir: None,
            auto_migrate: false,
            migration_timeout: 60,
        };

        let corrected_config = crate::config::ConfigCorrector::auto_correct(config);

        assert_eq!(corrected_config.max_connections, 5);
        assert_eq!(corrected_config.min_connections, 5); // 修正为等于 max
    }

    /// TEST-U-022: 配置自动修正测试 - 零值修正
    #[test]
    fn test_auto_correct_zero_values() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 0,
            min_connections: 0,
            idle_timeout: 0,
            acquire_timeout: 0,
            permissions_path: None,
            migrations_dir: None,
            auto_migrate: false,
            migration_timeout: 60,
        };

        let corrected_config = crate::config::ConfigCorrector::auto_correct(config);

        assert_eq!(corrected_config.max_connections, 10);
        assert_eq!(corrected_config.min_connections, 1);
        assert_eq!(corrected_config.idle_timeout, 300);
        assert_eq!(corrected_config.acquire_timeout, 5000);
    }

    /// TEST-U-023: 配置自动修正测试 - 范围限制
    #[test]
    fn test_auto_correct_value_bounds() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 10,
            min_connections: 5,
            idle_timeout: 10,       // 太小
            acquire_timeout: 50000, // 在范围内
            permissions_path: None,
            migrations_dir: None,
            auto_migrate: false,
            migration_timeout: 60,
        };

        let corrected_config = crate::config::ConfigCorrector::auto_correct(config);

        assert_eq!(corrected_config.idle_timeout, 30); // 调整为最小值
        assert_eq!(corrected_config.acquire_timeout, 50000); // 保持不变（50000 < 60000）
    }

    /// TEST-U-024: 数据库后端检测 - SQLite
    #[test]
    fn test_database_backend_sqlite() {
        let backend = DbPool::get_database_backend("sqlite::memory:");
        assert_eq!(backend, sea_orm::DatabaseBackend::Sqlite);

        let backend = DbPool::get_database_backend("sqlite:///tmp/test.db");
        assert_eq!(backend, sea_orm::DatabaseBackend::Sqlite);
    }

    /// TEST-U-025: 数据库后端检测 - PostgreSQL
    #[test]
    fn test_database_backend_postgres() {
        let backend = DbPool::get_database_backend("postgres://localhost/test");
        assert_eq!(backend, sea_orm::DatabaseBackend::Postgres);

        let backend = DbPool::get_database_backend("postgresql://localhost/test");
        assert_eq!(backend, sea_orm::DatabaseBackend::Postgres);
    }

    /// TEST-U-026: 数据库后端检测 - MySQL
    #[test]
    fn test_database_backend_mysql() {
        let backend = DbPool::get_database_backend("mysql://localhost/test");
        assert_eq!(backend, sea_orm::DatabaseBackend::MySql);
    }

    /// TEST-U-027: 健康检查查询生成 - SQLite
    #[test]
    fn test_health_check_query_sqlite() {
        let query = DbPool::get_health_check_query("sqlite::memory:");
        assert_eq!(query, "SELECT 1");
    }

    /// TEST-U-028: 健康检查查询生成 - PostgreSQL
    #[test]
    fn test_health_check_query_postgres() {
        let query = DbPool::get_health_check_query("postgres://localhost/test");
        assert_eq!(query, "SELECT 1");
    }

    /// TEST-U-029: 健康检查查询生成 - MySQL
    #[test]
    fn test_health_check_query_mysql() {
        let query = DbPool::get_health_check_query("mysql://localhost/test");
        assert_eq!(query, "SELECT 1");
    }

    /// TEST-U-030: 未知数据库后端默认使用 SQLite
    #[test]
    fn test_unknown_database_backend_defaults_to_sqlite() {
        let backend = DbPool::get_database_backend("unknown://localhost/test");
        assert_eq!(backend, sea_orm::DatabaseBackend::Sqlite);

        let query = DbPool::get_health_check_query("unknown://localhost/test");
        assert_eq!(query, "SELECT 1");
    }
}
