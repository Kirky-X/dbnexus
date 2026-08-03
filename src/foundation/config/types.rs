// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 配置类型定义
//!
//! 纯数据结构，配置加载通过 serde 直接反序列化

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

    /// 无效缓存容量
    #[error("Invalid cache capacity: {0}")]
    InvalidCacheCapacity(String),

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

fn default_policy_cache_capacity() -> u64 {
    4096
}
fn default_sql_parse_cache_capacity() -> u64 {
    1000
}
fn default_query_cache_capacity() -> u64 {
    10000
}
fn default_cache_ttl() -> u64 {
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
    /// 验证缓存配置有效性
    ///
    /// 所有 capacity 字段必须 > 0。
    ///
    /// # Errors
    ///
    /// 任何 capacity 为 0 时返回 `ConfigError::InvalidCacheCapacity`
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.policy_cache_capacity == 0 {
            return Err(ConfigError::InvalidCacheCapacity(
                "policy_cache_capacity must be > 0".to_string(),
            ));
        }
        if self.sql_parse_cache_capacity == 0 {
            return Err(ConfigError::InvalidCacheCapacity(
                "sql_parse_cache_capacity must be > 0".to_string(),
            ));
        }
        if self.query_cache_capacity == 0 {
            return Err(ConfigError::InvalidCacheCapacity(
                "query_cache_capacity must be > 0".to_string(),
            ));
        }
        Ok(())
    }

    /// 获取默认 TTL Duration
    pub fn default_ttl_duration(&self) -> Duration {
        Duration::from_secs(self.default_ttl)
    }

    /// 从 YAML 字符串加载配置
    ///
    /// 使用 `serde_yaml_ng` 直接反序列化，缺失字段使用 serde 默认值。
    ///
    /// # Errors
    ///
    /// 如果 YAML 格式无效或字段类型不匹配，返回解析错误
    #[cfg(feature = "yaml")]
    pub fn from_yaml_str(yaml: &str) -> Result<Self, serde_yaml_ng::Error> {
        serde_yaml_ng::from_str(yaml)
    }

    /// 从 `serde_json::Value` 加载配置
    ///
    /// # Errors
    ///
    /// 如果 JSON 结构无法反序列化为 `CacheConfig`，返回解析错误
    pub fn from_json_value(v: serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(v)
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
    /// 验证连接池配置有效性
    ///
    /// # Errors
    ///
    /// - `max_connections == 0` 或 `acquire_timeout == 0` 返回 `ConfigError::InvalidValue`
    /// - `min_connections > max_connections` 返回 `ConfigError::InvalidValue`
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_connections == 0 {
            return Err(ConfigError::InvalidValue {
                key: "max_connections".to_string(),
                message: "max_connections must be > 0".to_string(),
            });
        }
        if self.min_connections > self.max_connections {
            return Err(ConfigError::InvalidValue {
                key: "min_connections".to_string(),
                message: format!(
                    "min_connections ({}) must be <= max_connections ({})",
                    self.min_connections, self.max_connections
                ),
            });
        }
        if self.acquire_timeout == 0 {
            return Err(ConfigError::InvalidValue {
                key: "acquire_timeout".to_string(),
                message: "acquire_timeout must be > 0".to_string(),
            });
        }
        Ok(())
    }

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DatabaseType {
    /// PostgreSQL
    Postgres,
    /// MySQL
    MySql,
    /// SQLite
    #[default]
    Sqlite,
    /// DuckDB（嵌入式分析型数据库，0.3.0 新增）
    DuckDb,
    /// Ladybug（嵌入式图数据库，0.4.0 新增）
    Ladybug,
    /// Neo4j（图数据库服务器，0.4.0 新增）
    Neo4j,
}

