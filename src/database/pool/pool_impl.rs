// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Pool module implementation details.
//!
//! Contains impl blocks extracted from [`super`].

use super::*;

use crate::foundation::DbResult;
use crate::foundation::{DbConfig, PoolConfig};

#[cfg(any(feature = "cache", feature = "oxcache-integration"))]
use crate::domain::DbCacheProvider;
#[cfg(any(feature = "cache", feature = "oxcache-integration"))]
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
    #[cfg(any(feature = "cache", feature = "oxcache-integration"))]
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
        #[cfg(any(feature = "cache", feature = "oxcache-integration"))]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_new() {
        let builder = DbPoolBuilder::new();
        assert!(builder.url.is_none());
        assert!(builder.config.is_none());
        assert!(builder.admin_role.is_none());
    }

    #[test]
    fn test_builder_url() {
        let builder = DbPoolBuilder::new().url("sqlite::memory:");
        assert_eq!(builder.url.as_deref(), Some("sqlite::memory:"));
    }

    #[test]
    fn test_builder_config() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let builder = DbPoolBuilder::new().config(config);
        assert!(builder.config.is_some());
        assert_eq!(builder.config.unwrap().url, "sqlite::memory:");
    }

    #[test]
    fn test_builder_admin_role_with_config() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            admin_role: "old_admin".to_string(),
            ..Default::default()
        };
        let builder = DbPoolBuilder::new().config(config).admin_role("new_admin");
        assert_eq!(builder.config.unwrap().admin_role, "new_admin");
    }

    #[test]
    fn test_builder_admin_role_without_config() {
        let builder = DbPoolBuilder::new().admin_role("super_admin");
        assert_eq!(builder.admin_role.as_deref(), Some("super_admin"));
    }

    #[test]
    fn test_builder_max_connections_with_config() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let builder = DbPoolBuilder::new().config(config).max_connections(50);
        assert_eq!(builder.config.unwrap().pool_config.max_connections, 50);
    }

    #[test]
    fn test_builder_max_connections_with_url_only() {
        let builder = DbPoolBuilder::new().url("sqlite::memory:").max_connections(30);
        let config = builder.config.unwrap();
        assert_eq!(config.pool_config.max_connections, 30);
        assert_eq!(config.url, "sqlite::memory:");
    }

    #[test]
    fn test_builder_max_connections_no_config_no_url() {
        // Neither config nor url set -> no-op (self stays same)
        let builder = DbPoolBuilder::new().max_connections(30);
        assert!(builder.config.is_none());
    }

    #[test]
    fn test_builder_min_connections_with_config() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        };
        let builder = DbPoolBuilder::new().config(config).min_connections(10);
        assert_eq!(builder.config.unwrap().pool_config.min_connections, 10);
    }

    #[test]
    fn test_builder_min_connections_with_url_only() {
        let builder = DbPoolBuilder::new().url("sqlite::memory:").min_connections(5);
        let config = builder.config.unwrap();
        assert_eq!(config.pool_config.min_connections, 5);
    }

    #[test]
    fn test_builder_min_connections_no_config_no_url() {
        let builder = DbPoolBuilder::new().min_connections(5);
        assert!(builder.config.is_none());
    }

    #[test]
    fn test_builder_debug_format() {
        let builder = DbPoolBuilder::new().url("sqlite::memory:");
        let debug = format!("{:?}", builder);
        assert!(debug.contains("DbPoolBuilder"));
        assert!(debug.contains("sqlite::memory:"));
    }

    #[tokio::test]
    async fn test_builder_build_no_url_no_config_fails() {
        let result = DbPoolBuilder::new().build().await;
        assert!(result.is_err());
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_builder_build_with_url() {
        let pool = DbPoolBuilder::new()
            .url("sqlite::memory:")
            .build()
            .await
            .expect("should build pool");
        assert_eq!(pool.config().url, "sqlite::memory:");
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_builder_build_with_config() {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: PoolConfig {
                max_connections: 15,
                ..Default::default()
            },
            ..Default::default()
        };
        let pool = DbPoolBuilder::new()
            .config(config)
            .build()
            .await
            .expect("should build pool");
        assert_eq!(pool.config().pool_config.max_connections, 15);
    }

    #[cfg(any(feature = "cache", feature = "oxcache-integration"))]
    #[test]
    fn test_builder_cache_provider() {
        use crate::foundation::DbError;
        use std::future::Future;
        use std::pin::Pin;

        struct NoopCacheProvider;
        impl DbCacheProvider for NoopCacheProvider {
            fn get<'a>(
                &'a self,
                _key: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, DbError>> + Send + 'a>> {
                Box::pin(async { Ok(None) })
            }
            fn set<'a>(
                &'a self,
                _key: &'a str,
                _value: Vec<u8>,
                _ttl: Option<std::time::Duration>,
            ) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }
            fn delete<'a>(&'a self, _key: &'a str) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }
        }

        let provider = Arc::new(NoopCacheProvider);
        let builder = DbPoolBuilder::new().cache_provider(provider);
        assert!(builder.cache_provider.is_some());
    }

    #[cfg(any(feature = "cache", feature = "oxcache-integration"))]
    #[tokio::test]
    async fn test_builder_build_with_cache_provider() {
        use crate::foundation::DbError;
        use std::future::Future;
        use std::pin::Pin;

        struct NoopCacheProvider;
        impl DbCacheProvider for NoopCacheProvider {
            fn get<'a>(
                &'a self,
                _key: &'a str,
            ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, DbError>> + Send + 'a>> {
                Box::pin(async { Ok(None) })
            }
            fn set<'a>(
                &'a self,
                _key: &'a str,
                _value: Vec<u8>,
                _ttl: Option<std::time::Duration>,
            ) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }
            fn delete<'a>(&'a self, _key: &'a str) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }
        }

        let provider = Arc::new(NoopCacheProvider);
        let pool = DbPoolBuilder::new()
            .url("sqlite::memory:")
            .cache_provider(provider)
            .build()
            .await
            .expect("should build pool with cache provider");
        assert!(pool.cache_provider().is_some());
    }
}
