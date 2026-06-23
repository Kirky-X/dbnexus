// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限模块配置

use super::error::PermissionConfigError;
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
        Ok(())
    }

    fn default_admin_role() -> String {
        "admin".into()
    }
    fn default_rate_limit_max() -> u32 {
        100
    }
}
