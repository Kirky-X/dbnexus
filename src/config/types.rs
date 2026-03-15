// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置类型定义
//!
//! 包含所有配置相关的核心类型定义。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// 配置加载统计
pub(crate) struct ConfigLoadStats {
    /// 环境变量加载次数
    pub(crate) env_loads: std::sync::atomic::AtomicU64,
    /// 文件加载次数
    pub(crate) file_loads: std::sync::atomic::AtomicU64,
    /// 路径遍历攻击拦截次数
    pub(crate) path_traversal_blocked: std::sync::atomic::AtomicU64,
    /// 无效协议拦截次数
    pub(crate) invalid_protocol_blocked: std::sync::atomic::AtomicU64,
}

impl ConfigLoadStats {
    /// 创建新的统计实例
    pub fn new() -> Self {
        use std::sync::atomic::AtomicU64;
        Self {
            env_loads: AtomicU64::new(0),
            file_loads: AtomicU64::new(0),
            path_traversal_blocked: AtomicU64::new(0),
            invalid_protocol_blocked: AtomicU64::new(0),
        }
    }

    /// 记录环境变量加载
    pub fn record_env_load(&self) {
        use std::sync::atomic::Ordering;
        self.env_loads.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录文件加载
    pub fn record_file_load(&self) {
        use std::sync::atomic::Ordering;
        self.file_loads.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录路径遍历攻击拦截
    pub fn record_path_traversal_blocked(&self) {
        use std::sync::atomic::Ordering;
        self.path_traversal_blocked.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录无效协议拦截
    pub fn record_invalid_protocol_blocked(&self) {
        use std::sync::atomic::Ordering;
        self.invalid_protocol_blocked.fetch_add(1, Ordering::SeqCst);
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> ConfigLoadStatsSnapshot {
        use std::sync::atomic::Ordering;
        ConfigLoadStatsSnapshot {
            env_loads: self.env_loads.load(Ordering::SeqCst),
            file_loads: self.file_loads.load(Ordering::SeqCst),
            path_traversal_blocked: self.path_traversal_blocked.load(Ordering::SeqCst),
            invalid_protocol_blocked: self.invalid_protocol_blocked.load(Ordering::SeqCst),
        }
    }
}

impl Default for ConfigLoadStats {
    fn default() -> Self {
        Self::new()
    }
}

/// 配置加载统计快照
#[derive(Debug, Clone)]
pub struct ConfigLoadStatsSnapshot {
    /// 环境变量加载次数
    pub env_loads: u64,
    /// 文件加载次数
    pub file_loads: u64,
    /// 路径遍历攻击拦截次数
    pub path_traversal_blocked: u64,
    /// 无效协议拦截次数
    pub invalid_protocol_blocked: u64,
}

/// 缓存配置
///
/// 用于配置各类缓存的容量和 TTL。
/// 所有缓存容量配置都支持通过配置文件或环境变量进行自定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 权限策略缓存容量
    ///
    /// 控制权限策略缓存的最大条目数。
    /// 默认值: 4096
    #[serde(default = "default_policy_cache_capacity")]
    pub policy_cache_capacity: u64,

    /// SQL 解析缓存容量
    ///
    /// 控制解析后的 SQL 语句缓存的最大条目数。
    /// 默认值: 1000
    #[serde(default = "default_sql_parse_cache_capacity")]
    pub sql_parse_cache_capacity: u64,

    /// 查询结果缓存容量
    ///
    /// 控制查询结果缓存的最大条目数。
    /// 默认值: 10000
    #[serde(default = "default_query_cache_capacity")]
    pub query_cache_capacity: u64,

    /// 默认 TTL（秒）
    ///
    /// 缓存条目的默认生存时间。
    /// 默认值: 300 秒（5 分钟）
    #[serde(default = "default_cache_ttl")]
    pub default_ttl: u64,
}

/// 默认权限策略缓存容量
pub(crate) fn default_policy_cache_capacity() -> u64 {
    4096
}

/// 默认 SQL 解析缓存容量
pub(crate) fn default_sql_parse_cache_capacity() -> u64 {
    1000
}

/// 默认查询结果缓存容量
pub(crate) fn default_query_cache_capacity() -> u64 {
    10000
}

/// 默认缓存 TTL（秒）
pub(crate) fn default_cache_ttl() -> u64 {
    300
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            policy_cache_capacity: default_policy_cache_capacity(),
            sql_parse_cache_capacity: default_sql_parse_cache_capacity(),
            query_cache_capacity: default_query_cache_capacity(),
            default_ttl: default_cache_ttl(),
        }
    }
}

impl CacheConfig {
    /// 创建新的缓存配置
    pub fn new(
        policy_cache_capacity: u64,
        sql_parse_cache_capacity: u64,
        query_cache_capacity: u64,
        default_ttl: u64,
    ) -> Self {
        Self {
            policy_cache_capacity,
            sql_parse_cache_capacity,
            query_cache_capacity,
            default_ttl,
        }
    }

