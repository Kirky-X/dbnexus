// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 权限模块错误类型

use thiserror::Error;

/// 权限配置错误
#[derive(Debug, Error)]
pub enum PermissionConfigError {
    /// 缺少必填字段
    #[error("missing required field: {0}")]
    MissingField(String),

    /// 字段值无效
    #[error("invalid value for field '{field}': {reason}")]
    InvalidValue {
        /// 字段名
        field: String,
        /// 原因
        reason: String,
    },

    /// 策略文件未找到
    #[error("policy file not found: {0}")]
    PolicyFileNotFound(String),
}

/// 权限运行时错误
#[derive(Debug, Error)]
pub enum PermissionError {
    /// 权限被拒绝
    #[error("permission denied for {operation} on {resource}")]
    Denied {
        /// 资源名
        resource: String,
        /// 操作名
        operation: String,
    },

    /// 角色未找到
    #[error("role not found: {0}")]
    RoleNotFound(String),

    /// 无效的策略配置
    #[error("invalid policy configuration: {0}")]
    InvalidPolicy(String),

    /// 速率限制
    #[error("rate limit exceeded")]
    RateLimited,

    /// 策略解析错误
    #[error("policy parse error: {0}")]
    ParseError(String),
}