impl DatabaseType {
    /// 从 URL 解析数据库类型
    ///
    /// 使用 `url` crate 解析连接串，支持 `sqlite`/`sqlite3`/`postgres`/`postgresql`/`mysql`/`duckdb`/
    /// `lbug`/`ladybug`/`neo4j`/`neo4j+s`/`neo4j+ssc` scheme。
    /// 未知 scheme 返回 `Err(DbNexusError::UnsupportedDatabaseScheme)`。
    ///
    /// # Errors
    ///
    /// - URL 解析失败（无 scheme）
    /// - 未知数据库协议
    pub fn from_url(url: &str) -> Result<Self, crate::error::DbNexusError> {
        // 处理 SQLite 特殊格式 sqlite::memory: / sqlite3::memory:
        let lower = url.to_lowercase();
        if lower == "sqlite::memory:" || lower.starts_with("sqlite://") || lower.starts_with("sqlite3://") {
            return Ok(DatabaseType::Sqlite);
        }
        if lower.starts_with("duckdb:") {
            return Ok(DatabaseType::DuckDb);
        }
        // 处理 Ladybug 图数据库格式 lbug: / ladybug:
        if lower.starts_with("lbug:") || lower.starts_with("ladybug:") {
            return Ok(DatabaseType::Ladybug);
        }
        // 处理 Neo4j 图数据库格式 neo4j: / neo4j+s: / neo4j+ssc:
        if lower.starts_with("neo4j:") || lower.starts_with("neo4j+s:") || lower.starts_with("neo4j+ssc:") {
            return Ok(DatabaseType::Neo4j);
        }

        let parsed = url::Url::parse(url).map_err(|_| {
            crate::error::DbNexusError::UnsupportedDatabaseScheme(format!("failed to parse URL: {url}"))
        })?;

        match parsed.scheme() {
            "sqlite" | "sqlite3" => Ok(DatabaseType::Sqlite),
            "postgres" | "postgresql" => Ok(DatabaseType::Postgres),
            "mysql" => Ok(DatabaseType::MySql),
            "duckdb" => Ok(DatabaseType::DuckDb),
            "lbug" | "ladybug" => Ok(DatabaseType::Ladybug),
            "neo4j" | "neo4j+s" | "neo4j+ssc" => Ok(DatabaseType::Neo4j),
            other => Err(crate::error::DbNexusError::UnsupportedDatabaseScheme(format!(
                "'{other}' is not a supported database scheme"
            ))),
        }
    }

    /// 从 URL 解析数据库类型（别名）
    ///
    /// # Errors
    ///
    /// 同 [`from_url`](Self::from_url)
    pub fn parse_database_type(url: &str) -> Result<Self, crate::error::DbNexusError> {
        Self::from_url(url)
    }

    /// 获取数据库类型的显示名称
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseType::Postgres => "postgres",
            DatabaseType::MySql => "mysql",
            DatabaseType::Sqlite => "sqlite",
            DatabaseType::DuckDb => "duckdb",
            DatabaseType::Ladybug => "ladybug",
            DatabaseType::Neo4j => "neo4j",
        }
    }

    /// 检查是否为嵌入式数据库（SQLite / DuckDB）
    ///
    /// 0.3.0 新增：取代 `is_real_database()` 的二分法，更清晰地表达数据库部署模式。
    /// 嵌入式数据库运行在进程内，无需独立服务器；服务器端数据库（Postgres/MySQL）需要独立进程。
    ///
    /// 注意：图数据库（Ladybug/Neo4j）不属于此分类，请使用 [`is_graph()`](Self::is_graph) 判断。
    pub fn is_embedded(&self) -> bool {
        matches!(self, DatabaseType::Sqlite | DatabaseType::DuckDb)
    }

    /// 检查是否为服务器端数据库（PostgreSQL / MySQL）
    ///
    /// 0.3.0 新增：与 `is_embedded()` 互补。
    ///
    /// 注意：图数据库（Ladybug/Neo4j）不属于此分类，请使用 [`is_graph()`](Self::is_graph) 判断。
    pub fn is_server_side(&self) -> bool {
        matches!(self, DatabaseType::Postgres | DatabaseType::MySql)
    }

    /// 检查是否为图数据库（Ladybug / Neo4j）
    ///
    /// 0.4.0 新增：图数据库使用 Cypher 查询语言和图遍历模型，
    /// 与关系型数据库（SQL）的表/行/列模型完全不同。
    /// Ladybug 是嵌入式图数据库，Neo4j 是服务器端图数据库。
    pub fn is_graph(&self) -> bool {
        matches!(self, DatabaseType::Ladybug | DatabaseType::Neo4j)
    }

    /// 检查是否为真实数据库（非 SQLite 内存数据库）
    ///
    /// # Deprecated
    ///
    /// 此方法语义模糊（"真实"数据库 vs "内存"数据库 vs "嵌入式"数据库），
    /// 0.3.0 推荐使用 [`is_embedded()`](Self::is_embedded) 或 [`is_server_side()`](Self::is_server_side) 替代。
    /// 0.4.0 新增 [`is_graph()`](Self::is_graph) 用于判断图数据库。
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

// 数据库配置
//
// 纯数据结构，通过 `from_yaml_str()` / `from_json_str()` 直接反序列化加载
//
// # 配置字段
//
// | 字段 | 默认值 |
// |------|--------|
// | `url` | **必填** |
// | `pool_config` (flatten) | `PoolConfig::default()` |
// ============================================================================
// 故障转移配置
// ============================================================================

