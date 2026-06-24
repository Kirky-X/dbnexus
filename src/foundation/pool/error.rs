// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池错误类型

use thiserror::Error;

/// 连接池配置错误
#[derive(Debug, Error)]
pub enum PoolConfigError {
    /// 缺少必填字段
    #[error("missing required field: {0}")]
    MissingField(String),

    /// 字段值无效
    #[error("invalid value for field '{field}': {reason}")]
    InvalidValue {
        /// 字段名
        field: String,
        /// 无效原因
        reason: String,
    },
}

/// 连接池运行时错误
#[derive(Debug, Error)]
pub enum PoolError {
    /// 获取连接超时
    #[error("failed to acquire connection within timeout")]
    AcquireTimeout,

    /// 连接池耗尽
    #[error("connection pool exhausted")]
    PoolExhausted,

    /// 连接失败
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    /// 健康检查失败
    #[error("health check failed: {0}")]
    HealthCheckFailed(String),

    /// 数据库错误
    #[error("database error: {0}")]
    Database(String),
}
