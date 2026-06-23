// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池接口定义

use super::error::PoolError;
use super::types::{Connection, PoolStatus, Session};
use async_trait::async_trait;

/// 连接池读取能力
#[async_trait]
pub trait PoolReader: Send + Sync {
    /// 获取连接池状态
    fn status(&self) -> PoolStatus;

    /// 获取当前活跃连接数
    fn connection_count(&self) -> u32;
}

/// 连接池写入能力
#[async_trait]
pub trait PoolWriter: Send + Sync {
    /// 获取连接
    async fn acquire(&self) -> Result<Connection, PoolError>;

    /// 释放连接回池
    async fn release(&self, conn: Connection);

    /// 获取会话
    async fn get_session(&self, role: &str) -> Result<Session, PoolError>;
}

/// 连接池生命周期管理
#[async_trait]
pub trait PoolLifecycle: Send + Sync {
    /// 健康检查
    async fn health_check(&self) -> anyhow::Result<()>;

    /// 优雅关闭
    async fn shutdown(&self);
}

/// 连接池组合 trait
pub trait PoolConnector: PoolReader + PoolWriter + PoolLifecycle + Send + Sync {}
