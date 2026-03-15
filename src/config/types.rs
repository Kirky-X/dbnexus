// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置类型定义
//!
//! 纯数据结构，配置加载由 confers 库接管

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

// ============================================================================
// 错误类型
// ============================================================================

/// 配置错误
#[derive(Debug, Error)]
pub enum ConfigError {
    /// 缺少必填字段
    #[error("Missing required configuration: {0}")]
    MissingField(String),

    /// 缺少 URL
    #[error("Missing required configuration: dbnexus.url")]
    MissingUrl,

    /// 无效值
    #[error("Invalid configuration value for '{key}': {message}")]
    InvalidValue {
        /// 配置键名
        key: String,
        /// 错误消息
        message: String,
    },

    /// 无效格式
    #[error("Invalid configuration format: {0}")]
    InvalidFormat(String),

    /// 文件未找到
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),

    /// IO 错误
    #[error("IO error: {0}")]
    IoError(String),

    /// 无效 URL
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// 不支持的协议
    #[error("Unsupported database protocol: {0}")]
    UnsupportedProtocol(String),

    /// 解析错误
    #[error("Parse error: {0}")]
    ParseError(String),

    /// 验证错误
    #[error("Validation error: {0}")]
    ValidationError(String),
}

// ============================================================================
// 缓存配置
// ============================================================================

/// 缓存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 权限策略缓存容量
    #[serde(default = "default_policy_cache_capacity")]
    pub policy_cache_capacity: u64,

    /// SQL 解析缓存容量
    #[serde(default = "default_sql_parse_cache_capacity")]
    pub sql_parse_cache_capacity: u64,

    /// 查询结果缓存容量
    #[serde(default = "default_query_cache_capacity")]
    pub query_cache_capacity: u64,

    /// 默认 TTL（秒）
    #[serde(default = "default_cache_ttl")]
    pub default_ttl: u64,
}

fn default_policy_cache_capacity() -> u64 { 4096 }
fn default_sql_parse_cache_capacity() -> u64 { 1000 }
fn default_query_cache_capacity() -> u64 { 10000 }
fn default_cache_ttl() -> u64 { 300 }

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
    /// 获取默认 TTL Duration
    pub fn default_ttl_duration(&self) -> Duration {
        Duration::from_secs(self.default_ttl)
    }

    /// 从 confers ConfigProvider 加载
    #[cfg(feature = "confers")]
    pub fn from_confers(provider: &dyn confers::ConfigProvider) -> Result<Self, ConfigError> {
        use confers::ConfigProviderExt;

        Ok(Self {
            policy_cache_capacity: provider
                .get_uint("dbnexus.cache.policy_capacity")
                .unwrap_or(4096),
            sql_parse_cache_capacity: provider
                .get_uint("dbnexus.cache.sql_parse_capacity")
                .unwrap_or(1000),
            query_cache_capacity: provider
                .get_uint("dbnexus.cache.query_capacity")
                .unwrap_or(10000),
            default_ttl: provider
                .get_uint("dbnexus.cache.default_ttl")
                .unwrap_or(300),
        })
    }
}

// ============================================================================
// 连接池配置
// ============================================================================

/// 数据库连接池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// 最大连接数
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// 最小连接数
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// 连接空闲超时时间（秒）
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,

    /// 连接获取超时时间（毫秒）
    #[serde(default = "default_acquire_timeout")]
    pub acquire_timeout: u64,
}

fn default_max_connections() -> u32 { 20 }
fn default_min_connections() -> u32 { 5 }
fn default_idle_timeout() -> u64 { 300 }
fn default_acquire_timeout() -> u64 { 5000 }

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            idle_timeout: default_idle_timeout(),
            acquire_timeout: default_acquire_timeout(),
        }
    }
}

impl PoolConfig {
    /// 获取空闲超时 Duration
    pub fn idle_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.idle_timeout)
    }

    /// 获取获取超时 Duration
    pub fn acquire_timeout_duration(&self) -> Duration {
        Duration::from_millis(self.acquire_timeout)
    }
}

// ============================================================================
// 数据库类型
// ============================================================================

/// 数据库类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseType {
    /// PostgreSQL
    Postgres,
    /// MySQL
    MySql,
    /// SQLite
    Sqlite,
}

impl DatabaseType {
    /// 从 URL 解析数据库类型
    pub fn from_url(url: &str) -> Self {
        let url = url.to_lowercase();
        if url.starts_with("postgres") || url.starts_with("postgresql") {
            DatabaseType::Postgres
        } else if url.starts_with("mysql") {
            DatabaseType::MySql
        } else {
            DatabaseType::Sqlite
        }
    }

    /// 从 URL 解析数据库类型（别名）
    pub fn parse_database_type(url: &str) -> Self {
        Self::from_url(url)
    }

    /// 获取数据库类型的显示名称
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseType::Postgres => "postgres",
            DatabaseType::MySql => "mysql",
            DatabaseType::Sqlite => "sqlite",
        }
    }

    /// 检查是否为真实数据库（非 SQLite 内存数据库）
    pub fn is_real_database(&self) -> bool {
        matches!(self, DatabaseType::Postgres | DatabaseType::MySql)
    }
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// 数据库配置
// ============================================================================

