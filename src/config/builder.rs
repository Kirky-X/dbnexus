// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置构建器
//!
//! 提供链式 API 用于构建 [`DbConfig`] 配置。

use std::path::Path;

use super::types::{
    CacheConfig, ConfigError, DbConfig, default_idle_timeout, default_max_connections,
    default_min_connections, default_migration_timeout, default_warmup_retries,
    default_warmup_timeout, default_acquire_timeout, default_admin_role,
};
use super::validator::validate_config;

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
    migrations_dir: Option<std::path::PathBuf>,
    auto_migrate: Option<bool>,
    migration_timeout: Option<u64>,
    admin_role: Option<String>,
    warmup_timeout: Option<u64>,
    warmup_retries: Option<u32>,
    cache_config: Option<CacheConfig>,
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

    /// 设置缓存配置
    ///
    /// 配置各类缓存的容量和 TTL。
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::config::{DbConfigBuilder, CacheConfig};
    ///
    /// let cache_config = CacheConfig::new(8192, 2000, 20000, 600);
    /// let config = DbConfigBuilder::new()
    ///     .url("sqlite::memory:")
    ///     .cache_config(cache_config)
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn cache_config(mut self, config: CacheConfig) -> Self {
        self.cache_config = Some(config);
        self
    }

    /// 设置权限策略缓存容量
    ///
    /// 便捷方法，用于单独设置权限策略缓存容量。
    pub fn policy_cache_capacity(mut self, capacity: u64) -> Self {
        let mut cache_config = self.cache_config.unwrap_or_default();
        cache_config.policy_cache_capacity = capacity;
        self.cache_config = Some(cache_config);
        self
    }

    /// 设置 SQL 解析缓存容量
    ///
    /// 便捷方法，用于单独设置 SQL 解析缓存容量。
    pub fn sql_parse_cache_capacity(mut self, capacity: u64) -> Self {
        let mut cache_config = self.cache_config.unwrap_or_default();
        cache_config.sql_parse_cache_capacity = capacity;
        self.cache_config = Some(cache_config);
        self
    }

    /// 设置查询结果缓存容量
    ///
    /// 便捷方法，用于单独设置查询结果缓存容量。
    pub fn query_cache_capacity(mut self, capacity: u64) -> Self {
        let mut cache_config = self.cache_config.unwrap_or_default();
        cache_config.query_cache_capacity = capacity;
        self.cache_config = Some(cache_config);
        self
    }

    /// 设置缓存默认 TTL（秒）
    ///
    /// 便捷方法，用于单独设置缓存默认 TTL。
    pub fn cache_default_ttl(mut self, ttl: u64) -> Self {
        let mut cache_config = self.cache_config.unwrap_or_default();
        cache_config.default_ttl = ttl;
        self.cache_config = Some(cache_config);
        self
    }

    /// 构建配置
    ///
    /// # Errors
    ///
    /// 如果验证失败，返回 [`ConfigError`]
    pub fn build(self) -> Result<DbConfig, ConfigError> {
        let cache_config = self.cache_config.unwrap_or_default();

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
            cache_config,
        };

        validate_config(&config)?;
        Ok(config)
    }
}

/// DbConfigBuilder 的别名，用于向后兼容
pub type DbnexusConfigBuilder = DbConfigBuilder;

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(config.acquire_timeout(), 5000);
        assert_eq!(config.admin_role(), "admin");
    }
}
