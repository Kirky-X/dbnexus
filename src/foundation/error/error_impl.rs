// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Error module implementation details.
//!
//! Contains impl blocks extracted from [`super`].

use super::*;

impl DbError {
    /// 从 sea_orm::DbErr 创建数据库连接错误
    pub fn new(error: sea_orm::DbErr) -> Self {
        Self::Connection(error)
    }

    /// 获取错误消息
    pub fn message(&self) -> String {
        match self {
            DbError::Connection(e) => e.to_string(),
            DbError::Config(msg) => msg.clone(),
            DbError::Permission(msg) => msg.clone(),
            DbError::Transaction(msg) => msg.clone(),
            DbError::Migration(msg) => msg.clone(),
            DbError::Cache(msg) => msg.clone(),
            DbError::Query(msg) => msg.clone(),
            #[cfg(feature = "validation")]
            DbError::Validation(msg) => msg.clone(),
        }
    }
}

/// 从字符串创建 DbError::Config
impl From<String> for DbError {
    fn from(msg: String) -> Self {
        Self::Config(msg)
    }
}

/// 从 &str 创建 DbError::Config
impl From<&str> for DbError {
    fn from(msg: &str) -> Self {
        Self::Config(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_connection() {
        let err = DbError::new(sea_orm::DbErr::Custom("conn failed".to_string()));
        assert!(err.message().contains("conn failed"));
    }

    #[test]
    fn test_message_config() {
        let err = DbError::Config("bad config".to_string());
        assert_eq!(err.message(), "bad config");
    }

    #[test]
    fn test_message_permission() {
        let err = DbError::Permission("denied".to_string());
        assert_eq!(err.message(), "denied");
    }

    #[test]
    fn test_message_transaction() {
        let err = DbError::Transaction("txn failed".to_string());
        assert_eq!(err.message(), "txn failed");
    }

    #[test]
    fn test_message_migration() {
        let err = DbError::Migration("migrate failed".to_string());
        assert_eq!(err.message(), "migrate failed");
    }

    #[test]
    fn test_message_cache() {
        let err = DbError::Cache("cache miss".to_string());
        assert_eq!(err.message(), "cache miss");
    }

    #[test]
    fn test_message_query() {
        let err = DbError::Query("query failed".to_string());
        assert_eq!(err.message(), "query failed");
    }

    #[test]
    fn test_from_string() {
        let err: DbError = "config error".to_string().into();
        assert_eq!(err.message(), "config error");
    }

    #[test]
    fn test_from_str() {
        let err: DbError = "config error".into();
        assert_eq!(err.message(), "config error");
    }
}
