// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 默认连接池实现

use crate::foundation::pool::error::PoolConfigError;
use crate::foundation::pool::{
    Connection, PoolConfig, PoolConnector, PoolError, PoolLifecycle, PoolReader, PoolStatus, PoolWriter, Session,
};
use async_trait::async_trait;

/// 数据库连接池实现
pub struct DbPool {
    config: PoolConfig,
    inner: sea_orm::DatabaseConnection,
}

impl DbPool {
    /// 连接数据库
    pub async fn connect(config: PoolConfig) -> Result<Self, PoolConfigError> {
        let inner = sea_orm::Database::connect(&config.url)
            .await
            .map_err(|e| PoolConfigError::InvalidValue {
                field: "url".into(),
                reason: e.to_string(),
            })?;
        Ok(Self { config, inner })
    }
}

#[async_trait]
impl PoolReader for DbPool {
    fn status(&self) -> PoolStatus {
        PoolStatus {
            active_connections: 0, // sea-orm 不暴露连接计数
            max_connections: self.config.max_connections,
            idle_connections: 0,
        }
    }

    fn connection_count(&self) -> u32 {
        0 // sea-orm 不暴露连接计数
    }
}

#[async_trait]
impl PoolWriter for DbPool {
    async fn acquire(&self) -> Result<Connection, PoolError> {
        // sea-orm 使用连接池，每次操作自动获取连接
        // 这里返回一个克隆的连接
        Ok(Connection::new(self.inner.clone()))
    }

    async fn release(&self, _conn: Connection) {
        // sea-orm 自动管理连接，无需显式释放
    }

    async fn get_session(&self, role: &str) -> Result<Session, PoolError> {
        Ok(Session::new(role.to_string(), self.inner.clone()))
    }
}

#[async_trait]
impl PoolLifecycle for DbPool {
    async fn health_check(&self) -> anyhow::Result<()> {
        // 使用 sea-orm 的 ping 方法验证连接
        self.inner
            .ping()
            .await
            .map_err(|e| anyhow::anyhow!("pool health check failed: {}", e))
    }

    async fn shutdown(&self) {
        // sea-orm 的 close 需要 self，这里使用 clone 来保留引用
        let conn = self.inner.clone();
        let _ = conn.close().await;
    }
}

impl PoolConnector for DbPool {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sqlite_config() -> PoolConfig {
        PoolConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 10,
            min_connections: 1,
            acquire_timeout: 5000,
            idle_timeout: 300,
        }
    }

    #[tokio::test]
    async fn test_connect_success() {
        let config = make_sqlite_config();
        let pool = DbPool::connect(config).await;
        assert!(pool.is_ok());
        let pool = pool.unwrap();
        assert_eq!(pool.config.max_connections, 10);
    }

    #[tokio::test]
    async fn test_connect_invalid_url() {
        let config = PoolConfig {
            url: "invalid://url".to_string(),
            max_connections: 10,
            min_connections: 1,
            acquire_timeout: 5000,
            idle_timeout: 300,
        };
        let result = DbPool::connect(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pool_reader_status() {
        let pool = DbPool::connect(make_sqlite_config()).await.unwrap();
        let status = pool.status();
        assert_eq!(status.max_connections, 10);
        assert_eq!(status.active_connections, 0);
        assert_eq!(status.idle_connections, 0);
    }

    #[tokio::test]
    async fn test_pool_reader_connection_count() {
        let pool = DbPool::connect(make_sqlite_config()).await.unwrap();
        assert_eq!(pool.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_pool_writer_acquire() {
        let pool = DbPool::connect(make_sqlite_config()).await.unwrap();
        let conn = pool.acquire().await;
        assert!(conn.is_ok());
    }

    #[tokio::test]
    async fn test_pool_writer_release() {
        let pool = DbPool::connect(make_sqlite_config()).await.unwrap();
        let conn = pool.acquire().await.unwrap();
        // release 不应 panic
        pool.release(conn).await;
    }

    #[tokio::test]
    async fn test_pool_writer_get_session() {
        let pool = DbPool::connect(make_sqlite_config()).await.unwrap();
        let session = pool.get_session("admin").await;
        assert!(session.is_ok());
        let session = session.unwrap();
        assert_eq!(session.role, "admin");
        assert!(!session.in_transaction);
    }

    #[tokio::test]
    async fn test_pool_lifecycle_health_check() {
        let pool = DbPool::connect(make_sqlite_config()).await.unwrap();
        let result = pool.health_check().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pool_lifecycle_shutdown() {
        let pool = DbPool::connect(make_sqlite_config()).await.unwrap();
        // shutdown 不应 panic
        pool.shutdown().await;
    }

    #[tokio::test]
    async fn test_pool_connector_trait_object() {
        let pool = DbPool::connect(make_sqlite_config()).await.unwrap();
        let connector: Box<dyn PoolConnector> = Box::new(pool);
        // 验证可以通过 trait object 调用方法
        let status = PoolReader::status(&*connector);
        assert_eq!(status.max_connections, 10);
    }
}
