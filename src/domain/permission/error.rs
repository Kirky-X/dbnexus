// Copyright (c) 2025-2026 Kirky.X🌠
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
            Self::InvalidValue { field, reason } => {
                vec![("field", field.clone()), ("reason", reason.clone())]
            }
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
            Self::Denied {
                resource,
                operation,
            } => {
                vec![
                    ("resource", resource.clone()),
                    ("operation", operation.clone()),
                ]
            }
            Self::RoleNotFound(role) => vec![("role", role.clone())],
            Self::InvalidPolicy(reason) => vec![("reason", reason.clone())],
            Self::RateLimited => vec![],
            Self::ParseError(reason) => vec![("reason", reason.clone())],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::error_ext::LocalizedMsg;

    #[test]
    fn test_permission_config_error_localized_msg() {
        let cases = [
            (
                PermissionConfigError::MissingField("roles".to_string()),
                "perm-config-missing-field",
            ),
            (
                PermissionConfigError::InvalidValue {
                    field: "ttl".to_string(),
                    reason: "negative".to_string(),
                },
                "perm-config-invalid-value",
            ),
            (
                PermissionConfigError::PolicyFileNotFound("p.json".to_string()),
                "perm-config-policy-not-found",
            ),
        ];
        for (err, key) in cases {
            assert_eq!(err.message_key(), key);
        }

        assert_eq!(
            PermissionConfigError::MissingField("roles".to_string()).message_args(),
            vec![("field", "roles".to_string())]
        );
        assert_eq!(
            PermissionConfigError::InvalidValue {
                field: "ttl".to_string(),
                reason: "negative".to_string(),
            }
            .message_args(),
            vec![
                ("field", "ttl".to_string()),
                ("reason", "negative".to_string())
            ]
        );
        assert_eq!(
            PermissionConfigError::PolicyFileNotFound("p.json".to_string()).message_args(),
            vec![("path", "p.json".to_string())]
        );
    }

    #[test]
    fn test_permission_error_localized_msg() {
        let cases = [
            (
                PermissionError::Denied {
                    resource: "users".to_string(),
                    operation: "DELETE".to_string(),
                },
                "perm-denied",
            ),
            (
                PermissionError::RoleNotFound("admin".to_string()),
                "perm-role-not-found",
            ),
            (
                PermissionError::InvalidPolicy("bad".to_string()),
                "perm-invalid-policy",
            ),
            (PermissionError::RateLimited, "perm-rate-limited"),
            (
                PermissionError::ParseError("oops".to_string()),
                "perm-parse-error",
            ),
        ];
        for (err, key) in cases {
            assert_eq!(err.message_key(), key);
        }

        assert_eq!(
            PermissionError::Denied {
                resource: "users".to_string(),
                operation: "DELETE".to_string(),
            }
            .message_args(),
            vec![
                ("resource", "users".to_string()),
                ("operation", "DELETE".to_string())
            ]
        );
        assert_eq!(
            PermissionError::RoleNotFound("admin".to_string()).message_args(),
            vec![("role", "admin".to_string())]
        );
        assert!(PermissionError::RateLimited.message_args().is_empty());
        assert_eq!(
            PermissionError::InvalidPolicy("bad".to_string()).message_args(),
            vec![("reason", "bad".to_string())]
        );
    }
}
