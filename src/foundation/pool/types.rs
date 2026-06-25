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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub(crate) fn connection_ref<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.connection.downcast_ref::<T>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_status_default() {
        let status = PoolStatus::default();
        assert_eq!(status.active_connections, 0);
        assert_eq!(status.max_connections, 20);
        assert_eq!(status.idle_connections, 0);
    }

    #[test]
    fn test_pool_status_clone_serialize() {
        let status = PoolStatus {
            active_connections: 5,
            max_connections: 10,
            idle_connections: 5,
        };
        let cloned = status.clone();
        assert_eq!(cloned.active_connections, 5);
        assert_eq!(cloned.max_connections, 10);
        assert_eq!(cloned.idle_connections, 5);

        // 序列化/反序列化往返
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: PoolStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.active_connections, 5);
        assert_eq!(deserialized.max_connections, 10);
        assert_eq!(deserialized.idle_connections, 5);
    }

    #[test]
    fn test_connection_new_and_inner() {
        let conn = Connection::new(42_i32);
        assert_eq!(conn.inner::<i32>(), Some(&42));
        assert_eq!(conn.inner::<String>(), None);
    }

    #[test]
    fn test_connection_into_inner() {
        let conn = Connection::new("hello".to_string());
        let inner: Option<String> = conn.into_inner();
        assert_eq!(inner, Some("hello".to_string()));
    }

    #[test]
    fn test_connection_into_inner_wrong_type_returns_none() {
        let conn = Connection::new(42_i32);
        let inner: Option<String> = conn.into_inner();
        assert_eq!(inner, None);
    }

    #[test]
    fn test_session_new_and_connection_ref() {
        let session = Session::new("admin".to_string(), 42_i32);
        assert_eq!(session.role, "admin");
        assert!(!session.in_transaction);
        assert_eq!(session.connection_ref::<i32>(), Some(&42));
        assert_eq!(session.connection_ref::<String>(), None);
    }

    #[test]
    fn test_session_with_complex_type() {
        #[derive(Debug)]
        struct FakeConn {
            url: String,
        }

        let fake = FakeConn {
            url: "sqlite::memory:".to_string(),
        };
        let session = Session::new("user".to_string(), fake);
        assert_eq!(session.role, "user");
        let conn_ref = session.connection_ref::<FakeConn>();
        assert!(conn_ref.is_some());
        assert_eq!(conn_ref.unwrap().url, "sqlite::memory:");
    }
}