    /// 获取权限策略缓存容量
    pub fn policy_cache_capacity(&self) -> u64 {
        self.policy_cache_capacity
    }

    /// 获取 SQL 解析缓存容量
    pub fn sql_parse_cache_capacity(&self) -> u64 {
        self.sql_parse_cache_capacity
    }

    /// 获取查询结果缓存容量
    pub fn query_cache_capacity(&self) -> u64 {
        self.query_cache_capacity
    }

    /// 获取默认 TTL（秒）
    pub fn default_ttl(&self) -> u64 {
        self.default_ttl
    }

    /// 获取默认 TTL Duration
    pub fn default_ttl_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.default_ttl)
    }

    /// 验证配置有效性
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.policy_cache_capacity == 0 {
            return Err(ConfigError::ValidationFailed);
        }
        if self.sql_parse_cache_capacity == 0 {
            return Err(ConfigError::ValidationFailed);
        }
        if self.query_cache_capacity == 0 {
            return Err(ConfigError::ValidationFailed);
        }
        if self.default_ttl == 0 {
            return Err(ConfigError::ValidationFailed);
        }
        Ok(())
    }
}

/// 数据库连接池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// 最大连接数
    max_connections: u32,
    /// 最小连接数
    min_connections: u32,
    /// 连接空闲超时时间（秒）
    idle_timeout: u64,
    /// 连接获取超时时间（毫秒）
    acquire_timeout: u64,
}

impl PoolConfig {
    /// 创建新的连接池配置
    ///
    /// 用于手动构建 `PoolConfig` 实例，适用于需要自定义连接池参数的场景。
    ///
    /// # Arguments
    ///
    /// * `max_connections` - 最大连接数
    /// * `min_connections` - 最小连接数
    /// * `idle_timeout` - 空闲连接超时时间（秒）
    /// * `acquire_timeout` - 获取连接超时时间（毫秒）
    ///
    /// # Example
    ///
    /// ```rust
    /// # use dbnexus::config::PoolConfig;
    /// let config = PoolConfig::new(100, 10, 300, 5000);
    /// ```
    pub fn new(max_connections: u32, min_connections: u32, idle_timeout: u64, acquire_timeout: u64) -> Self {
        Self {
            max_connections,
            min_connections,
            idle_timeout,
            acquire_timeout,
        }
    }

    /// 获取最大连接数
    pub fn max_connections(&self) -> u32 {
        self.max_connections
    }

    /// 获取最小连接数
    pub fn min_connections(&self) -> u32 {
        self.min_connections
    }

    /// 获取空闲超时时间（秒）
    pub fn idle_timeout(&self) -> u64 {
        self.idle_timeout
    }

    /// 获取连接获取超时时间（毫秒）
    pub fn acquire_timeout(&self) -> u64 {
        self.acquire_timeout
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 5,
            min_connections: 1,
            idle_timeout: 300,
            acquire_timeout: 5000,
        }
    }
}

/// 数据库类型枚举
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DatabaseType {
    /// PostgreSQL
    Postgres,
    /// MySQL
    MySql,
    /// SQLite
    Sqlite,
}

