// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置加载
//!
//! 包含从各种来源加载配置的函数。

use std::path::PathBuf;
use std::str::FromStr;

#[cfg(any(feature = "config-yaml", feature = "config-toml"))]
use std::path::Path;

#[cfg(any(feature = "config-yaml", feature = "config-toml"))]
use super::security::is_safe_config_path;

use super::types::{
    CacheConfig, ConfigError, DbConfig, default_cache_ttl, default_policy_cache_capacity,
    default_query_cache_capacity, default_sql_parse_cache_capacity,
};
use super::validator::validate_config;

/// 解析环境变量为指定类型（带默认值）
///
/// # Type Parameters
///
/// * `T` - 目标类型，必须实现 `FromStr` trait
///
/// # Arguments
///
/// * `key` - 环境变量名称
/// * `default_value` - 默认值字符串
///
/// # Returns
///
/// 解析后的值，如果解析失败返回 `ConfigError::InvalidFormat`
fn parse_env_var<T: FromStr>(key: &str, default_value: &str) -> Result<T, ConfigError> {
    std::env::var(key)
        .unwrap_or_else(|_| default_value.to_string())
        .parse()
        .map_err(|_| ConfigError::InvalidFormat)
}

/// 解析可选环境变量为指定类型
///
/// 如果环境变量不存在或解析失败，返回提供的默认值。
///
/// # Type Parameters
///
/// * `T` - 目标类型，必须实现 `FromStr` trait
///
/// # Arguments
///
/// * `key` - 环境变量名称
/// * `default_value` - 默认值
///
/// # Returns
///
/// 解析后的值或默认值
fn parse_env_var_optional<T: FromStr>(key: &str, default_value: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default_value)
}

/// 解析可选环境变量为指定类型（使用默认值函数）
///
/// 如果环境变量不存在或解析失败，调用默认值函数获取默认值。
///
/// # Type Parameters
///
/// * `T` - 目标类型，必须实现 `FromStr` trait
///
/// # Arguments
///
/// * `key` - 环境变量名称
/// * `default_fn` - 默认值函数
///
/// # Returns
///
/// 解析后的值或默认值
fn parse_env_var_with_default_fn<T: FromStr, F: FnOnce() -> T>(key: &str, default_fn: F) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(default_fn)
}

impl DbConfig {
    /// 验证配置必填字段
    ///
    /// # Errors
    ///
    /// 如果缺少必填字段或格式无效，返回错误
    pub fn validate(&self) -> Result<(), ConfigError> {
        validate_config(self)
    }
    /// 从环境变量创建配置
    ///
    /// # Errors
    ///
    /// 如果必需的环境变量缺失或格式错误，返回错误
    pub fn from_env() -> Result<Self, ConfigError> {
        const MAX_URL_LENGTH: usize = 2048;
        const MAX_ROLE_LENGTH: usize = 64;

        let url = std::env::var("DATABASE_URL").map_err(|_| ConfigError::MissingField)?;

        // URL 长度限制，防止 DoS 攻击
        if url.len() > MAX_URL_LENGTH {
            return Err(ConfigError::InvalidFormat);
        }

        // 使用辅助函数解析必填配置项
        let max_connections: u32 = parse_env_var("DB_MAX_CONNECTIONS", "20")?;
        let min_connections: u32 = parse_env_var("DB_MIN_CONNECTIONS", "5")?;
        let idle_timeout: u64 = parse_env_var("DB_IDLE_TIMEOUT", "300")?;
        let acquire_timeout: u64 = parse_env_var("DB_ACQUIRE_TIMEOUT", "5000")?;

        let admin_role = std::env::var("DB_ADMIN_ROLE").unwrap_or_else(|_| "admin".to_string());

        // 角色名长度限制
        if admin_role.len() > MAX_ROLE_LENGTH {
            return Err(ConfigError::InvalidFormat);
        }

        // 使用辅助函数解析可选配置项
        let auto_migrate: bool = parse_env_var_optional("DB_AUTO_MIGRATE", false);
        let migration_timeout: u64 = parse_env_var_optional("DB_MIGRATION_TIMEOUT", 60);
        let warmup_timeout: u64 = parse_env_var_optional("DB_WARMUP_TIMEOUT", 30);
        let warmup_retries: u32 = parse_env_var_optional("DB_WARMUP_RETRIES", 3);

        // 使用辅助函数解析缓存配置
        let cache_config = CacheConfig {
            policy_cache_capacity: parse_env_var_with_default_fn(
                "DB_POLICY_CACHE_CAPACITY",
                default_policy_cache_capacity,
            ),
            sql_parse_cache_capacity: parse_env_var_with_default_fn(
                "DB_SQL_PARSE_CACHE_CAPACITY",
                default_sql_parse_cache_capacity,
            ),
            query_cache_capacity: parse_env_var_with_default_fn(
                "DB_QUERY_CACHE_CAPACITY",
                default_query_cache_capacity,
            ),
            default_ttl: parse_env_var_with_default_fn("DB_CACHE_DEFAULT_TTL", default_cache_ttl),
        };

        Ok(Self {
            url,
            max_connections,
            min_connections,
            idle_timeout,
            acquire_timeout,
            permissions_path: std::env::var("DB_PERMISSIONS_PATH").ok(),
            migrations_dir: std::env::var("DB_MIGRATIONS_DIR").ok().map(PathBuf::from),
            auto_migrate,
            migration_timeout,
            admin_role,
            warmup_timeout,
            warmup_retries,
            cache_config,
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

        validate_config(&wrapper.database)?;
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

        validate_config(&config)?;
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

        validate_config(&config)?;
        Ok(config)
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
                if is_safe_config_path(path)? {
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
                    if is_safe_config_path(config_path)? {
                        tracing::info!("Loading configuration from: {}", config_path.display());

                        if config_path.extension().map(|e| e == "yaml").unwrap_or(false) {
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

                if is_safe_config_path(path)? {
                    tracing::info!("Loading configuration from: {}", config_path);
                    return Self::from_yaml_file(path);
                }
            }

            if let Some(home_dir) = home::home_dir() {
                let user_config_paths = [home_dir.join(".config").join("dbnexus").join("config.yaml")];

                for config_path in &user_config_paths {
                    if is_safe_config_path(config_path)? {
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

                if is_safe_config_path(path)? {
                    tracing::info!("Loading configuration from: {}", config_path);
                    return Self::from_toml_file(path);
                }
            }

            if let Some(home_dir) = home::home_dir() {
                let user_config_paths = [home_dir.join(".dbnexus").join("config.toml")];

                for config_path in &user_config_paths {
                    if is_safe_config_path(config_path)? {
                        tracing::info!("Loading configuration from: {}", config_path.display());
                        return Self::from_toml_file(config_path);
                    }
                }
            }
        }

        Err(ConfigError::FileNotFound)
    }
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
        let config = crate::config::DbConfigBuilder::new()
            .url("sqlite::memory:")
            .max_connections(10)
            .min_connections(2)
            .idle_timeout(300)
            .acquire_timeout(5000)
            .admin_role("admin")
            .build()
            .unwrap();

        assert_eq!(config.idle_timeout_duration(), std::time::Duration::from_secs(300));
        assert_eq!(config.acquire_timeout_duration(), std::time::Duration::from_millis(5000));
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
}
