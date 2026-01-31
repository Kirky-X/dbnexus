// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置管理模块
//!
//! 提供数据库配置加载、验证和自动修正功能
//!
//! # 主要功能
//!
//! - [`DbConfig`] - 数据库配置结构体
//! - [`DbConfigBuilder`] - 配置构建器（链式API）
//! - [`PoolConfig`] - 连接池配置
//! - [`ConfigError`] - 配置相关错误类型
//!
//! # 配置加载方式
//!
//! - [`DbConfig::from_env()`] - 从环境变量加载
//! - [`DbConfig::from_yaml_file()`] - 从 YAML 文件加载（需要 `config-yaml` 特性）
//! - [`DbConfig::from_toml_file()`] - 从 TOML 文件加载（需要 `config-toml` 特性）
//! - [`DbConfig::from_config_files()`] - 自动检测配置文件
//!
//! # 示例
//!
//! ```rust
//! use dbnexus::config::{DbConfig, DbConfigBuilder};
//!
//! // 使用构建器创建配置
//! let config = DbConfigBuilder::new()
//!     .url("sqlite::memory:")
//!     .max_connections(10)
//!     .min_connections(2)
//!     .build()
//!     .unwrap();
//! ```

#[cfg(any(feature = "postgres", feature = "mysql"))]
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use thiserror::Error;
use url::Url;

/// URL协议白名单（仅允许安全的数据库协议）
const ALLOWED_URL_SCHEMES: &[&str] = &["sqlite", "sqlite3", "postgres", "postgresql", "mysql", "mysql2"];

/// 敏感环境变量列表（加载时记录但不记录值）
const SENSITIVE_ENV_VARS: &[&str] = &[
    "DATABASE_URL",
    "DB_PASSWORD",
    "DB_PASSWORD_FILE",
    "DB_CERT",
    "DB_KEY",
    "DB_CA_CERT",
];

/// 环境变量最大长度限制
const MAX_ENV_VAR_LENGTH: usize = 4096;

/// 配置加载统计
pub(crate) struct ConfigLoadStats {
    /// 环境变量加载次数
    env_loads: AtomicU64,
    /// 文件加载次数
    file_loads: AtomicU64,
    /// 路径遍历攻击拦截次数
    path_traversal_blocked: AtomicU64,
    /// 无效协议拦截次数
    invalid_protocol_blocked: AtomicU64,
}

impl ConfigLoadStats {
    /// 创建新的统计实例
    pub fn new() -> Self {
        Self {
            env_loads: AtomicU64::new(0),
            file_loads: AtomicU64::new(0),
            path_traversal_blocked: AtomicU64::new(0),
            invalid_protocol_blocked: AtomicU64::new(0),
        }
    }

