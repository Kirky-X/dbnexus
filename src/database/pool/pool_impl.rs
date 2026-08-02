// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Pool module implementation details.
//!
//! Contains impl blocks extracted from [`super`].

use super::*;

use crate::foundation::DbResult;
use crate::foundation::{DbConfig, PoolConfig};

#[cfg(feature = "permission")]
use crate::access::PermissionConfig;
#[cfg(feature = "cache")]
use crate::domain::DbCacheProvider;
#[cfg(feature = "metrics")]
use crate::observability::MetricsCollector;
#[cfg(feature = "cache")]
use oxcache::Cache;
#[cfg(any(feature = "metrics", feature = "cache"))]
use std::sync::Arc;

impl DbPoolBuilder {
    /// 创建新的构造器
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置数据库连接 URL
    ///
    /// # Arguments
    ///
    /// * `url` - 数据库连接 URL 字符串
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    pub fn url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    /// 设置数据库配置
    ///
    /// # Arguments
    ///
    /// * `config` - 数据库配置
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    pub fn config(mut self, config: DbConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// 设置指标收集器
    ///
    /// # Deprecated
    ///
    /// **此 setter 为 no-op**：`build()` 当前不会使用此值（HIGH-001）。
    /// 请通过 `DbConfig` 或在 `DbPool::with_config()` 创建后注入。
    #[deprecated(
        since = "0.3.0",
        note = "DbPoolBuilder::build() 静默丢弃此值；请通过 DbConfig 或 DbPool::with_config() 后注入"
    )]
    #[cfg(feature = "metrics")]
    pub fn metrics_collector(mut self, metrics_collector: Arc<MetricsCollector>) -> Self {
        self.metrics_collector = Some(metrics_collector);
        self
    }

    /// 设置权限配置
    ///
    /// # Deprecated
    ///
    /// **此 setter 为 no-op**：`build()` 当前不会使用此值（HIGH-001）。
    /// 请通过 `DbConfig.permission_config_path` 指定权限配置文件路径。
    #[deprecated(
        since = "0.3.0",
        note = "DbPoolBuilder::build() 静默丢弃此值；请使用 DbConfig.permission_config_path"
    )]
    #[cfg(feature = "permission")]
    pub fn permission_config(mut self, permission_config: PermissionConfig) -> Self {
        self.permission_config = Some(permission_config);
        self
    }

    /// 设置管理员角色名称
    ///
    /// # Arguments
    ///
    /// * `admin_role` - 管理员角色名称
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    pub fn admin_role(mut self, admin_role: &str) -> Self {
        if let Some(ref mut config) = self.config {
            config.admin_role = admin_role.to_string();
        } else {
            self.admin_role = Some(admin_role.to_string());
        }
        self
    }

    /// 注入oxcache缓存实例（DI支持）
    ///
    /// # Deprecated
    ///
    /// **此 setter 为 no-op**：`build()` 当前不会使用此值（HIGH-001）。
    /// 缓存实例由 `DbPool::with_config()` 根据 `DbConfig.cache_config` 自动创建。
    #[deprecated(
        since = "0.3.0",
        note = "DbPoolBuilder::build() 静默丢弃此值；缓存由 DbPool::with_config() 根据 DbConfig.cache_config 自动创建"
    )]
    #[cfg(feature = "cache")]
    pub fn with_oxcache(mut self, cache: Arc<Cache<String, serde_json::Value>>) -> Self {
        self.cache = Some(cache);
        self
    }

    /// 注入缓存提供者（DI 注入点）
    ///
    /// 允许外部注入 `DbCacheProvider` 实现，覆盖默认的内置缓存。
    /// 仅在 `cache` 特性启用时可用。
    ///
    /// # Arguments
    ///
    /// * `provider` - 缓存提供者实例
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    #[cfg(feature = "cache")]
    pub fn cache_provider(mut self, provider: Arc<dyn DbCacheProvider + Send + Sync>) -> Self {
        self.cache_provider = Some(provider);
        self
    }

    /// 设置最大连接数
    ///
    /// # Arguments
    ///
    /// * `max_connections` - 最大连接数
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    pub fn max_connections(mut self, max_connections: u32) -> Self {
        if let Some(ref mut config) = self.config {
            config.pool_config.max_connections = max_connections;
        } else if let Some(ref url) = self.url {
            // 如果只有 url，创建一个默认配置然后修改
            let config = DbConfig {
                url: url.clone(),
                pool_config: PoolConfig {
                    max_connections,
                    ..Default::default()
                },
                ..Default::default()
            };
            self.config = Some(config);
        }
        self
    }

    /// 设置最小连接数
    ///
    /// # Arguments
    ///
    /// * `min_connections` - 最小连接数
    ///
    /// # Returns
    ///
    /// 返回构造器自身以支持链式调用
    pub fn min_connections(mut self, min_connections: u32) -> Self {
        if let Some(ref mut config) = self.config {
            config.pool_config.min_connections = min_connections;
        } else if let Some(ref url) = self.url {
            let config = DbConfig {
                url: url.clone(),
                pool_config: PoolConfig {
                    min_connections,
                    ..Default::default()
                },
                ..Default::default()
            };
            self.config = Some(config);
        }
        self
    }

    /// 构建 DbPool
    ///
    /// # Errors
    ///
    /// 如果配置无效或无法连接数据库，返回错误
    ///
    /// # Returns
    ///
    /// 返回新创建的 DbPool 实例
    pub async fn build(self) -> DbResult<DbPool> {
        // 确定最终配置
        let config = if let Some(config) = self.config {
            config
        } else if let Some(url) = self.url {
            // 从 url 创建默认配置
            DbConfig {
                url,
                pool_config: PoolConfig {
                    max_connections: 20,
                    min_connections: 5,
                    idle_timeout: 300,
                    acquire_timeout: 5000,
                },
                admin_role: self.admin_role.unwrap_or_else(|| "admin".to_string()),
                ..Default::default()
            }
        } else {
            return Err(crate::foundation::DbError::new(sea_orm::DbErr::Custom(
                "Either url or config must be provided".to_string(),
            )));
        };

        // 创建 pool
        #[allow(unused_mut)]
        let mut pool = DbPool::with_config(config).await?;

        // 注入缓存提供者（如果设置）
        #[cfg(feature = "cache")]
        if let Some(cache_provider) = self.cache_provider {
            pool.set_cache_provider(cache_provider);
        }

        // 注意：以下值已通过 config 设置，不需要额外调用 setter 方法
        // - admin_role: 在 config 创建时已设置（line 327）
        // - metrics_collector: 通过 config 或其他方式设置
        // - permission_config: 在 config 创建时已设置

        Ok(pool)
    }
}

impl std::fmt::Debug for DbPoolBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbPoolBuilder")
            .field("url", &self.url)
            .field("config", &self.config.is_some())
            .field("admin_role", &self.admin_role)
            .finish()
    }
}