/// 连接故障转移配置
///
/// 定义故障转移链：当主库不可用时自动切换到备用 URL。
/// 与 CircuitBreaker 协同工作，连续失败达到阈值时触发切换。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct FailoverConfig {
    /// 有序 URL 列表：[primary, replica1, replica2, ...]
    /// 第一个为 primary，后续为故障转移目标
    pub urls: Vec<String>,
    /// 自定义健康检查 SQL（默认 `SELECT 1`）
    #[serde(default)]
    pub health_check_query: Option<String>,
    /// 连续失败 N 次触发故障转移，默认 3
    #[serde(default = "default_failover_threshold")]
    pub failover_threshold: u32,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            urls: Vec::new(),
            health_check_query: None,
            failover_threshold: default_failover_threshold(),
        }
    }
}

#[allow(dead_code)]
fn default_failover_threshold() -> u32 {
    3
}

// ============================================================================
// 副本路由配置
// ============================================================================

/// 副本路由配置
///
/// 基于复制 lag 检测的读写分离配置。
/// 当副本延迟超过阈值时自动回退到主库。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ReplicaConfig {
    /// 副本数据库 URL 列表
    pub replica_urls: Vec<String>,
    /// 最大允许复制延迟（秒），超过则回退主库，默认 5.0
    #[serde(default = "default_max_lag_seconds")]
    pub max_lag_seconds: f64,
    /// Lag 检测间隔（秒），默认 10
    #[serde(default = "default_lag_check_interval")]
    pub lag_check_interval_secs: u64,
}

impl Default for ReplicaConfig {
    fn default() -> Self {
        Self {
            replica_urls: Vec::new(),
            max_lag_seconds: default_max_lag_seconds(),
            lag_check_interval_secs: default_lag_check_interval(),
        }
    }
}

#[allow(dead_code)]
fn default_max_lag_seconds() -> f64 {
    5.0
}
#[allow(dead_code)]
fn default_lag_check_interval() -> u64 {
    10
}

/// 数据库配置
///
/// 纯数据结构，通过 `from_yaml_str()` / `from_json_str()` 直接反序列化加载
///
/// # 配置字段
///
/// | 字段 | 默认值 |
/// |------|--------|
/// | `url` | **必填** |
/// | `pool_config` (flatten) | `PoolConfig::default()` |
/// | `admin_role` | "admin" |
/// | `permissions_path` | None |
/// | `migrations_dir` | None |
/// | `auto_migrate` | false |
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    /// 数据库连接 URL
    pub url: String,

    /// 连接池配置（通过 `#[serde(flatten)]` 扁平化，保持序列化向后兼容）
    #[serde(flatten)]
    pub pool_config: PoolConfig,

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

    /// 运行时重试策略（`retry` feature 启用时可用）
    #[cfg(feature = "retry")]
    #[serde(default)]
    pub retry_policy: Option<crate::reliability::RetryPolicy>,

    /// 连接故障转移配置（`failover` feature 启用时可用）
    #[cfg(feature = "failover")]
    #[serde(default)]
    pub failover_config: Option<FailoverConfig>,

    /// 副本路由配置（`replica-routing` feature 启用时可用）
    #[cfg(feature = "replica-routing")]
    #[serde(default)]
    pub replica_config: Option<ReplicaConfig>,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            pool_config: PoolConfig::default(),
            permissions_path: None,
            migrations_dir: None,
            auto_migrate: false,
            migration_timeout: default_migration_timeout(),
            admin_role: default_admin_role(),
            warmup_timeout: default_warmup_timeout(),
            warmup_retries: default_warmup_retries(),
            cache_config: CacheConfig::default(),
            #[cfg(feature = "retry")]
            retry_policy: None,
            #[cfg(feature = "failover")]
            failover_config: None,
            #[cfg(feature = "replica-routing")]
            replica_config: None,
        }
    }
}

