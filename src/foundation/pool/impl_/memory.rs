// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 内存连接池实现（测试用）

use crate::foundation::pool::{
    Connection, PoolConfig, PoolConnector, PoolError, PoolLifecycle, PoolReader, PoolStatus, PoolWriter, Session,
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
        let inner = conn.into_inner::<sea_orm::DatabaseConnection>().ok_or_else(|| PoolError::PoolExhausted)?;
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
