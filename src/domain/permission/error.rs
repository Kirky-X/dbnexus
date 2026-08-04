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

impl crate::i18n::error_ext::LocalizedMsg for PermissionConfigError {
    fn message_key(&self) -> &'static str {
        match self {
            Self::MissingField(_) => "perm-config-missing-field",
            Self::InvalidValue { .. } => "perm-config-invalid-value",
            Self::PolicyFileNotFound(_) => "perm-config-policy-not-found",
        }
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        match self {
            Self::MissingField(field) => vec![("field", field.clone())],
            Self::InvalidValue { field, reason } => vec![("field", field.clone()), ("reason", reason.clone())],
            Self::PolicyFileNotFound(path) => vec![("path", path.clone())],
        }
    }
}

impl crate::i18n::error_ext::LocalizedMsg for PermissionError {
    fn message_key(&self) -> &'static str {
        match self {
            Self::Denied { .. } => "perm-denied",
            Self::RoleNotFound(_) => "perm-role-not-found",
            Self::InvalidPolicy(_) => "perm-invalid-policy",
            Self::RateLimited => "perm-rate-limited",
            Self::ParseError(_) => "perm-parse-error",
        }
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        match self {
            Self::Denied { resource, operation } => {
                vec![("resource", resource.clone()), ("operation", operation.clone())]
            }
            Self::RoleNotFound(role) => vec![("role", role.clone())],
            Self::InvalidPolicy(reason) => vec![("reason", reason.clone())],
            Self::RateLimited => vec![],
            Self::ParseError(reason) => vec![("reason", reason.clone())],
        }
    }
}