impl DatabaseType {
    /// 从字符串解析数据库类型
    pub fn parse_database_type(s: &str) -> Self {
        let s = s.to_lowercase();
        if s.starts_with("postgres") {
            DatabaseType::Postgres
        } else if s.starts_with("mysql") {
            DatabaseType::MySql
        } else {
            DatabaseType::Sqlite
        }
    }

    /// 获取数据库类型的显示名称
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseType::Postgres => "postgres",
            DatabaseType::MySql => "mysql",
            DatabaseType::Sqlite => "sqlite",
        }
    }

    /// 检查是否为真实数据库（非内存数据库）
    pub fn is_real_database(&self) -> bool {
        !matches!(self, DatabaseType::Sqlite)
    }
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 配置加载错误（生产环境安全，不暴露内部细节）
#[derive(Debug, Error)]
pub enum ConfigError {
    /// 文件未找到
    #[error("Configuration file not found")]
    FileNotFound,

    /// 格式无效
    #[error("Invalid configuration format")]
    InvalidFormat,

    /// 缺少必填字段
    #[error("Missing required configuration field")]
    MissingField,

    /// 环境变量错误
    #[error("Environment variable error")]
    EnvVarError,

    /// IO错误
    #[error("Configuration file I/O error")]
    IoError,

    /// URL格式错误（带详细错误信息）
    #[error("Invalid database URL format: {0}")]
    InvalidUrl(String),

    /// 不支持的数据库协议
    #[error("Unsupported database protocol")]
    UnsupportedProtocol,

    /// 验证失败
    #[error("Configuration validation failed")]
    ValidationFailed,