/// 数据库配置
///
/// 纯数据结构，通过 `from_confers()` 从 confers 配置中加载
///
/// # 配置键
///
/// | 键 | 字段 | 默认值 |
/// |---|------|--------|
/// | `dbnexus.url` | `url` | **必填** |
/// | `dbnexus.max_connections` | `max_connections` | 20 |
/// | `dbnexus.min_connections` | `min_connections` | 5 |
/// | `dbnexus.idle_timeout` | `idle_timeout` | 300 |
/// | `dbnexus.acquire_timeout` | `acquire_timeout` | 5000 |
/// | `dbnexus.admin_role` | `admin_role` | "admin" |
/// | `dbnexus.permissions_path` | `permissions_path` | None |
/// | `dbnexus.migrations_dir` | `migrations_dir` | None |
/// | `dbnexus.auto_migrate` | `auto_migrate` | false |
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DbConfig {
    /// 数据库连接 URL
    pub url: String,

    /// 最大连接数
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// 最小连接数
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// 空闲连接超时（秒）
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,

    /// 连接获取超时（毫秒）
    #[serde(default = "default_acquire_timeout")]
    pub acquire_timeout: u64,

    /// 权限配置文件路径
    #[serde(default)]
    pub permissions_path: Option<String>,

    /// 迁移文件目录
    #[serde(default)]
    pub migrations_dir: Option<PathBuf>,

    /// 是否启用自动迁移
    #[serde(default)]
    pub auto_migrate: bool,

    /// 迁移超时时间（秒）
    #[serde(default = "default_migration_timeout")]
    pub migration_timeout: u64,

    /// 管理员角色名称（用于 DDL 操作）
    #[serde(default = "default_admin_role")]
    pub admin_role: String,

    /// 预热超时时间（秒）
    #[serde(default = "default_warmup_timeout")]
    pub warmup_timeout: u64,

    /// 预热重试次数
    #[serde(default = "default_warmup_retries")]
    pub warmup_retries: u32,

    /// 缓存配置
    #[serde(default)]
    pub cache_config: CacheConfig,
}

fn default_admin_role() -> String { "admin".to_string() }
fn default_migration_timeout() -> u64 { 60 }
fn default_warmup_timeout() -> u64 { 30 }
fn default_warmup_retries() -> u32 { 3 }

impl DbConfig {
    /// 从 confers ConfigProvider 加载配置
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use confers::ConfigProvider;
    /// use dbnexus::config::DbConfig;
    ///
    /// let provider = /* confers provider */;
    /// let config = DbConfig::from_confers(&provider)?;
    /// ```
    #[cfg(feature = "confers")]
    pub fn from_confers(provider: &dyn confers::ConfigProvider) -> Result<Self, ConfigError> {
        use confers::ConfigProviderExt;

        Ok(Self {
            url: provider
                .get_string("dbnexus.url")
                .ok_or(ConfigError::MissingUrl)?,
            max_connections: provider
                .get_uint("dbnexus.max_connections")
                .unwrap_or(20) as u32,
            min_connections: provider
                .get_uint("dbnexus.min_connections")
                .unwrap_or(5) as u32,
            idle_timeout: provider
                .get_uint("dbnexus.idle_timeout")
                .unwrap_or(300),
            acquire_timeout: provider
                .get_uint("dbnexus.acquire_timeout")
                .unwrap_or(5000),
            permissions_path: provider.get_string("dbnexus.permissions_path"),
            migrations_dir: provider
                .get_string("dbnexus.migrations_dir")
                .map(PathBuf::from),
            auto_migrate: provider
                .get_bool("dbnexus.auto_migrate")
                .unwrap_or(false),
            migration_timeout: provider
                .get_uint("dbnexus.migration_timeout")
                .unwrap_or(60),
            admin_role: provider
                .get_string("dbnexus.admin_role")
                .unwrap_or_else(|| "admin".to_string()),
            warmup_timeout: provider
                .get_uint("dbnexus.warmup_timeout")
                .unwrap_or(30),
            warmup_retries: provider
                .get_uint("dbnexus.warmup_retries")
                .unwrap_or(3) as u32,
            cache_config: CacheConfig::from_confers(provider)?,
        })
    }

    /// 获取数据库类型
    pub fn database_type(&self) -> DatabaseType {
        DatabaseType::from_url(&self.url)
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

    /// 获取缓存配置
    pub fn cache_config(&self) -> &CacheConfig {
        &self.cache_config
    }
}

// ============================================================================
// 类型别名
// ============================================================================

/// DbConfig 的别名，用于向后兼容
pub type DbnexusConfig = DbConfig;

/// 数据库操作结果类型
pub type DbResult<T> = Result<T, DbError>;

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

impl From<crate::error::DbError> for DbError {
    fn from(err: crate::error::DbError) -> Self {
        let inner_err = err.inner().clone();
        Self::Connection(inner_err)
    }
}