fn default_admin_role() -> String {
    "admin".to_string()
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

/// 解析环境变量为 u32，变量不存在时返回默认值，存在但解析失败时返回错误
#[cfg(feature = "config-env")]
fn parse_env_u32(key: &str, default: u32) -> Result<u32, ConfigError> {
    match std::env::var(key) {
        Ok(val) => val.parse::<u32>().map_err(|_| ConfigError::InvalidValue {
            key: key.to_string(),
            message: format!("expected u32, got '{val}'"),
        }),
        Err(_) => Ok(default),
    }
}

/// 解析环境变量为 u64，变量不存在时返回默认值，存在但解析失败时返回错误
#[cfg(feature = "config-env")]
fn parse_env_u64(key: &str, default: u64) -> Result<u64, ConfigError> {
    match std::env::var(key) {
        Ok(val) => val.parse::<u64>().map_err(|_| ConfigError::InvalidValue {
            key: key.to_string(),
            message: format!("expected u64, got '{val}'"),
        }),
        Err(_) => Ok(default),
    }
}

impl DbConfig {
    /// 从环境变量加载配置
    ///
    /// 支持的环境变量：
    /// - `DATABASE_URL`: 数据库连接 URL（必需）
    /// - `DB_MAX_CONNECTIONS`: 最大连接数（默认 20）
    /// - `DB_MIN_CONNECTIONS`: 最小连接数（默认 5）
    /// - `DB_IDLE_TIMEOUT`: 空闲超时秒数（默认 300）
    /// - `DB_ACQUIRE_TIMEOUT`: 获取连接超时毫秒数（默认 5000）
    /// - `DB_ADMIN_ROLE`: 管理员角色名（默认 "admin"）
    /// - `DB_PERMISSIONS_PATH`: 权限配置文件路径
    /// - `DB_MIGRATIONS_DIR`: 迁移文件目录
    /// - `DB_AUTO_MIGRATE`: 是否启用自动迁移（默认 false）
    /// - `DB_MIGRATION_TIMEOUT`: 迁移超时秒数（默认 60）
    #[cfg(feature = "config-env")]
    pub fn from_env() -> Result<Self, ConfigError> {
        let url = std::env::var("DATABASE_URL").map_err(|_| ConfigError::MissingUrl)?;

        Ok(Self {
            url,
            pool_config: PoolConfig {
                max_connections: parse_env_u32("DB_MAX_CONNECTIONS", 20)?,
                min_connections: parse_env_u32("DB_MIN_CONNECTIONS", 5)?,
                idle_timeout: parse_env_u64("DB_IDLE_TIMEOUT", 300)?,
                acquire_timeout: parse_env_u64("DB_ACQUIRE_TIMEOUT", 5000)?,
            },
            admin_role: std::env::var("DB_ADMIN_ROLE").unwrap_or_else(|_| "admin".to_string()),
            permissions_path: std::env::var("DB_PERMISSIONS_PATH").ok(),
            migrations_dir: std::env::var("DB_MIGRATIONS_DIR").ok().map(PathBuf::from),
            auto_migrate: std::env::var("DB_AUTO_MIGRATE")
                .ok()
                .map(|s| s.to_lowercase() == "true")
                .unwrap_or(false),
            migration_timeout: parse_env_u64("DB_MIGRATION_TIMEOUT", 60)?,
            warmup_timeout: parse_env_u64("DB_WARMUP_TIMEOUT", 30)?,
            warmup_retries: parse_env_u32("DB_WARMUP_RETRIES", 3)?,
            cache_config: CacheConfig::default(),
            #[cfg(feature = "retry")]
            retry_policy: None,
            #[cfg(feature = "failover")]
            failover_config: None,
            #[cfg(feature = "replica-routing")]
            replica_config: None,
        })
    }

    /// 从 YAML 字符串加载配置
    ///
    /// 使用 `serde_yaml_ng` 直接反序列化，缺失字段使用 serde 默认值。
    /// `cache_config` 子结构通过 `#[serde(default)]` 嵌入，serde 会自动反序列化
    /// `cache_config:` 子节点（缺失时回退到 `CacheConfig::default()`）。
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dbnexus::DbConfig;
    ///
    /// let yaml = r#"
    /// url: "sqlite::memory:"
    /// max_connections: 20
    /// "#;
    /// let config = DbConfig::from_yaml_str(yaml)?;
    /// ```
    ///
    /// # Errors
    ///
    /// 如果 YAML 格式无效或字段类型不匹配，返回解析错误
    #[cfg(feature = "yaml")]
    pub fn from_yaml_str(yaml: &str) -> Result<Self, serde_yaml_ng::Error> {
        serde_yaml_ng::from_str(yaml)
    }

    /// 从 JSON 字符串加载配置
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dbnexus::DbConfig;
    ///
    /// let json = r#"{"url":"sqlite::memory:","max_connections":20}"#;
    /// let config = DbConfig::from_json_str(json)?;
    /// ```
    ///
    /// # Errors
    ///
    /// 如果 JSON 格式无效或字段类型不匹配，返回解析错误
    pub fn from_json_str(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 获取数据库类型
    ///
    /// # Errors
    ///
    /// URL 解析失败或未知协议时返回 `DbNexusError::UnsupportedDatabaseScheme`
    pub fn database_type(&self) -> Result<DatabaseType, crate::error::DbNexusError> {
        DatabaseType::from_url(&self.url)
    }

    /// 获取空闲超时 Duration（委托到 pool_config）
    pub fn idle_timeout_duration(&self) -> Duration {
        self.pool_config.idle_timeout_duration()
    }

    /// 获取获取超时 Duration（委托到 pool_config）
    pub fn acquire_timeout_duration(&self) -> Duration {
        self.pool_config.acquire_timeout_duration()
    }

    /// 获取迁移超时 Duration
    pub fn migration_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.migration_timeout)
    }

    /// 获取缓存配置
    pub fn cache_config(&self) -> &CacheConfig {
        &self.cache_config
    }

    /// 验证数据库配置有效性
    ///
    /// 委托调用 `CacheConfig::validate()` 并验证连接池字段。
    ///
    /// # Errors
    ///
    /// - 缓存容量为 0 时返回 `ConfigError::InvalidCacheCapacity`
    /// - `max_connections == 0` 或 `min_connections > max_connections` 返回 `ConfigError::InvalidValue`
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.cache_config.validate()?;
        self.pool_config.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ConfigError Display 测试 =====

    #[test]
    fn test_config_error_display_variants() {
        assert_eq!(
            ConfigError::MissingField("url".into()).to_string(),
            "Missing required configuration: url"
        );
        assert_eq!(
            ConfigError::MissingUrl.to_string(),
            "Missing required configuration: dbnexus.url"
        );
        assert_eq!(
            ConfigError::InvalidCacheCapacity("negative".into()).to_string(),
            "Invalid cache capacity: negative"
        );
        assert_eq!(
            ConfigError::InvalidValue {
                key: "max".into(),
                message: "too large".into(),
            }
            .to_string(),
            "Invalid configuration value for 'max': too large"
        );
        assert_eq!(
            ConfigError::InvalidFormat("yaml".into()).to_string(),
            "Invalid configuration format: yaml"
        );
        assert_eq!(
            ConfigError::FileNotFound("/tmp/cfg".into()).to_string(),
            "Configuration file not found: /tmp/cfg"
        );
        assert_eq!(
            ConfigError::IoError("read fail".into()).to_string(),
            "IO error: read fail"
        );
        assert_eq!(ConfigError::InvalidUrl("bad".into()).to_string(), "Invalid URL: bad");
        assert_eq!(
            ConfigError::UnsupportedProtocol("ftp".into()).to_string(),
            "Unsupported database protocol: ftp"
        );
        assert_eq!(
            ConfigError::ParseError("syntax".into()).to_string(),
            "Parse error: syntax"
        );
        assert_eq!(
            ConfigError::ValidationError("bad value".into()).to_string(),
            "Validation error: bad value"
        );
    }

    // ===== CacheConfig 测试 =====

    #[test]
    fn test_cache_config_default() {
        let cfg = CacheConfig::default();
        assert_eq!(cfg.policy_cache_capacity, 4096);
        assert_eq!(cfg.sql_parse_cache_capacity, 1000);
        assert_eq!(cfg.query_cache_capacity, 10000);
        assert_eq!(cfg.default_ttl, 300);
    }

    #[test]
    fn test_cache_config_default_ttl_duration() {
        let cfg = CacheConfig::default();
        assert_eq!(cfg.default_ttl_duration(), Duration::from_secs(300));
    }

    #[test]
    fn test_cache_config_serde_roundtrip() {
        let cfg = CacheConfig {
            policy_cache_capacity: 100,
            sql_parse_cache_capacity: 200,
            query_cache_capacity: 300,
            default_ttl: 60,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: CacheConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.policy_cache_capacity, 100);
        assert_eq!(deserialized.sql_parse_cache_capacity, 200);
        assert_eq!(deserialized.query_cache_capacity, 300);
        assert_eq!(deserialized.default_ttl, 60);
    }

    #[test]
    fn test_cache_config_serde_defaults_applied() {
        // 空 JSON 应使用 serde default 函数
        let json = r#"{}"#;
        let cfg: CacheConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.policy_cache_capacity, 4096);
        assert_eq!(cfg.sql_parse_cache_capacity, 1000);
        assert_eq!(cfg.query_cache_capacity, 10000);
        assert_eq!(cfg.default_ttl, 300);
    }

    #[test]
    fn test_cache_config_validate_accepts_valid() {
        assert!(CacheConfig::default().validate().is_ok());
    }

    #[test]
    fn test_cache_config_validate_rejects_zero_policy_capacity() {
        let cfg = CacheConfig {
            policy_cache_capacity: 0,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(
            err.to_string().contains("policy_cache_capacity"),
            "error should mention field name, got: {err}"
        );
    }

    #[test]
    fn test_cache_config_validate_rejects_zero_sql_parse_capacity() {
        let cfg = CacheConfig {
            sql_parse_cache_capacity: 0,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("sql_parse_cache_capacity"));
    }

    #[test]
    fn test_cache_config_validate_rejects_zero_query_capacity() {
        let cfg = CacheConfig {
            query_cache_capacity: 0,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("query_cache_capacity"));
    }

    // ===== PoolConfig 测试 =====

    #[test]
    fn test_pool_config_default() {
        let cfg = PoolConfig::default();
        assert_eq!(cfg.max_connections, 20);
        assert_eq!(cfg.min_connections, 5);
        assert_eq!(cfg.idle_timeout, 300);
        assert_eq!(cfg.acquire_timeout, 5000);
    }

    #[test]
    fn test_pool_config_duration_methods() {
        let cfg = PoolConfig::default();
        assert_eq!(cfg.idle_timeout_duration(), Duration::from_secs(300));
        assert_eq!(cfg.acquire_timeout_duration(), Duration::from_millis(5000));
    }

    #[test]
    fn test_pool_config_serde_defaults_applied() {
        let json = r#"{}"#;
        let cfg: PoolConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.max_connections, 20);
        assert_eq!(cfg.min_connections, 5);
        assert_eq!(cfg.idle_timeout, 300);
        assert_eq!(cfg.acquire_timeout, 5000);
    }

    #[test]
    fn test_pool_config_validate_accepts_valid() {
        assert!(PoolConfig::default().validate().is_ok());
    }

    #[test]
    fn test_pool_config_validate_rejects_zero_max_connections() {
        let cfg = PoolConfig {
            max_connections: 0,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("max_connections"));
    }

    #[test]
    fn test_pool_config_validate_rejects_min_greater_than_max() {
        let cfg = PoolConfig {
            min_connections: 10,
            max_connections: 5,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("min_connections"));
    }

    #[test]
    fn test_pool_config_validate_rejects_zero_acquire_timeout() {
        let cfg = PoolConfig {
            acquire_timeout: 0,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("acquire_timeout"));
    }

    // ===== DatabaseType 测试 =====

    #[test]
    fn test_database_type_from_url_postgres() {
        assert_eq!(
            DatabaseType::from_url("postgres://localhost/db").unwrap(),
            DatabaseType::Postgres
        );
        assert_eq!(
            DatabaseType::from_url("postgresql://localhost/db").unwrap(),
            DatabaseType::Postgres
        );
        // 大小写不敏感（url crate 自动处理 scheme 小写化）
        assert_eq!(
            DatabaseType::from_url("POSTGRES://localhost/db").unwrap(),
            DatabaseType::Postgres
        );
    }

    #[test]
    fn test_database_type_from_url_mysql() {
        assert_eq!(
            DatabaseType::from_url("mysql://localhost/db").unwrap(),
            DatabaseType::MySql
        );
        assert_eq!(
            DatabaseType::from_url("MYSQL://localhost/db").unwrap(),
            DatabaseType::MySql
        );
    }

    #[test]
    fn test_database_type_from_url_sqlite() {
        assert_eq!(DatabaseType::from_url("sqlite::memory:").unwrap(), DatabaseType::Sqlite);
        assert_eq!(
            DatabaseType::from_url("sqlite://test.db").unwrap(),
            DatabaseType::Sqlite
        );
    }

    #[test]
    fn test_database_type_from_url_duckdb() {
        assert_eq!(DatabaseType::from_url("duckdb::memory:").unwrap(), DatabaseType::DuckDb);
        assert_eq!(
            DatabaseType::from_url("duckdb://test.db").unwrap(),
            DatabaseType::DuckDb
        );
        assert_eq!(DatabaseType::from_url("duckdb:test.ddb").unwrap(), DatabaseType::DuckDb);
    }

    #[test]
    fn test_database_type_from_url_ladybug() {
        // lbug: scheme（短别名）
        assert_eq!(
            DatabaseType::from_url("lbug://test.lbug").unwrap(),
            DatabaseType::Ladybug
        );
        assert_eq!(DatabaseType::from_url("lbug:test.lbug").unwrap(), DatabaseType::Ladybug);
        // ladybug: scheme（完整名）
        assert_eq!(
            DatabaseType::from_url("ladybug://test.lbug").unwrap(),
            DatabaseType::Ladybug
        );
        // 大小写不敏感
        assert_eq!(
            DatabaseType::from_url("LBUG://test.lbug").unwrap(),
            DatabaseType::Ladybug
        );
        assert_eq!(
            DatabaseType::from_url("Ladybug://test.lbug").unwrap(),
            DatabaseType::Ladybug
        );
    }

    #[test]
    fn test_database_type_from_url_neo4j() {
        // neo4j: scheme（明文）
        assert_eq!(
            DatabaseType::from_url("neo4j://user:pass@localhost:7687").unwrap(),
            DatabaseType::Neo4j
        );
        // neo4j+s: scheme（TLS）
        assert_eq!(
            DatabaseType::from_url("neo4j+s://user:pass@host:7687").unwrap(),
            DatabaseType::Neo4j
        );
        // neo4j+ssc: scheme（自签名 TLS）
        assert_eq!(
            DatabaseType::from_url("neo4j+ssc://user:pass@host:7687").unwrap(),
            DatabaseType::Neo4j
        );
        // 大小写不敏感
        assert_eq!(
            DatabaseType::from_url("NEO4J://localhost:7687").unwrap(),
            DatabaseType::Neo4j
        );
    }

    #[test]
    fn test_database_type_from_url_unknown_scheme_returns_error() {
        // 未知 scheme 现在返回错误（不再默认 SQLite）
        assert!(DatabaseType::from_url("unknown://foo").is_err());
        // 空字符串无法解析为 URL，返回错误
        assert!(DatabaseType::from_url("").is_err());
        // 无 scheme 的相对路径返回错误
        assert!(DatabaseType::from_url("/path/to/db.db").is_err());
    }

    #[test]
    fn test_database_type_parse_database_type_alias() {
        assert_eq!(
            DatabaseType::parse_database_type("mysql://x").unwrap(),
            DatabaseType::MySql
        );
    }

    #[test]
    fn test_database_type_as_str() {
        assert_eq!(DatabaseType::Postgres.as_str(), "postgres");
        assert_eq!(DatabaseType::MySql.as_str(), "mysql");
        assert_eq!(DatabaseType::Sqlite.as_str(), "sqlite");
        assert_eq!(DatabaseType::DuckDb.as_str(), "duckdb");
        assert_eq!(DatabaseType::Ladybug.as_str(), "ladybug");
        assert_eq!(DatabaseType::Neo4j.as_str(), "neo4j");
    }

    #[test]
    fn test_database_type_is_embedded() {
        assert!(DatabaseType::Sqlite.is_embedded());
        assert!(DatabaseType::DuckDb.is_embedded());
        assert!(!DatabaseType::Postgres.is_embedded());
        assert!(!DatabaseType::MySql.is_embedded());
        // 图 DB 不属于嵌入式关系型数据库
        assert!(!DatabaseType::Ladybug.is_embedded());
        assert!(!DatabaseType::Neo4j.is_embedded());
    }

    #[test]
    fn test_database_type_is_server_side() {
        assert!(DatabaseType::Postgres.is_server_side());
        assert!(DatabaseType::MySql.is_server_side());
        assert!(!DatabaseType::Sqlite.is_server_side());
        assert!(!DatabaseType::DuckDb.is_server_side());
        // 图 DB 不属于服务器端关系型数据库
        assert!(!DatabaseType::Ladybug.is_server_side());
        assert!(!DatabaseType::Neo4j.is_server_side());
    }

    #[test]
    fn test_database_type_is_graph() {
        assert!(DatabaseType::Ladybug.is_graph());
        assert!(DatabaseType::Neo4j.is_graph());
        // 关系型数据库不是图数据库
        assert!(!DatabaseType::Postgres.is_graph());
        assert!(!DatabaseType::MySql.is_graph());
        assert!(!DatabaseType::Sqlite.is_graph());
        assert!(!DatabaseType::DuckDb.is_graph());
    }

    #[test]
    fn test_database_type_is_real_database() {
        assert!(DatabaseType::Postgres.is_real_database());
        assert!(DatabaseType::MySql.is_real_database());
        assert!(!DatabaseType::Sqlite.is_real_database());
        assert!(!DatabaseType::DuckDb.is_real_database());
        // 图 DB 不属于 is_real_database（deprecated 方法的原有语义：服务器端关系型）
        assert!(!DatabaseType::Ladybug.is_real_database());
        assert!(!DatabaseType::Neo4j.is_real_database());
    }

    #[test]
    fn test_database_type_display() {
        assert_eq!(DatabaseType::Postgres.to_string(), "postgres");
        assert_eq!(DatabaseType::MySql.to_string(), "mysql");
        assert_eq!(DatabaseType::Sqlite.to_string(), "sqlite");
        assert_eq!(DatabaseType::DuckDb.to_string(), "duckdb");
        assert_eq!(DatabaseType::Ladybug.to_string(), "ladybug");
        assert_eq!(DatabaseType::Neo4j.to_string(), "neo4j");
    }

    #[test]
    fn test_database_type_default_is_sqlite() {
        let db_type = DatabaseType::default();
        assert!(matches!(db_type, DatabaseType::Sqlite));
    }

    #[test]
    fn test_database_type_serde_round_trip() {
        let cases = [
            DatabaseType::Sqlite,
            DatabaseType::Postgres,
            DatabaseType::MySql,
            DatabaseType::DuckDb,
            DatabaseType::Ladybug,
            DatabaseType::Neo4j,
        ];
        for original in cases {
            let json = serde_json::to_string(&original).expect("serialize should succeed");
            let restored: DatabaseType = serde_json::from_str(&json).expect("deserialize should succeed");
            assert_eq!(original, restored, "round-trip failed for {:?}", original);
        }
    }

    // ===== DbConfig 测试 =====

    #[test]
    fn test_db_config_default() {
        let cfg = DbConfig::default();
        assert_eq!(cfg.url, String::new());
        assert_eq!(cfg.pool_config.max_connections, 20);
        assert_eq!(cfg.pool_config.min_connections, 5);
        assert_eq!(cfg.pool_config.idle_timeout, 300);
        assert_eq!(cfg.pool_config.acquire_timeout, 5000);
        assert_eq!(cfg.admin_role, "admin");
        assert_eq!(cfg.migration_timeout, 60);
        assert_eq!(cfg.warmup_timeout, 30);
        assert_eq!(cfg.warmup_retries, 3);
        assert!(!cfg.auto_migrate);
        assert!(cfg.permissions_path.is_none());
        assert!(cfg.migrations_dir.is_none());
        assert_eq!(cfg.cache_config.policy_cache_capacity, 4096);
    }

    #[test]
    fn test_db_config_database_type() {
        let cfg = DbConfig {
            url: "postgres://localhost/db".into(),
            ..Default::default()
        };
        assert_eq!(cfg.database_type().unwrap(), DatabaseType::Postgres);

        let cfg = DbConfig {
            url: "mysql://localhost/db".into(),
            ..Default::default()
        };
        assert_eq!(cfg.database_type().unwrap(), DatabaseType::MySql);

        let cfg = DbConfig {
            url: "sqlite::memory:".into(),
            ..Default::default()
        };
        assert_eq!(cfg.database_type().unwrap(), DatabaseType::Sqlite);
    }

    #[test]
    fn test_db_config_duration_methods() {
        let cfg = DbConfig::default();
        assert_eq!(cfg.idle_timeout_duration(), Duration::from_secs(300));
        assert_eq!(cfg.acquire_timeout_duration(), Duration::from_millis(5000));
        assert_eq!(cfg.migration_timeout_duration(), Duration::from_secs(60));
    }

    #[test]
    fn test_db_config_cache_config_ref() {
        let cfg = DbConfig::default();
        let cache = cfg.cache_config();
        assert_eq!(cache.default_ttl, 300);
    }

    #[test]
    fn test_db_config_validate_delegates_to_cache_and_pool() {
        // Default should be valid
        assert!(DbConfig::default().validate().is_ok());

        // Invalid cache config should fail
        let cfg = DbConfig {
            cache_config: CacheConfig {
                policy_cache_capacity: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());

        // Invalid pool config should fail
        let cfg = DbConfig {
            pool_config: PoolConfig {
                max_connections: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());

        // min > max should fail
        let cfg = DbConfig {
            pool_config: PoolConfig {
                min_connections: 100,
                max_connections: 5,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_db_config_serde_roundtrip() {
        let cfg = DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: PoolConfig {
                max_connections: 10,
                min_connections: 2,
                idle_timeout: 100,
                acquire_timeout: 3000,
            },
            permissions_path: Some("/tmp/perms.yaml".into()),
            migrations_dir: Some(PathBuf::from("/tmp/migrations")),
            auto_migrate: true,
            migration_timeout: 120,
            admin_role: "root".to_string(),
            warmup_timeout: 15,
            warmup_retries: 5,
            cache_config: CacheConfig {
                policy_cache_capacity: 512,
                sql_parse_cache_capacity: 256,
                query_cache_capacity: 1024,
                default_ttl: 60,
            },
            #[cfg(feature = "retry")]
            retry_policy: None,
            #[cfg(feature = "failover")]
            failover_config: None,
            #[cfg(feature = "replica-routing")]
            replica_config: None,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: DbConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.url, "sqlite::memory:");
        assert_eq!(deserialized.pool_config.max_connections, 10);
        assert_eq!(deserialized.pool_config.min_connections, 2);
        assert_eq!(deserialized.pool_config.idle_timeout, 100);
        assert_eq!(deserialized.pool_config.acquire_timeout, 3000);
        assert_eq!(deserialized.permissions_path, Some("/tmp/perms.yaml".to_string()));
        assert_eq!(deserialized.migrations_dir, Some(PathBuf::from("/tmp/migrations")));
        assert!(deserialized.auto_migrate);
        assert_eq!(deserialized.migration_timeout, 120);
        assert_eq!(deserialized.admin_role, "root");
        assert_eq!(deserialized.warmup_timeout, 15);
        assert_eq!(deserialized.warmup_retries, 5);
        assert_eq!(deserialized.cache_config.policy_cache_capacity, 512);
    }

    #[test]
    fn test_db_config_serde_partial_uses_defaults() {
        // 只提供 url，其他字段应使用 serde default
        let json = r#"{"url":"sqlite::memory:"}"#;
        let cfg: DbConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.url, "sqlite::memory:");
        assert_eq!(cfg.pool_config.max_connections, 20);
        assert_eq!(cfg.pool_config.min_connections, 5);
        assert_eq!(cfg.admin_role, "admin");
        assert!(!cfg.auto_migrate);
    }

    // 注意：from_env() 测试需要修改环境变量，在 Rust 2024 edition 中
    // set_var/remove_var 为 unsafe，但 lib crate 有 #![forbid(unsafe_code)]。
    // from_env 的覆盖率为外部测试目录（tests/）中独立 crate 的测试覆盖。
}