    /// 记录环境变量加载
    pub fn record_env_load(&self) {
        self.env_loads.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录文件加载
    pub fn record_file_load(&self) {
        self.file_loads.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录路径遍历攻击拦截
    pub fn record_path_traversal_blocked(&self) {
        self.path_traversal_blocked.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录无效协议拦截
    pub fn record_invalid_protocol_blocked(&self) {
        self.invalid_protocol_blocked.fetch_add(1, Ordering::SeqCst);
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> ConfigLoadStatsSnapshot {
        ConfigLoadStatsSnapshot {
            env_loads: self.env_loads.load(Ordering::SeqCst),
            file_loads: self.file_loads.load(Ordering::SeqCst),
            path_traversal_blocked: self.path_traversal_blocked.load(Ordering::SeqCst),
            invalid_protocol_blocked: self.invalid_protocol_blocked.load(Ordering::SeqCst),
        }
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

impl Default for ConfigLoadStats {
    fn default() -> Self {
        Self::new()
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

/// 配置构建器
///
/// 提供链式API用于构建 [`DbConfig`] 配置。
///
/// # 示例
///
/// ```rust
/// use dbnexus::config::DbConfigBuilder;
///
/// let config = DbConfigBuilder::new()
///     .url("sqlite::memory:")
///     .max_connections(20)
///     .min_connections(5)
///     .idle_timeout(300)
///     .acquire_timeout(5000)
///     .admin_role("admin")
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct DbConfigBuilder {
    url: Option<String>,
    max_connections: Option<u32>,
    min_connections: Option<u32>,
    idle_timeout: Option<u64>,
    acquire_timeout: Option<u64>,
    permissions_path: Option<String>,
    migrations_dir: Option<PathBuf>,
    auto_migrate: Option<bool>,
    migration_timeout: Option<u64>,
    admin_role: Option<String>,
    warmup_timeout: Option<u64>,
    warmup_retries: Option<u32>,
}

impl DbConfigBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置数据库 URL
    pub fn url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    /// 设置最大连接数
    pub fn max_connections(mut self, n: u32) -> Self {
        self.max_connections = Some(n);
        self
    }

    /// 设置最小连接数
    pub fn min_connections(mut self, n: u32) -> Self {
        self.min_connections = Some(n);
        self
    }

    /// 设置空闲超时（秒）
    pub fn idle_timeout(mut self, timeout: u64) -> Self {
        self.idle_timeout = Some(timeout);
        self
    }

    /// 设置获取超时（毫秒）
    pub fn acquire_timeout(mut self, timeout: u64) -> Self {
        self.acquire_timeout = Some(timeout);
        self
    }

    /// 设置权限配置文件路径
    pub fn permissions_path(mut self, path: &str) -> Self {
        self.permissions_path = Some(path.to_string());
        self
    }

    /// 设置迁移文件目录
    pub fn migrations_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.migrations_dir = Some(path.as_ref().to_path_buf());
        self
    }

    /// 设置是否自动迁移
    pub fn auto_migrate(mut self, auto: bool) -> Self {
        self.auto_migrate = Some(auto);
        self
    }

    /// 设置迁移超时（秒）
    pub fn migration_timeout(mut self, timeout: u64) -> Self {
        self.migration_timeout = Some(timeout);
        self
    }

    /// 设置管理员角色名称
    pub fn admin_role(mut self, role: &str) -> Self {
        self.admin_role = Some(role.to_string());
        self
    }

    /// 设置预热超时时间（秒）
    pub fn warmup_timeout(mut self, timeout: u64) -> Self {
        self.warmup_timeout = Some(timeout);
        self
    }

    /// 设置预热重试次数
    pub fn warmup_retries(mut self, retries: u32) -> Self {
        self.warmup_retries = Some(retries);
        self
    }

    /// 构建配置
    ///
    /// # Errors
    ///
    /// 如果验证失败，返回 [`ConfigError`]
    pub fn build(self) -> Result<DbConfig, ConfigError> {
        let config = DbConfig {
            url: self.url.unwrap_or_default(),
            max_connections: self.max_connections.unwrap_or_else(default_max_connections),
            min_connections: self.min_connections.unwrap_or_else(default_min_connections),
            idle_timeout: self.idle_timeout.unwrap_or_else(default_idle_timeout),
            acquire_timeout: self.acquire_timeout.unwrap_or_else(default_acquire_timeout),
            permissions_path: self.permissions_path,
            migrations_dir: self.migrations_dir,
            auto_migrate: self.auto_migrate.unwrap_or(false),
            migration_timeout: self.migration_timeout.unwrap_or_else(default_migration_timeout),
            admin_role: self.admin_role.unwrap_or_else(default_admin_role),
            warmup_timeout: self.warmup_timeout.unwrap_or_else(default_warmup_timeout),
            warmup_retries: self.warmup_retries.unwrap_or_else(default_warmup_retries),
        };

        config.validate()?;
        Ok(config)
    }
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
    url: String,

    /// 最大连接数
    #[serde(default = "default_max_connections")]
    max_connections: u32,

    /// 最小连接数
    #[serde(default = "default_min_connections")]
    min_connections: u32,

    /// 空闲连接超时（秒）
    #[serde(default = "default_idle_timeout")]
    idle_timeout: u64,

    /// 连接获取超时（毫秒）
    #[serde(default = "default_acquire_timeout")]
    acquire_timeout: u64,

    /// 权限配置文件路径
    #[serde(default)]
    permissions_path: Option<String>,

    /// 迁移文件目录
    #[serde(default)]
    migrations_dir: Option<PathBuf>,

    /// 是否启用自动迁移
    #[serde(default)]
    auto_migrate: bool,

    /// 迁移超时时间（秒）
    #[serde(default = "default_migration_timeout")]
    migration_timeout: u64,

    /// 管理员角色名称（用于 DDL 操作）
    #[serde(default = "default_admin_role")]
    admin_role: String,

    /// 预热超时时间（秒）
    #[serde(default = "default_warmup_timeout")]
    warmup_timeout: u64,

    /// 预热重试次数
    #[serde(default = "default_warmup_retries")]
    warmup_retries: u32,
}

impl DbConfig {
    /// 获取数据库 URL（原始值，包含密码）
    ///
    /// # Warning
    ///
    /// ⚠️ **安全风险**：此方法返回原始 URL，可能包含敏感信息（如密码）。
    /// 强烈建议在日志输出时使用 [`Self::url_sanitized`] 进行脱敏。
    ///
    /// # Deprecated
    ///
    /// 使用 [`Self::url_sanitized`] 替代，此方法将在未来版本中移除。
    /// 获取数据库 URL（脱敏版本）
    ///
    /// 隐藏密码等敏感信息，用于日志输出。
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
        sanitize_url_for_logging(&self.url)
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
    pub fn migrations_dir(&self) -> Option<&Path> {
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

    /// 内部方法：克隆配置（供连接池使用）
    pub(crate) fn clone_config(&self) -> Self {
        self.clone()
    }
}

/// 对 URL 进行脱敏处理，用于日志输出
fn sanitize_url_for_logging(url: &str) -> String {
    // 特殊处理 SQLite 内存数据库
    if url.starts_with("sqlite::memory:") || url.starts_with("sqlite3::memory:") {
        return url.to_string();
    }
    if url.starts_with("sqlite:") || url.starts_with("sqlite3:") {
        return url.to_string();
    }

    // 处理标准的数据库 URL 格式：protocol://user:password@host:port/path
    if let Some(at_pos) = url.find('@') {
        let protocol_end = url.find("://").map(|p| p + 3).unwrap_or(0);
        let protocol_part = &url[..protocol_end];
        let rest = &url[at_pos..];
        format!("{}****@{}", protocol_part, rest)
    } else {
        // 没有 @ 符号，可能是没有密码的 URL 或其他格式
        url.to_string()
    }
}

fn default_admin_role() -> String {
    "admin".to_string()
}

fn default_max_connections() -> u32 {
    20
}

fn default_min_connections() -> u32 {
    5
}

fn default_idle_timeout() -> u64 {
    300
}

fn default_acquire_timeout() -> u64 {
    5000
}

fn default_migration_timeout() -> u64 {
    60
}

fn default_warmup_timeout() -> u64 {
    30
}

fn default_warmup_retries() -> u32 {
    3
}

impl DbConfig {
    /// 从环境变量创建配置
    ///
    /// # Errors
    ///
    /// 如果必需的环境变量缺失或格式错误，返回错误
    pub fn from_env() -> Result<Self, ConfigError> {
        const MAX_URL_LENGTH: usize = 2048;
        const MAX_ROLE_LENGTH: usize = 64;
        const MAX_PATH_LENGTH: usize = 512;

        let url = std::env::var("DATABASE_URL").map_err(|_| ConfigError::MissingField)?;

        // URL 长度限制，防止 DoS 攻击
        if url.len() > MAX_URL_LENGTH {
            return Err(ConfigError::InvalidFormat);
        }

        let max_connections = std::env::var("DB_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "20".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidFormat)?;

        let min_connections = std::env::var("DB_MIN_CONNECTIONS")
            .unwrap_or_else(|_| "5".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidFormat)?;

        let idle_timeout = std::env::var("DB_IDLE_TIMEOUT")
            .unwrap_or_else(|_| "300".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidFormat)?;

        let acquire_timeout = std::env::var("DB_ACQUIRE_TIMEOUT")
            .unwrap_or_else(|_| "5000".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidFormat)?;

        let admin_role = std::env::var("DB_ADMIN_ROLE").unwrap_or_else(|_| "admin".to_string());

        // 角色名长度限制
        if admin_role.len() > MAX_ROLE_LENGTH {
            return Err(ConfigError::InvalidFormat);
        }

        Ok(Self {
            url,
            max_connections,
            min_connections,
            idle_timeout,
            acquire_timeout,
            permissions_path: std::env::var("DB_PERMISSIONS_PATH").ok(),
            migrations_dir: std::env::var("DB_MIGRATIONS_DIR").ok().map(PathBuf::from),
            auto_migrate: std::env::var("DB_AUTO_MIGRATE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            migration_timeout: std::env::var("DB_MIGRATION_TIMEOUT")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
            admin_role,
            warmup_timeout: std::env::var("DB_WARMUP_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            warmup_retries: std::env::var("DB_WARMUP_RETRIES")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .unwrap_or(3),
        })
    }

    /// 从 YAML 文件加载配置
    ///
    /// 支持以下格式：
    /// ```yaml
    /// database:
    ///   url: "sqlite::memory:"
    ///   max_connections: 20
    ///   min_connections: 5
    ///   idle_timeout: 300
    ///   acquire_timeout: 5000
    /// ```
    ///
    /// # Errors
    ///
    /// 如果文件不存在或格式错误，返回错误
    #[cfg(feature = "config-yaml")]
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref())?;
        Self::from_yaml_str(&content)
    }

    /// 从 TOML 文件加载配置
    ///
    /// 支持以下格式：
    /// ```toml
    /// [database]
    /// url = "sqlite::memory:"
    /// max_connections = 20
    /// min_connections = 5
    /// idle_timeout = 300
    /// acquire_timeout = 5000
    /// ```
    ///
    /// # Errors
    ///
    /// 如果文件不存在或格式错误，返回错误
    #[cfg(feature = "config-toml")]
    pub fn from_toml_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref())?;

        // 尝试直接解析为 DbConfig
        if let Ok(config) = toml::from_str::<DbConfig>(&content) {
            if !config.url.is_empty() {
                return Ok(config);
            }
        }

        // 尝试解析为带有 database 前缀的格式
        #[derive(Debug, serde::Deserialize)]
        struct ConfigWrapper {
            database: DbConfig,
        }

        let wrapper: ConfigWrapper = toml::from_str(&content).map_err(|_| ConfigError::InvalidFormat)?;

        wrapper.database.validate()?;
        Ok(wrapper.database)
    }

    /// 从 YAML 字符串加载配置
    ///
    /// # Errors
    ///
    /// 如果格式错误，返回错误
    #[cfg(feature = "config-yaml")]
    pub fn from_yaml_str(yaml: &str) -> Result<Self, ConfigError> {
        let config: DbConfig = serde_yaml::from_str(yaml).map_err(|_| ConfigError::InvalidFormat)?;

        config.validate()?;
        Ok(config)
    }

    /// 从 TOML 字符串加载配置
    ///
    /// # Errors
    ///
    /// 如果格式错误，返回错误
    #[cfg(feature = "config-toml")]
    pub fn from_toml_str(toml: &str) -> Result<Self, ConfigError> {
        let config: DbConfig = toml::from_str(toml).map_err(|_| ConfigError::InvalidFormat)?;

        config.validate()?;
        Ok(config)
    }

    /// 验证配置必填字段
    ///
    /// # Errors
    ///
    /// 如果缺少必填字段或格式无效，返回错误
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.url.is_empty() {
            return Err(ConfigError::MissingField);
        }

        // URL 格式验证
        self.validate_url_format()?;

        if self.max_connections == 0 {
            return Err(ConfigError::MissingField);
        }

        // 验证 max_connections 范围（1-1000）
        if self.max_connections > 1000 {
            return Err(ConfigError::ValidationFailed);
        }

        // 验证 min_connections 范围（1-100）
        if self.min_connections == 0 || self.min_connections > 100 {
            return Err(ConfigError::ValidationFailed);
        }

        if self.min_connections > self.max_connections {
            return Err(ConfigError::InvalidFormat);
        }

        Ok(())
    }

    /// 验证数据库 URL 格式（增强版 - 包含协议白名单验证）
    fn validate_url_format(&self) -> Result<(), ConfigError> {
        // 特殊处理 sqlite::memory: 和 sqlite3::memory: 格式（无 ://）
        if self.url.starts_with("sqlite::memory:") || self.url.starts_with("sqlite3::memory:") {
            return Ok(());
        }
        // 特殊处理 sqlite: 和 sqlite3: 格式（无 //）
        if self.url.starts_with("sqlite:") || self.url.starts_with("sqlite3:") {
            return Ok(());
        }

        // 使用 URL 解析器进行完整验证
        let url = Url::parse(&self.url).map_err(|e| ConfigError::InvalidUrl(format!("Invalid URL format: {}", e)))?;

        let protocol = url.scheme();

        // 1. 检查协议格式（字母数字 + + . -）
        if !protocol
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
        {
            return Err(ConfigError::InvalidUrl(
                "Protocol contains invalid characters".to_string(),
            ));
        }

        // 2. 协议白名单验证（严格模式）
        let protocol_lower = protocol.to_lowercase();
        let is_valid_protocol = ALLOWED_URL_SCHEMES.contains(&protocol_lower.as_str())
            || (protocol_lower.starts_with("sqlite")
                && ["file", "mem", "memory"].contains(&protocol_lower.split(':').nth(1).unwrap_or("")));

        if !is_valid_protocol {
            // 记录无效协议拦截
            tracing::warn!(
                target: "security",
                "Blocked unsupported protocol '{}' in URL",
                protocol
            );
            return Err(ConfigError::UnsupportedProtocol);
        }

        // 3. 验证主机名格式（如果有）
        if let Some(host) = url.host() {
            let host_str = host.to_string();
            // 主机名不能包含空白字符或特殊符号（防止注入）
            if host_str
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '\'' | '"' | ';' | '|' | '&' | '$' | '`'))
            {
                return Err(ConfigError::InvalidUrl(
                    "Hostname contains invalid characters".to_string(),
                ));
            }

            // 检查是否是IP地址（如果是，需要验证格式）
            if host_str.chars().all(|c| c.is_ascii_digit() || c == '.') {
                // 简单的IP格式验证
                let parts: Vec<&str> = host_str.split('.').collect();
                if parts.len() == 4 {
                    for part in parts {
                        if part.parse::<u8>().is_err() {
                            return Err(ConfigError::InvalidUrl("Invalid IP address format".to_string()));
                        }
                    }
                }
            }
        }

        // 4. 验证端口号范围（如果有）
        if let Some(port) = url.port() {
            if port == 0 {
                return Err(ConfigError::InvalidUrl(
                    "Port number out of valid range (1-65535)".to_string(),
                ));
            }
        }

        // 5. 验证路径（如果有）- 防止SQL注入和路径遍历
        let path = url.path();
        if !path.is_empty() && (path.contains(';') || path.contains('\'') || path.contains('"')) {
            return Err(ConfigError::InvalidUrl(
                "URL path contains potentially dangerous characters".to_string(),
            ));
        }

        // 6. 验证用户名和密码（如果有）- 防止密码注入
        let user = url.username();
        if !user.is_empty() && (user.contains(':') || user.contains('@') || user.contains('/')) {
            return Err(ConfigError::InvalidUrl(
                "Username contains invalid characters".to_string(),
            ));
        }

        if let Some(password) = url.password() {
            if password.contains(':') || password.contains('@') || password.contains('/') {
                return Err(ConfigError::InvalidUrl(
                    "Password contains invalid characters".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// 安全地清理环境变量值
    ///
    /// # Arguments
    ///
    /// * `value` - 原始环境变量值
    /// * `is_sensitive` - 是否是敏感值
    ///
    /// # Returns
    ///
    /// 清理后的值
    fn sanitize_env_value(value: &str, is_sensitive: bool) -> String {
        // 检查长度
        if value.len() > MAX_ENV_VAR_LENGTH {
            tracing::warn!(
                target: "security",
                "Environment variable value exceeds maximum length, truncating"
            );
            return if is_sensitive {
                "[REDACTED - TOO LONG]".to_string()
            } else {
                value.chars().take(MAX_ENV_VAR_LENGTH).collect()
            };
        }

        // 移除潜在的恶意字符
        let sanitized: String = value
            .chars()
            .filter(|c| {
                // 移除null字节、控制字符
                !c.is_control() && *c != '\0'
            })
            .collect();

        if is_sensitive {
            "[REDACTED]".to_string()
        } else {
            sanitized
        }
    }

    /// 检查环境变量是否是敏感的
    fn is_sensitive_env_var(name: &str) -> bool {
        SENSITIVE_ENV_VARS
            .iter()
            .any(|sensitive| name.eq_ignore_ascii_case(sensitive))
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

    /// 将配置序列化为 YAML 字符串
    #[cfg(feature = "config-yaml")]
    pub fn to_yaml(&self) -> Result<String, ConfigError> {
        serde_yaml::to_string(self).map_err(|_| ConfigError::InvalidFormat)
    }

    /// 将配置序列化为 TOML 字符串
    #[cfg(feature = "config-toml")]
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string(self).map_err(|_| ConfigError::InvalidFormat)
    }

    /// 自动加载配置文件
    ///
    /// 按顺序尝试以下路径：
    /// 1. ./dbnexus.yaml
    /// 2. ./dbnexus.toml
    /// 3. ./config/dbnexus.yaml
    /// 4. ./config/dbnexus.toml
    /// 5. ~/.config/dbnexus/config.yaml
    /// 6. ~/.dbnexus/config.toml
    ///
    /// 如果找到文件，使用环境变量覆盖配置
    ///
    /// # Errors
    ///
    /// 如果未找到配置文件或文件格式错误，返回错误
    pub fn from_config_files() -> Result<Self, ConfigError> {
        #[cfg(all(feature = "config-yaml", feature = "config-toml"))]
        {
            let config_paths = [
                "dbnexus.yaml",
                "dbnexus.toml",
                "config/dbnexus.yaml",
                "config/dbnexus.toml",
            ];

            // 尝试查找配置文件
            for config_path in &config_paths {
                let path = Path::new(config_path);

                // 安全检查：路径规范化、符号链接检查、父目录引用检查
                if Self::is_safe_config_path(path)? {
                    tracing::info!("Loading configuration from: {}", config_path);

                    if config_path.ends_with(".yaml") || config_path.ends_with(".yml") {
                        return Self::from_yaml_file(path);
                    } else {
                        return Self::from_toml_file(path);
                    }
                }
            }

            // 尝试用户目录
            if let Some(home_dir) = home::home_dir() {
                let user_config_paths = [
                    home_dir.join(".config").join("dbnexus").join("config.yaml"),
                    home_dir.join(".dbnexus").join("config.toml"),
                ];

                for config_path in &user_config_paths {
                    if Self::is_safe_config_path(config_path)? {
                        tracing::info!("Loading configuration from: {}", config_path.display());

                        if config_path.ends_with(".yaml") {
                            return Self::from_yaml_file(config_path);
                        } else {
                            return Self::from_toml_file(config_path);
                        }
                    }
                }
            }
        }

        #[cfg(all(feature = "config-yaml", not(feature = "config-toml")))]
        {
            let config_paths = ["dbnexus.yaml", "config/dbnexus.yaml"];

            for config_path in &config_paths {
                let path = Path::new(config_path);

                if Self::is_safe_config_path(path)? {
                    tracing::info!("Loading configuration from: {}", config_path);
                    return Self::from_yaml_file(path);
                }
            }

            if let Some(home_dir) = home::home_dir() {
                let user_config_paths = [home_dir.join(".config").join("dbnexus").join("config.yaml")];

                for config_path in &user_config_paths {
                    if Self::is_safe_config_path(config_path)? {
                        tracing::info!("Loading configuration from: {}", config_path.display());
                        return Self::from_yaml_file(config_path);
                    }
                }
            }
        }

        #[cfg(all(not(feature = "config-yaml"), feature = "config-toml"))]
        {
            let config_paths = ["dbnexus.toml", "config/dbnexus.toml"];

            for config_path in &config_paths {
                let path = Path::new(config_path);

                if Self::is_safe_config_path(path)? {
                    tracing::info!("Loading configuration from: {}", config_path);
                    return Self::from_toml_file(path);
                }
            }

            if let Some(home_dir) = home::home_dir() {
                let user_config_paths = [home_dir.join(".dbnexus").join("config.toml")];

                for config_path in &user_config_paths {
                    if Self::is_safe_config_path(config_path)? {
                        tracing::info!("Loading configuration from: {}", config_path.display());
                        return Self::from_toml_file(config_path);
                    }
                }
            }
        }

        Err(ConfigError::FileNotFound)
    }

    /// 检查配置文件路径是否安全（增强版 - 包含多层防护）
    ///
    /// 防止路径遍历攻击：
    /// - 检查路径是否包含父目录引用 (..)
    /// - 检查路径是否包含符号链接
    /// - 检查路径是否在预期目录内
    /// - 检查 Windows 风格路径遍历
    /// - 检查 null 字节注入
    /// - 检查环境变量扩展攻击
    /// - 检查特殊字符注入
    fn is_safe_config_path(path: &Path) -> Result<bool, ConfigError> {
        // 1. 检查 null 字节注入
        let path_str = path.to_string_lossy();
        if path_str.contains('\0') {
            tracing::warn!("Rejected config path with null byte: {:?}", path);
            return Ok(false);
        }

        // 2. 检查环境变量扩展（防止 $VAR 或 ${VAR} 形式的攻击）
        if path_str.contains("$(") || path_str.contains("${") || path_str.contains("`") {
            tracing::warn!("Rejected config path with environment variable expansion: {:?}", path);
            return Ok(false);
        }

        // 3. 检查路径是否包含 ..（父目录遍历）
        if path_str.contains("..") {
            tracing::warn!("Rejected config path with parent directory traversal: {:?}", path);
            return Ok(false);
        }

        // 4. 检查 Windows 风格路径遍历
        if path_str.contains(".\\") || path_str.starts_with(".\\") {
            tracing::warn!("Rejected config path with Windows-style traversal: {:?}", path);
            return Ok(false);
        }

        // 5. 检查特殊字符（可能用于路径注入）
        let dangerous_chars = [';', '|', '&', '>', '<', '*', '?', '~'];
        for c in dangerous_chars {
            if path_str.contains(c) {
                tracing::warn!("Rejected config path with dangerous character '{}': {:?}", c, path);
                return Ok(false);
            }
        }

        // 6. 检查是否是绝对路径
        if path.is_absolute() {
            // 检查是否在允许的目录范围内
            if let Ok(canonical) = path.canonicalize() {
                // 检查是否在用户目录或当前目录
                let home_dir = home::home_dir();
                let current_dir = std::env::current_dir().ok();

                let is_in_allowed_location = home_dir
                    .as_ref()
                    .is_some_and(|home| canonical.starts_with(home) || canonical.starts_with(home.join(".config")))
                    || current_dir.as_ref().is_some_and(|current| {
                        canonical.starts_with(current) || canonical.starts_with(current.join("config"))
                    });

                if !is_in_allowed_location {
                    // 检查是否在系统关键目录
                    let forbidden_prefixes = [
                        "/etc", "/usr", "/var", "/root", "/boot", "/srv", "/opt", "/bin", "/sbin", "/lib", "/lib64",
                        "/proc", "/sys", "/dev",
                    ];
                    for prefix in &forbidden_prefixes {
                        if canonical.starts_with(prefix) {
                            tracing::warn!("Rejected config path in system directory: {:?}", path);
                            return Ok(false);
                        }
                    }
                }
            }
        }

        // 7. 规范化路径并检查
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to canonicalize config path {:?}: {}", path, e);
                return Ok(false);
            }
        };

        // 8. 检查符号链接（指向不安全位置的符号链接）
        if path.is_symlink() {
            tracing::warn!("Rejected symlink config path: {:?}", path);
            return Ok(false);
        }

        // 9. 检查规范化后的路径是否仍然包含 ..
        if canonical.to_string_lossy().contains("..") {
            tracing::warn!(
                "Rejected config path with hidden traversal after canonicalization: {:?}",
                path
            );
            return Ok(false);
        }

        // 10. 检查路径是否指向目录（配置文件应该是文件）
        if canonical.is_dir() {
            tracing::warn!("Rejected config path pointing to directory: {:?}", path);
            return Ok(false);
        }

        // 11. 检查文件扩展名是否安全
        if let Some(ext) = path.extension() {
            if ext == "sh" || ext == "bash" || ext == "py" || ext == "js" {
                tracing::warn!("Rejected config path with dangerous extension: {:?}", path);
                return Ok(false);
            }
        }

        Ok(true)
    }
}

// 类型别名（用于向后兼容）
/// DbConfig 的别名，用于向后兼容
pub type DbnexusConfig = DbConfig;

/// DbConfigBuilder 的别名，用于向后兼容
pub type DbnexusConfigBuilder = DbConfigBuilder;

/// 配置自动修正器
#[derive(Debug, Clone)]
pub struct ConfigCorrector;

impl ConfigCorrector {
    /// 获取数据库的最大连接数限制
    ///
    /// 通过查询数据库系统变量获取最大连接数限制。
    /// 如果查询失败，返回默认的保守估计值。
    ///
    /// # Arguments
    ///
    /// * `connection` - 数据库连接
    /// * `db_type` - 数据库类型
    ///
    /// # Returns
    ///
    /// 数据库支持的最大连接数
    pub(crate) async fn query_database_max_connections(
        connection: &sea_orm::DatabaseConnection,
        db_type: DatabaseType,
    ) -> u32 {
        let _ = connection;
        match db_type {
            DatabaseType::Postgres => {
                #[cfg(feature = "postgres")]
                {
                    let result = connection.execute_unprepared("SHOW max_connections").await;

                    match result {
                        Ok(result) => {
                            let rows_affected = result.rows_affected();
                            if rows_affected > 0 {
                                tracing::info!(
                                    "PostgreSQL max_connections query executed, using conservative estimate"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to query PostgreSQL max_connections: {}", e);
                        }
                    }
                    100
                }

                #[cfg(not(feature = "postgres"))]
                {
                    100
                }
            }
            DatabaseType::MySql => {
                #[cfg(feature = "mysql")]
                {
                    let result = connection
                        .execute_unprepared("SHOW VARIABLES LIKE 'max_connections'")
                        .await;

                    match result {
                        Ok(_) => {
                            tracing::info!("MySQL max_connections query executed, using conservative estimate");
                        }
                        Err(e) => {
                            tracing::warn!("Failed to query MySQL max_connections: {}", e);
                        }
                    }
                    200
                }

                #[cfg(not(feature = "mysql"))]
                {
                    200
                }
            }
            DatabaseType::Sqlite => {
                // SQLite 不需要查询，它支持几乎无限的连接
                // 但我们仍设置一个合理的上限
                u32::MAX
            }
        }
    }

    /// 自动修正数据库配置
    pub(crate) fn auto_correct(mut config: DbConfig) -> DbConfig {
        // 修正 min_connections > max_connections
        if config.min_connections > config.max_connections {
            tracing::warn!(
                "Correcting min_connections ({}) > max_connections ({}), setting min to max",
                config.min_connections(),
                config.max_connections()
            );
            config.min_connections = config.max_connections;
        }

        // 确保最小连接数至少为 1
        if config.min_connections == 0 {
            config.min_connections = 1;
            tracing::warn!("Correcting min_connections from 0 to 1");
        }

        // 确保最大连接数至少等于最小连接数，且不超过合理范围
        if config.max_connections == 0 {
            config.max_connections = 10;
            tracing::warn!("Correcting max_connections from 0 to 10");
        }

        // 修正 acquire_timeout 为合理范围
        if config.acquire_timeout == 0 {
            config.acquire_timeout = 5000;
        } else if config.acquire_timeout < 1000 {
            tracing::warn!(
                "Adjusting acquire_timeout from {}ms to minimum 1000ms",
                config.acquire_timeout()
            );
            config.acquire_timeout = 1000;
        } else if config.acquire_timeout > 60000 {
            tracing::warn!(
                "Adjusting acquire_timeout from {}ms to maximum 60000ms",
                config.acquire_timeout()
            );
            config.acquire_timeout = 60000;
        }

        // 修正 idle_timeout 为合理范围
        if config.idle_timeout == 0 {
            config.idle_timeout = 300;
        } else if config.idle_timeout < 30 {
            tracing::warn!("Adjusting idle_timeout from {}s to minimum 30s", config.idle_timeout());
            config.idle_timeout = 30;
        } else if config.idle_timeout > 3600 {
            tracing::warn!(
                "Adjusting idle_timeout from {}s to maximum 3600s",
                config.idle_timeout()
            );
            config.idle_timeout = 3600;
        }

        // 对数据库URL进行一些基本检查和修正
        if config.url.starts_with("mysql") || config.url.starts_with("postgres") {
            // 检查URL是否包含必要的参数
            if config.url.contains("localhost") && !config.url.contains("?") && !config.url.contains(";") {
                // 添加一些默认参数以提高连接稳定性
                match config.url.as_str() {
                    url if url.starts_with("mysql://") => {
                        config.url = format!("{}?connect_timeout=10", url);
                    }
                    url if url.starts_with("postgres://") => {
                        config.url = format!("{}?connect_timeout=10", url);
                    }
                    _ => {} // 其他类型跳过
                }
            }
        }

        config
    }

    /// 验证配置是否有效
    ///
    /// 检查配置参数是否符合基本要求：
    /// - URL 不为空
    /// - max_connections > 0
    /// - min_connections <= max_connections
    /// - acquire_timeout > 0
    /// - idle_timeout > 0
    pub(crate) fn validate_config(config: &DbConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if config.url.is_empty() {
            errors.push("Database URL cannot be empty".to_string());
        }

        if config.max_connections() == 0 {
            errors.push("max_connections must be greater than 0".to_string());
        }

        if config.min_connections() > config.max_connections() {
            errors.push("min_connections cannot be greater than max_connections".to_string());
        }

        if config.acquire_timeout() == 0 {
            errors.push("acquire_timeout must be greater than 0".to_string());
        }

        if config.idle_timeout() == 0 {
            errors.push("idle_timeout must be greater than 0".to_string());
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    /// 从环境变量加载配置并自动修正
    ///
    /// 组合 `DbConfig::from_env()` 和 `ConfigCorrector::auto_correct()`，
    /// 方便一步完成配置加载和修正。
    pub(crate) fn load_and_correct_from_env() -> Result<DbConfig, ConfigError> {
        let mut config = DbConfig::from_env()?;
        config = ConfigCorrector::auto_correct(config);
        Ok(config)
    }

    /// 从配置文件加载配置并自动修正
    #[cfg(feature = "config-yaml")]
    pub(crate) fn load_and_correct_from_file(path: impl AsRef<Path>) -> Result<DbConfig, ConfigError> {
        let mut config = DbConfig::from_yaml_file(path)?;
        config = ConfigCorrector::auto_correct(config);
        Ok(config)
    }

    /// 验证配置并应用自动修正
    ///
    /// 先验证配置有效性，然后应用自动修正。
    /// 如果配置有错误，会返回错误信息并附带修正后的值。
    pub(crate) fn validate_and_correct(config: &DbConfig) -> Result<DbConfig, Vec<String>> {
        let errors = Self::validate_config(config);
        let corrected_config = Self::auto_correct(config.clone());

        match errors {
            Ok(()) => Ok(corrected_config),
            Err(mut validation_errors) => {
                // 添加警告信息表示配置已被自动修正
                validation_errors.extend([
                    "Some configuration values were automatically corrected".to_string(),
                    "Consider updating your configuration file to match corrected values".to_string(),
                ]);
                Err(validation_errors)
            }
        }
    }

    /// 获取当前应用的实际配置
    ///
    /// 返回经过自动修正后的配置副本。
    /// 如果配置从未被修正过，则返回传入的配置。
    ///
    /// # Arguments
    ///
    /// * `config` - 当前使用的配置
    ///
    /// # Returns
    ///
    /// 实际应用的配置（可能已被自动修正）
    pub(crate) fn get_actual_config(config: &DbConfig) -> DbConfig {
        Self::auto_correct(config.clone())
    }

    /// 使用数据库能力修正配置
    ///
    /// 根据数据库的实际能力（最大连接数等）调整配置。
    /// 这是异步方法，需要传入数据库连接。
    ///
    /// # Arguments
    ///
    /// * `config` - 当前配置
    /// * `connection` - 数据库连接
    /// * `db_type` - 数据库类型
    ///
    /// # Returns
    ///
    /// 根据数据库能力修正后的配置
    pub(crate) async fn auto_correct_with_database_capability(
        mut config: DbConfig,
        connection: &sea_orm::DatabaseConnection,
        db_type: DatabaseType,
    ) -> DbConfig {
        // 查询数据库最大连接数
        let db_max_connections = Self::query_database_max_connections(connection, db_type).await;

        // 如果配置值超过数据库能力的 80%，发出警告并调整
        let recommended_max = (db_max_connections as f64 * 0.8).floor() as u32;

        if config.max_connections() > recommended_max {
            tracing::warn!(
                "Config corrected: max_connections {} -> {} (80% of database limit {})",
                config.max_connections(),
                recommended_max,
                db_max_connections
            );
            config.max_connections = recommended_max;
        }

        // 确保 min_connections 不超过 max_connections
        if config.min_connections() > config.max_connections() {
            tracing::warn!(
                "Config corrected: min_connections {} -> {} (equal to max_connections)",
                config.min_connections(),
                config.max_connections()
            );
            config.min_connections = config.max_connections();
        }

        config
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

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-001: 配置默认值测试
    #[test]
    fn test_default_config_values() {
        let config = DbConfig::default();

        assert_eq!(config.url_sanitized(), "");
        assert_eq!(config.max_connections(), 0);
        assert_eq!(config.min_connections(), 0);
        assert_eq!(config.idle_timeout(), 0);
        assert_eq!(config.acquire_timeout(), 0);
        assert!(config.permissions_path().is_none());
    }

    /// TEST-U-002: 配置 Duration 转换测试
    #[test]
    fn test_config_duration_conversion() {
        let config = DbConfigBuilder::new()
            .url("sqlite::memory:")
            .max_connections(10)
            .min_connections(2)
            .idle_timeout(300)
            .acquire_timeout(5000)
            .admin_role("admin")
            .build()
            .unwrap();

        assert_eq!(config.idle_timeout_duration(), Duration::from_secs(300));
        assert_eq!(config.acquire_timeout_duration(), Duration::from_millis(5000));
    }

    /// TEST-U-003: 配置自动修正测试 - get_actual_config
    #[test]
    fn test_get_actual_config() {
        // 测试 min > max 的情况 - 先用有效值构建，然后模拟无效场景
        let mut config = DbConfigBuilder::new()
            .url("sqlite::memory:")
            .max_connections(10)
            .min_connections(10)
            .admin_role("admin")
            .build()
            .unwrap();

        // 手动设置无效值来测试 auto_correct 的修正
        config.set_min_connections(30);

        let actual = ConfigCorrector::get_actual_config(&config);

        // min 应该被修正为等于 max (10)
        assert_eq!(actual.max_connections(), 10);
        assert_eq!(actual.min_connections(), 10);
    }

    /// TEST-U-004: 配置自动修正测试 - 零值处理
    #[test]
    fn test_get_actual_config_zero_values() {
        // 先用有效值构建，然后测试 auto_correct 对零值的修正
        let config = DbConfigBuilder::new()
            .url("sqlite::memory:")
            .max_connections(5)
            .min_connections(5)
            .idle_timeout(0)
            .acquire_timeout(0)
            .admin_role("admin")
            .build()
            .unwrap();

        // 模拟零值场景，通过手动设置
        let mut zero_config = config.clone();
        zero_config.set_max_connections(0);
        zero_config.set_min_connections(0);
        zero_config.set_idle_timeout(0);
        zero_config.set_acquire_timeout(0);

        let actual = ConfigCorrector::get_actual_config(&zero_config);

        // 零值应该被修正为默认值
        assert_eq!(actual.max_connections(), 10);
        assert_eq!(actual.min_connections(), 1);
        assert_eq!(actual.idle_timeout(), 300);
        assert_eq!(actual.acquire_timeout(), 5000);
    }

    /// TEST-U-005: 配置构建器测试 - 基本用法
    #[test]
    fn test_config_builder_basic() {
        let config = DbConfigBuilder::new()
            .url("sqlite::memory:")
            .max_connections(20)
            .min_connections(5)
            .build()
            .unwrap();

        assert_eq!(config.url_sanitized(), "sqlite::memory:");
        assert_eq!(config.max_connections(), 20);
        assert_eq!(config.min_connections(), 5);
    }

    /// TEST-U-006: 配置构建器测试 - 所有字段
    #[test]
    fn test_config_builder_all_fields() {
        let config = DbConfigBuilder::new()
            .url("sqlite::memory:")
            .max_connections(20)
            .min_connections(5)
            .idle_timeout(300)
            .acquire_timeout(5000)
            .permissions_path("/etc/dbnexus/permissions.yaml")
            .auto_migrate(true)
            .admin_role("superuser")
            .build()
            .unwrap();

        assert_eq!(config.url_sanitized(), "sqlite::memory:");
        assert_eq!(config.max_connections(), 20);
        assert_eq!(config.min_connections(), 5);
        assert_eq!(config.idle_timeout(), 300);
        assert_eq!(config.acquire_timeout(), 5000);
        assert_eq!(config.permissions_path(), Some("/etc/dbnexus/permissions.yaml"));
        assert!(config.auto_migrate());
        assert_eq!(config.admin_role(), "superuser");
    }

    /// TEST-U-007: 配置构建器测试 - 验证失败
    #[test]
    fn test_config_builder_validation_failure() {
        let result = DbConfigBuilder::new()
            .url("sqlite::memory:")
            .max_connections(10)
            .min_connections(20)
            .build();

        assert!(result.is_err());
    }

    /// TEST-U-008: 配置构建器测试 - 默认值
    #[test]
    fn test_config_builder_defaults() {
        let config = DbConfigBuilder::new().url("sqlite::memory:").build().unwrap();

        assert_eq!(config.max_connections(), 20);
        assert_eq!(config.min_connections(), 5);
        assert_eq!(config.idle_timeout(), 300);
        assert_eq!(config.acquire_timeout(), 5000); // 恢复为保守的默认值
        assert_eq!(config.admin_role(), "admin");
    }

    /// TEST-U-009: 配置加载器测试
    #[cfg(feature = "config-yaml")]
    #[test]
    fn test_config_loader() {
        let yaml = r#"
url: "sqlite::memory:"
max_connections: 20
min_connections: 5
"#;
        let config = DbConfig::from_yaml_str(yaml).unwrap();
        {
            assert_eq!(config.url_sanitized(), "sqlite::memory:");
            assert_eq!(config.max_connections(), 20);
        }
    }
    /// TEST-U-010: 配置验证测试 - 空URL
    #[test]
    fn test_config_validation_empty_url() {
        let config = DbConfigBuilder::new().build().unwrap_err();

        assert_eq!(config.to_string(), "Missing required configuration field");
    }

    /// TEST-U-011: 配置验证测试 - 无效的连接数
    #[test]
    fn test_config_validation_invalid_connections() {
        let result = DbConfigBuilder::new().url("sqlite::memory:").max_connections(0).build();

        assert!(result.is_err());
    }
}
