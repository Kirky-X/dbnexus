// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池类型定义

use serde::{Deserialize, Serialize};
use std::any::Any;

/// 连接池状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatus {
    /// 活跃连接数
    pub active_connections: u32,
    /// 最大连接数
    pub max_connections: u32,
    /// 空闲连接数
    pub idle_connections: u32,
}

impl Default for PoolStatus {
    fn default() -> Self {
        Self {
            active_connections: 0,
            max_connections: 20,
            idle_connections: 0,
        }
    }
}

/// 数据库连接包装（隐藏第三方类型）
pub struct Connection {
    /// 内部连接（类型擦除）
    inner: Box<dyn Any + Send + Sync>,
}

impl Connection {
    /// 创建新连接包装
    pub(crate) fn new<T: Any + Send + Sync>(inner: T) -> Self {
        Self { inner: Box::new(inner) }
    }

    /// 获取内部连接引用（仅内部使用）
    pub(crate) fn inner<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }

    /// 消费包装，返回内部连接（仅内部使用）
    pub(crate) fn into_inner<T: 'static + Send + Sync>(self) -> Option<T> {
        self.inner.downcast::<T>().ok().map(|b| *b)
    }
}

/// 数据库会话
pub struct Session {
    /// 角色
    pub role: String,
    /// 连接（类型擦除）
    pub connection: Box<dyn Any + Send + Sync>,
    /// 是否在事务中
    pub in_transaction: bool,
}

impl Session {
    /// 创建新会话
    pub(crate) fn new<T: Any + Send + Sync>(role: String, connection: T) -> Self {
        Self {
            role,
            connection: Box::new(connection),
            in_transaction: false,
        }
    }

    /// 获取内部连接引用（仅内部使用）
    pub(crate) fn connection_ref<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.connection.downcast_ref::<T>()
    }
}
