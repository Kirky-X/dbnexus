// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 内存连接池实现（测试用）

use crate::foundation::pool::{
    Connection, PoolConnector, PoolError, PoolLifecycle, PoolReader, PoolStatus, PoolWriter, Session,
};
use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};

/// 内存连接池（测试用）
pub struct MemoryPool {
    max_connections: u32,
    active: AtomicU32,
}

impl MemoryPool {
    /// 创建新的内存连接池
    pub fn new() -> Self {
        Self {
            max_connections: 20,
            active: AtomicU32::new(0),
        }
    }
}

impl Default for MemoryPool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PoolReader for MemoryPool {
    fn status(&self) -> PoolStatus {
        let active = self.active.load(Ordering::Relaxed);
        PoolStatus {
            active_connections: active,
            max_connections: self.max_connections,
            idle_connections: self.max_connections.saturating_sub(active),
        }
    }

    fn connection_count(&self) -> u32 {
        self.active.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl PoolWriter for MemoryPool {
    async fn acquire(&self) -> Result<Connection, PoolError> {
        let current = self.active.fetch_add(1, Ordering::Relaxed);
        if current >= self.max_connections {
            self.active.fetch_sub(1, Ordering::Relaxed);
            return Err(PoolError::PoolExhausted);
        }
        // 返回一个假的连接（测试用）
        Ok(Connection::new(sea_orm::DatabaseConnection::default()))
    }

    async fn release(&self, _conn: Connection) {
        self.active.fetch_sub(1, Ordering::Relaxed);
    }

    async fn get_session(&self, role: &str) -> Result<Session, PoolError> {
        let conn = self.acquire().await?;
        let inner = conn.into_inner::<sea_orm::DatabaseConnection>().ok_or(PoolError::PoolExhausted)?;
        Ok(Session::new(role.to_string(), inner))
    }
}

#[async_trait]
impl PoolLifecycle for MemoryPool {
    async fn health_check(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn shutdown(&self) {
        self.active.store(0, Ordering::Relaxed);
    }
}

impl PoolConnector for MemoryPool {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pool_new_defaults() {
        let pool = MemoryPool::new();
        let status = pool.status();
        assert_eq!(status.max_connections, 20);
        assert_eq!(status.active_connections, 0);
        assert_eq!(status.idle_connections, 20);
        assert_eq!(pool.connection_count(), 0);
    }

    #[test]
    fn test_memory_pool_default_equals_new() {
        let p1 = MemoryPool::new();
        let p2 = MemoryPool::default();
        assert_eq!(p1.status().max_connections, p2.status().max_connections);
    }

    #[tokio::test]
    async fn test_memory_pool_acquire_increments_active() {
        let pool = MemoryPool::new();
        let conn = pool.acquire().await;
        assert!(conn.is_ok());
        assert_eq!(pool.connection_count(), 1);
        assert_eq!(pool.status().active_connections, 1);
        assert_eq!(pool.status().idle_connections, 19);
    }

    #[tokio::test]
    async fn test_memory_pool_release_decrements_active() {
        let pool = MemoryPool::new();
        let conn = pool.acquire().await.unwrap();
        assert_eq!(pool.connection_count(), 1);
        pool.release(conn).await;
        assert_eq!(pool.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_memory_pool_acquire_until_exhausted() {
        let pool = MemoryPool::new();
        let mut conns = Vec::new();
        // max_connections = 20
        for _ in 0..20 {
            let conn = pool.acquire().await;
            assert!(conn.is_ok(), "acquire should succeed within limit");
            conns.push(conn.unwrap());
        }
        assert_eq!(pool.connection_count(), 20);

        // 第 21 个应该失败（不使用 unwrap_err，因 Connection 未实现 Debug）
        let result = pool.acquire().await;
        assert!(result.is_err());
        match result {
            Err(PoolError::PoolExhausted) => {}
            _ => panic!("expected PoolExhausted error"),
        }

        // 释放一个后应该可以再次获取
        pool.release(conns.pop().unwrap()).await;
        let conn = pool.acquire().await;
        assert!(conn.is_ok());
    }

    #[tokio::test]
    async fn test_memory_pool_get_session_success() {
        let pool = MemoryPool::new();
        let session = pool.get_session("admin").await;
        assert!(session.is_ok());
        let session = session.unwrap();
        assert_eq!(session.role, "admin");
        assert!(!session.in_transaction);
        // get_session 内部 acquire+into_inner，active 已 +1
        assert_eq!(pool.connection_count(), 1);
    }

    #[tokio::test]
    async fn test_memory_pool_health_check_always_ok() {
        let pool = MemoryPool::new();
        assert!(pool.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_memory_pool_shutdown_resets_active() {
        let pool = MemoryPool::new();
        let _conn = pool.acquire().await.unwrap();
        assert_eq!(pool.connection_count(), 1);
        pool.shutdown().await;
        assert_eq!(pool.connection_count(), 0);
        assert_eq!(pool.status().active_connections, 0);
    }

    #[tokio::test]
    async fn test_memory_pool_as_trait_object() {
        let pool: Box<dyn PoolConnector> = Box::new(MemoryPool::new());
        let status = PoolReader::status(&*pool);
        assert_eq!(status.max_connections, 20);
        let conn = pool.acquire().await;
        assert!(conn.is_ok());
        assert!(pool.health_check().await.is_ok());
        pool.shutdown().await;
    }
}
