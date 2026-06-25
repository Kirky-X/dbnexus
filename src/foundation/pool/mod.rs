// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池基础模块
//!
//! 提供数据库连接池的基础设施能力

mod config;
mod error;
mod impl_;
mod interface;
mod types;

pub use config::PoolConfig;
pub use error::{PoolConfigError, PoolError};
pub use interface::{PoolConnector, PoolLifecycle, PoolReader, PoolWriter};
pub use types::{Connection, PoolStatus, Session};

/// 标准工厂函数
pub async fn new(config: PoolConfig) -> Result<impl PoolConnector, PoolConfigError> {
    config.validate()?;
    impl_::default::DbPool::connect(config).await
}

/// 内存实现工厂函数（测试用）
pub fn new_in_memory() -> impl PoolConnector {
    impl_::memory::MemoryPool::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_factory_with_valid_config() {
        let config = PoolConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 5,
            min_connections: 1,
            acquire_timeout: 5000,
            idle_timeout: 300,
        };
        let pool = new(config).await;
        assert!(pool.is_ok());
        let pool = pool.unwrap();
        assert_eq!(pool.status().max_connections, 5);
    }

    #[tokio::test]
    async fn test_new_factory_with_invalid_config_fails() {
        let config = PoolConfig {
            url: String::new(), // 空 URL 应失败
            max_connections: 5,
            min_connections: 1,
            acquire_timeout: 5000,
            idle_timeout: 300,
        };
        let result = new(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_new_factory_with_zero_max_connections_fails() {
        let config = PoolConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 0,
            min_connections: 1,
            acquire_timeout: 5000,
            idle_timeout: 300,
        };
        let result = new(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_new_in_memory_factory() {
        let pool = new_in_memory();
        let status = pool.status();
        assert_eq!(status.max_connections, 20);
        assert_eq!(status.active_connections, 0);
        assert_eq!(status.idle_connections, 20);
    }

    #[tokio::test]
    async fn test_new_in_memory_acquire_and_release() {
        let pool = new_in_memory();
        let conn = pool.acquire().await;
        assert!(conn.is_ok());
        pool.release(conn.unwrap()).await;
        assert_eq!(pool.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_new_in_memory_health_check_and_shutdown() {
        let pool = new_in_memory();
        assert!(pool.health_check().await.is_ok());
        pool.shutdown().await;
        // shutdown 后仍可调用 status（active 被重置为 0）
        assert_eq!(pool.status().active_connections, 0);
    }
}
