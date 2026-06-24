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
