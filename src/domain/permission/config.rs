// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 权限模块配置

use super::error::{PermissionConfigError, PermissionError};
use serde::Deserialize;

/// 默认策略类型
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultPolicy {
    /// 拒绝所有
    #[default]
    DenyAll,
    /// 允许所有
    AllowAll,
}

/// 权限模块配置
#[derive(Debug, Clone, Deserialize)]
pub struct PermissionConfig {
    /// 权限策略文件路径
    #[serde(default)]
    pub policy_path: Option<String>,

    /// 默认策略
    #[serde(default)]
    pub default_policy: DefaultPolicy,

    /// 管理员角色名称
    #[serde(default = "PermissionConfig::default_admin_role")]
    pub admin_role: String,

    /// 是否启用速率限制
    #[serde(default)]
    pub rate_limit_enabled: bool,

    /// 速率限制最大请求数
    #[serde(default = "PermissionConfig::default_rate_limit_max")]
    pub rate_limit_max_requests: u32,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            policy_path: None,
            default_policy: DefaultPolicy::default(),
            admin_role: Self::default_admin_role(),
            rate_limit_enabled: false,
            rate_limit_max_requests: Self::default_rate_limit_max(),
        }
    }
}

impl PermissionConfig {
    /// 语义校验
    pub fn validate(&self) -> Result<(), PermissionConfigError> {
        if self.admin_role.is_empty() {
            return Err(PermissionConfigError::MissingField("admin_role".into()));
        }
        if self.rate_limit_enabled && self.rate_limit_max_requests == 0 {
            return Err(PermissionConfigError::InvalidValue {
                field: "rate_limit_max_requests".into(),
                reason: "must be greater than 0 when rate limiting enabled".into(),
            });
        }
        if let Some(path) = &self.policy_path {
            Self::validate_policy_path(path).map_err(|e| PermissionConfigError::InvalidValue {
                field: "policy_path".into(),
                reason: e.to_string(),
            })?;
        }
        Ok(())
    }

    /// 校验策略文件路径安全性
    ///
    /// 要求路径为绝对路径，且路径组件中不含 `..`（按 `std::path::Path` 分量检查，
    /// 不做字符串包含匹配，避免误伤文件名中含连续点的合法路径）。
    /// 供 `validate()` 与运行时读取前（`load_policies` / `health_check`）共用。
    ///
    /// # Errors
    ///
    /// 相对路径或包含 `..` 组件时返回 `PermissionError::InvalidPolicy`
    pub(crate) fn validate_policy_path(path: &str) -> Result<(), PermissionError> {
        let path = std::path::Path::new(path);
        if !path.is_absolute() {
            return Err(PermissionError::InvalidPolicy(format!(
                "policy path must be absolute: {}",
                path.display()
            )));
        }
        if path
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(PermissionError::InvalidPolicy(format!(
                "policy path must not contain '..' components: {}",
                path.display()
            )));
        }
        Ok(())
    }

    fn default_admin_role() -> String {
        "admin".into()
    }
    fn default_rate_limit_max() -> u32 {
        100
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_ok_without_policy_path() {
        assert!(PermissionConfig::default().validate().is_ok());
    }

    #[test]
    fn validate_rejects_relative_policy_path() {
        assert!(PermissionConfig::validate_policy_path("policies/roles.yaml").is_err());
        // 空路径同样视为相对路径
        assert!(PermissionConfig::validate_policy_path("").is_err());
    }

    #[test]
    fn validate_rejects_parent_dir_component() {
        assert!(PermissionConfig::validate_policy_path("/etc/dbnexus/../secrets.yaml").is_err());
        assert!(PermissionConfig::validate_policy_path("..").is_err());
    }

    #[test]
    fn validate_accepts_absolute_path_without_parent_dir() {
        assert!(PermissionConfig::validate_policy_path("/etc/dbnexus/policies.yaml").is_ok());
        // 文件名中含连续点不是 `..` 组件，不应误伤
        assert!(PermissionConfig::validate_policy_path("/etc/dbnexus/policies..yaml").is_ok());
    }

    #[test]
    fn validate_checks_policy_path_when_present() {
        let cfg = PermissionConfig {
            policy_path: Some("relative/roles.yaml".into()),
            ..PermissionConfig::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(
            matches!(err, PermissionConfigError::InvalidValue { ref field, .. } if field == "policy_path"),
            "expected InvalidValue for policy_path, got {err:?}"
        );
    }
}
