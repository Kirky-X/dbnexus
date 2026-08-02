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
