// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! RBAC权限提供者实现
//!
//! 提供基于角色的访问控制（RBAC）权限提供者。

use super::{PermissionAction, PermissionProvider, PermissionProviderError, RolePolicy, TablePermission};

use dashmap::DashMap;
use std::sync::Arc;

/// RBAC权限提供者
///
/// 实现基于角色的访问控制，提供基本的权限检查功能。
#[derive(Debug, Clone)]
pub struct RbacProvider {
    roles: Arc<DashMap<String, RolePolicy>>,
}

impl RbacProvider {
    /// 创建新的RBAC提供者
    pub fn new() -> Self {
        let roles = DashMap::new();

        // 添加默认管理员角色（完全访问权限）
        roles.insert(
            "admin".to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "*".to_string(),
                    operations: vec![
                        PermissionAction::Select,
                        PermissionAction::Insert,
                        PermissionAction::Update,
                        PermissionAction::Delete,
                    ],
                }],
            },
        );

        // 添加只读角色
        roles.insert(
            "readonly".to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "*".to_string(),
                    operations: vec![PermissionAction::Select],
                }],
            },
        );

        Self { roles: Arc::new(roles) }
    }

    /// 添加角色策略
    pub fn add_role(&self, role: String, policy: RolePolicy) {
        self.roles.insert(role, policy);
    }
}

impl Default for RbacProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionProvider for RbacProvider {
    fn get_role_policy(&self, role: &str) -> Option<RolePolicy> {
        self.roles.get(role).map(|p| p.clone())
    }

    fn check_access(
        &self,
        role: &str,
        table: &str,
        operation: PermissionAction,
    ) -> Result<bool, PermissionProviderError> {
        match self.roles.get(role) {
            Some(policy) => {
                for table_perm in &policy.tables {
                    if (table_perm.name == "*" || table_perm.name == table)
                        && table_perm.operations.contains(&operation)
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            None => Err(PermissionProviderError::RoleNotFound(role.to_string())),
        }
    }

    fn get_roles(&self) -> Vec<String> {
        self.roles.iter().map(|r| r.key().clone()).collect()
    }
}
