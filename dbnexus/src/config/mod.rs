//! 配置管理模块
//!
//! 提供数据库配置加载、验证和自动修正功能

pub mod database;

use std::path::Path;
use std::time::Duration;
use thiserror::Error;

pub use database::{DatabaseConfig, DatabaseType, PoolConfig};

/// 配置加载错误
#[derive(Debug, Error)]
pub enum ConfigError {
    /// 文件未找到
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// 格式无效
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// 缺少必填字段
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// 环境变量错误
    #[error("Environment variable error: {0}")]
    EnvVarError(#[from] std::env::VarError),

    /// IO错误
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// 数据库配置
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct DbConfig {
    /// 数据库连接 URL
    #[serde(default)]
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

impl DbConfig {
    /// 从环境变量创建配置
    ///
    /// # Errors
    ///
    /// 如果必需的环境变量缺失或格式错误，返回错误
    pub fn from_env() -> Result<Self, ConfigError> {
        let url = std::env::var("DATABASE_URL").map_err(|_| ConfigError::MissingField("DATABASE_URL".to_string()))?;

        let max_connections = std::env::var("DB_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "20".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidFormat("DB_MAX_CONNECTIONS must be a valid integer".to_string()))?;

        let min_connections = std::env::var("DB_MIN_CONNECTIONS")
            .unwrap_or_else(|_| "5".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidFormat("DB_MIN_CONNECTIONS must be a valid integer".to_string()))?;

        let idle_timeout = std::env::var("DB_IDLE_TIMEOUT")
            .unwrap_or_else(|_| "300".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidFormat("DB_IDLE_TIMEOUT must be a valid integer".to_string()))?;

        let acquire_timeout = std::env::var("DB_ACQUIRE_TIMEOUT")
            .unwrap_or_else(|_| "5000".to_string())
            .parse()
            .map_err(|_| ConfigError::InvalidFormat("DB_ACQUIRE_TIMEOUT must be a valid integer".to_string()))?;

        Ok(Self {
            url,
            max_connections,
            min_connections,
            idle_timeout,
            acquire_timeout,
            permissions_path: std::env::var("DB_PERMISSIONS_PATH").ok(),
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
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref())?;

        // 尝试直接解析为 DbConfig
        if let Ok(config) = serde_yaml::from_str::<DbConfig>(&content) {
            if !config.url.is_empty() {
                return Ok(config);
            }
        }

        // 尝试解析为带有 database 前缀的格式
        #[derive(Debug, serde::Deserialize)]
        struct ConfigWrapper {
            database: DbConfig,
        }

        let wrapper: ConfigWrapper =
            serde_yaml::from_str(&content).map_err(|e| ConfigError::InvalidFormat(e.to_string()))?;

        wrapper.database.validate()?;
        Ok(wrapper.database)
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

        let wrapper: ConfigWrapper = toml::from_str(&content).map_err(|e| ConfigError::InvalidFormat(e.to_string()))?;

        wrapper.database.validate()?;
        Ok(wrapper.database)
    }

    /// 从 YAML 字符串加载配置
    ///
    /// # Errors
    ///
    /// 如果格式错误，返回错误
    pub fn from_yaml_str(yaml: &str) -> Result<Self, ConfigError> {
        let config: DbConfig = serde_yaml::from_str(yaml).map_err(|e| ConfigError::InvalidFormat(e.to_string()))?;

        config.validate()?;
        Ok(config)
    }

    /// 从 TOML 字符串加载配置
    ///
    /// # Errors
    ///
    /// 如果格式错误，返回错误
    pub fn from_toml_str(toml: &str) -> Result<Self, ConfigError> {
        let config: DbConfig = toml::from_str(toml).map_err(|e| ConfigError::InvalidFormat(e.to_string()))?;

        config.validate()?;
        Ok(config)
    }

    /// 验证配置必填字段
    ///
    /// # Errors
    ///
    /// 如果缺少必填字段，返回错误
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.url.is_empty() {
            return Err(ConfigError::MissingField("url".to_string()));
        }

        if self.max_connections == 0 {
            return Err(ConfigError::MissingField("max_connections".to_string()));
        }

        if self.min_connections > self.max_connections {
            return Err(ConfigError::InvalidFormat(
                "min_connections cannot be greater than max_connections".to_string(),
            ));
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

    /// 将配置序列化为 YAML 字符串
    pub fn to_yaml(&self) -> Result<String, ConfigError> {
        serde_yaml::to_string(self).map_err(|e| ConfigError::InvalidFormat(e.to_string()))
    }

    /// 将配置序列化为 TOML 字符串
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string(self).map_err(|e| ConfigError::InvalidFormat(e.to_string()))
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
        let config_paths = [
            "dbnexus.yaml",
            "dbnexus.toml",
            "config/dbnexus.yaml",
            "config/dbnexus.toml",
        ];

        // 尝试查找配置文件
        for config_path in &config_paths {
            let path = Path::new(config_path);
            if path.exists() {
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
                if config_path.exists() {
                    tracing::info!("Loading configuration from: {}", config_path.display());

                    if config_path.ends_with(".yaml") {
                        return Self::from_yaml_file(config_path);
                    } else {
                        return Self::from_toml_file(config_path);
                    }
                }
            }
        }

        Err(ConfigError::FileNotFound("No configuration file found".to_string()))
    }
}

/// 配置自动修正器
#[derive(Debug, Clone)]
pub struct ConfigCorrector;

impl ConfigCorrector {
    /// 自动修正数据库配置
    pub fn auto_correct(mut config: DbConfig) -> DbConfig {
        // 修正 min_connections > max_connections
        if config.min_connections > config.max_connections {
            tracing::warn!(
                "Correcting min_connections ({}) > max_connections ({}), setting min to max",
                config.min_connections,
                config.max_connections
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
                config.acquire_timeout
            );
            config.acquire_timeout = 1000;
        } else if config.acquire_timeout > 60000 {
            tracing::warn!(
                "Adjusting acquire_timeout from {}ms to maximum 60000ms",
                config.acquire_timeout
            );
            config.acquire_timeout = 60000;
        }

        // 修正 idle_timeout 为合理范围
        if config.idle_timeout == 0 {
            config.idle_timeout = 300;
        } else if config.idle_timeout < 30 {
            tracing::warn!("Adjusting idle_timeout from {}s to minimum 30s", config.idle_timeout);
            config.idle_timeout = 30;
        } else if config.idle_timeout > 3600 {
            tracing::warn!("Adjusting idle_timeout from {}s to maximum 3600s", config.idle_timeout);
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
    pub fn validate_config(config: &DbConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if config.url.is_empty() {
            errors.push("Database URL cannot be empty".to_string());
        }

        if config.max_connections == 0 {
            errors.push("max_connections must be greater than 0".to_string());
        }

        if config.min_connections > config.max_connections {
            errors.push("min_connections cannot be greater than max_connections".to_string());
        }

        if config.acquire_timeout == 0 {
            errors.push("acquire_timeout must be greater than 0".to_string());
        }

        if config.idle_timeout == 0 {
            errors.push("idle_timeout must be greater than 0".to_string());
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    /// 从环境变量加载配置并自动修正
    pub fn load_and_correct_from_env() -> Result<DbConfig, ConfigError> {
        let mut config = DbConfig::from_env()?;
        config = ConfigCorrector::auto_correct(config);
        Ok(config)
    }

    /// 从配置文件加载配置并自动修正
    pub fn load_and_correct_from_file(path: impl AsRef<Path>) -> Result<DbConfig, ConfigError> {
        let mut config = DbConfig::from_yaml_file(path)?;
        config = ConfigCorrector::auto_correct(config);
        Ok(config)
    }

    /// 验证配置并应用自动修正
    pub fn validate_and_correct(config: &DbConfig) -> Result<DbConfig, Vec<String>> {
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
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-001: 配置默认值测试
    #[test]
    fn test_default_config_values() {
        let config = DbConfig::default();

        assert_eq!(config.url, "");
        assert_eq!(config.max_connections, 0);
        assert_eq!(config.min_connections, 0);
        assert_eq!(config.idle_timeout, 0);
        assert_eq!(config.acquire_timeout, 0);
        assert!(config.permissions_path.is_none());
    }

    /// TEST-U-002: 配置 Duration 转换测试
    #[test]
    fn test_config_duration_conversion() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 10,
            min_connections: 2,
            idle_timeout: 300,
            acquire_timeout: 5000,
            permissions_path: None,
        };

        assert_eq!(config.idle_timeout_duration(), Duration::from_secs(300));
        assert_eq!(config.acquire_timeout_duration(), Duration::from_millis(5000));
    }
}
