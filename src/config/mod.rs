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

// 子模块声明
mod builder;
mod corrector;
mod loader;
mod security;
mod types;
mod validator;

// 公共类型重导出
pub use types::{
    CacheConfig, ConfigError, ConfigLoadStatsSnapshot, DatabaseType, DbConfig,
    DbConfig as DbnexusConfig, DbError, DbResult, PoolConfig,
};

// 公共构建器重导出
pub use builder::{DbConfigBuilder, DbConfigBuilder as DbnexusConfigBuilder};

// 公共修正器重导出
pub use corrector::ConfigCorrector;

// 公共安全函数重导出
pub use security::{
    is_safe_config_path, is_sensitive_env_var, sanitize_env_value, sanitize_query_params,
    sanitize_url_for_logging, validate_url_format, ALLOWED_URL_SCHEMES, MAX_ENV_VAR_LENGTH,
    SENSITIVE_ENV_VARS, SENSITIVE_QUERY_KEYS,
};

// 公共验证函数重导出
pub use validator::validate_config;