    /// 内部错误（包含原始错误，用于调试）
    #[cfg(feature = "dev")]
    #[error(transparent)]
    Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl From<std::io::Error> for ConfigError {
    fn from(_: std::io::Error) -> Self {
        ConfigError::IoError
    }
}

impl From<std::env::VarError> for ConfigError {
    fn from(_: std::env::VarError) -> Self {
        ConfigError::EnvVarError
    }
}

/// 默认值函数
pub(crate) fn default_admin_role() -> String {
    "admin".to_string()
}

pub(crate) fn default_max_connections() -> u32 {
    20
}

pub(crate) fn default_min_connections() -> u32 {
    5
}

pub(crate) fn default_idle_timeout() -> u64 {
    300
}

pub(crate) fn default_acquire_timeout() -> u64 {
    5000
}

pub(crate) fn default_migration_timeout() -> u64 {
    60
}

pub(crate) fn default_warmup_timeout() -> u64 {
    30
}

pub(crate) fn default_warmup_retries() -> u32 {
    3
}

/// 数据库配置
///
/// # 安全说明
///
/// 此结构体包含敏感的数据库连接信息（URL 可能包含密码）。
/// 建议：
/// - 通过 [`DbConfigBuilder`] 构建配置
/// - 使用提供的 getter 方法访问配置值
/// - 避免直接暴露 `url` 字段
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct DbConfig {
    /// 数据库连接 URL（敏感信息）
    #[serde(default)]
    pub(crate) url: String,

    /// 最大连接数
    #[serde(default = "default_max_connections")]
    pub(crate) max_connections: u32,

    /// 最小连接数
    #[serde(default = "default_min_connections")]
    pub(crate) min_connections: u32,

    /// 空闲连接超时（秒）
    #[serde(default = "default_idle_timeout")]
    pub(crate) idle_timeout: u64,

    /// 连接获取超时（毫秒）
    #[serde(default = "default_acquire_timeout")]
    pub(crate) acquire_timeout: u64,

    /// 权限配置文件路径
    #[serde(default)]
    pub(crate) permissions_path: Option<String>,

    /// 迁移文件目录
    #[serde(default)]
    pub(crate) migrations_dir: Option<PathBuf>,

    /// 是否启用自动迁移
    #[serde(default)]
    pub(crate) auto_migrate: bool,

    /// 迁移超时时间（秒）
    #[serde(default = "default_migration_timeout")]
    pub(crate) migration_timeout: u64,

    /// 管理员角色名称（用于 DDL 操作）
    #[serde(default = "default_admin_role")]
    pub(crate) admin_role: String,

    /// 预热超时时间（秒）
    #[serde(default = "default_warmup_timeout")]
    pub(crate) warmup_timeout: u64,

    /// 预热重试次数
    #[serde(default = "default_warmup_retries")]
    pub(crate) warmup_retries: u32,

    /// 缓存配置
    #[serde(default)]
    pub(crate) cache_config: CacheConfig,
}

impl DbConfig {
    /// 获取数据库 URL（原始值，包含密码）
    ///
    /// # 安全警告
    ///
    /// 此方法返回包含密码的原始 URL，可能导致敏感信息泄露。
    /// 请勿在日志、错误消息或调试输出中使用此方法。
    ///
    /// # Deprecated
    ///
    /// 此方法已弃用，请使用 [`Self::url_sanitized`] 获取脱敏后的 URL。
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::config::DbConfigBuilder;
    ///
    /// let config = DbConfigBuilder::new()
    ///     .url("postgres://user:password@localhost/db")
    ///     .build()
    ///     .unwrap();
    ///
    /// // 不推荐：可能泄露密码
    /// // let url = config.url();
    ///
    /// // 推荐：使用脱敏版本
    /// let sanitized = config.url_sanitized();
    /// assert!(sanitized.contains("postgres://"));
    /// assert!(!sanitized.contains("password"));
    /// ```
    #[deprecated(
        since = "0.3.0",
        note = "此方法可能泄露敏感信息，请使用 url_sanitized() 获取脱敏后的 URL"
    )]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 获取数据库 URL（脱敏版本）
    ///
    /// 隐藏密码等敏感信息，用于日志输出。
    ///
    /// # 安全说明
    ///
    /// 此方法会将 URL 中的密码替换为 `****`，防止敏感信息泄露。
    /// 适用于日志记录、错误消息和调试输出等场景。
    ///
    /// # Example
    ///
    /// ```
    /// use dbnexus::config::DbConfigBuilder;
    ///
    /// let config = DbConfigBuilder::new()
    ///     .url("postgres://user:password@localhost/db")
    ///     .build()
    ///     .unwrap();
    ///
    /// // 日志中使用脱敏版本
    /// let sanitized = config.url_sanitized();
    /// assert!(sanitized.contains("postgres://"));
    /// assert!(!sanitized.contains("password"));
    /// ```
    pub fn url_sanitized(&self) -> String {
        crate::config::security::sanitize_url_for_logging(&self.url)
    }

    /// 获取数据库 URL（原始值，包含密码）
    ///
    /// 此方法仅供库内部使用，用于数据库连接。
    /// 不会触发弃用警告，因为这是受控的内部使用。
    ///
    /// # Note
    ///
    /// 外部调用者应使用 [`Self::url_sanitized`] 进行日志输出。
    #[doc(hidden)]
    pub(crate) fn url_for_connection(&self) -> &str {
        &self.url
    }

    /// 获取最大连接数
    pub fn max_connections(&self) -> u32 {
        self.max_connections
    }

    /// 获取最小连接数
    pub fn min_connections(&self) -> u32 {
        self.min_connections
    }

    /// 获取空闲超时（秒）
    pub fn idle_timeout(&self) -> u64 {
        self.idle_timeout
    }

    /// 获取连接获取超时（毫秒）
    pub fn acquire_timeout(&self) -> u64 {
        self.acquire_timeout
    }

    /// 获取权限配置文件路径
    pub fn permissions_path(&self) -> Option<&str> {
        self.permissions_path.as_deref()
    }

    /// 获取迁移文件目录
    pub fn migrations_dir(&self) -> Option<&std::path::Path> {
        self.migrations_dir.as_deref()
    }

    /// 是否启用自动迁移
    pub fn auto_migrate(&self) -> bool {
        self.auto_migrate
    }

    /// 获取迁移超时（秒）
    pub fn migration_timeout(&self) -> u64 {
        self.migration_timeout
    }

    /// 获取管理员角色名称
    pub fn admin_role(&self) -> &str {
        &self.admin_role
    }

    /// 获取预热超时时间（秒）
    pub fn warmup_timeout(&self) -> u64 {
        self.warmup_timeout
    }

    /// 获取预热重试次数
    pub fn warmup_retries(&self) -> u32 {
        self.warmup_retries
    }

    /// 获取缓存配置
    ///
    /// 返回缓存配置的引用，包含各类缓存的容量和 TTL 配置。
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::config::DbConfigBuilder;
    ///
    /// let config = DbConfigBuilder::new()
    ///     .url("sqlite::memory:")
    ///     .policy_cache_capacity(8192)
    ///     .build()
    ///     .unwrap();
    ///
    /// let cache_config = config.cache_config();
    /// assert_eq!(cache_config.policy_cache_capacity(), 8192);
    /// ```
    pub fn cache_config(&self) -> &CacheConfig {
        &self.cache_config
    }

    /// 获取权限策略缓存容量
    ///
    /// 便捷方法，直接返回权限策略缓存容量。
    pub fn policy_cache_capacity(&self) -> u64 {
        self.cache_config.policy_cache_capacity()
    }

    /// 获取 SQL 解析缓存容量
    ///
    /// 便捷方法，直接返回 SQL 解析缓存容量。
    pub fn sql_parse_cache_capacity(&self) -> u64 {
        self.cache_config.sql_parse_cache_capacity()
    }

    /// 获取查询结果缓存容量
    ///
    /// 便捷方法，直接返回查询结果缓存容量。
    pub fn query_cache_capacity(&self) -> u64 {
        self.cache_config.query_cache_capacity()
    }

    /// 获取缓存默认 TTL（秒）
    ///
    /// 便捷方法，直接返回缓存默认 TTL。
    pub fn cache_default_ttl(&self) -> u64 {
        self.cache_config.default_ttl()
    }

    /// 内部方法：设置 URL（供构建器使用）
    pub(crate) fn set_url(&mut self, url: String) {
        self.url = url;
    }

    /// 设置最大连接数（内部使用）
    pub(crate) fn set_max_connections(&mut self, max_connections: u32) {
        self.max_connections = max_connections;
    }

    /// 设置最小连接数（内部使用）
    pub(crate) fn set_min_connections(&mut self, min_connections: u32) {
        self.min_connections = min_connections;
    }

    /// 设置空闲超时（内部使用）
    pub(crate) fn set_idle_timeout(&mut self, idle_timeout: u64) {
        self.idle_timeout = idle_timeout;
    }

    /// 设置获取超时（内部使用）
    pub(crate) fn set_acquire_timeout(&mut self, acquire_timeout: u64) {
        self.acquire_timeout = acquire_timeout;
    }

    /// 设置管理员角色名称（内部使用）
    pub(crate) fn set_admin_role(&mut self, admin_role: String) {
        self.admin_role = admin_role;
    }

    /// 内部方法：克隆配置（供连接池使用）
    pub(crate) fn clone_config(&self) -> Self {
        self.clone()
    }

    /// 获取空闲超时 Duration
    pub fn idle_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.idle_timeout)
    }

    /// 获取获取超时 Duration
    pub fn acquire_timeout_duration(&self) -> Duration {
        Duration::from_millis(self.acquire_timeout)
    }

    /// 获取迁移超时 Duration
    pub fn migration_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.migration_timeout)
    }
}

/// 数据库操作结果类型
pub type DbResult<T> = Result<T, DbError>;

/// 从 error::DbError 转换到 config::DbError
impl From<crate::error::DbError> for DbError {
    fn from(err: crate::error::DbError) -> Self {
        // 提取内部的 sea_orm::DbErr
        let inner_err = err.inner().clone();
        Self::Connection(inner_err)
    }
}

/// 数据库错误
#[derive(Debug, Error)]
pub enum DbError {
    /// 连接错误
    #[error("Connection error: {0}")]
    Connection(#[from] sea_orm::DbErr),

    /// 配置错误
    #[error("Configuration error: {0}")]
    Config(String),

    /// 权限错误
    #[error("Permission denied: {0}")]
    Permission(String),

    /// 事务错误
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// 迁移错误
    #[error("Migration error: {0}")]
    Migration(String),
}

// 类型别名（用于向后兼容）
/// DbConfig 的别名，用于向后兼容
pub type DbnexusConfig = DbConfig;
