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
    ///
    /// 注意：默认不创建任何角色，需要显式配置所有权限。
    /// 这遵循最小权限原则，避免意外授予过宽的访问权限。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let provider = RbacProvider::new();
    /// provider.add_role("admin".to_string(), RolePolicy {
    ///     tables: vec![
    ///         TablePermission {
    ///             name: "users".to_string(),
    ///             operations: vec![
    ///                 PermissionAction::Select,
    ///                 PermissionAction::Insert,
    ///                 PermissionAction::Update,
    ///                 PermissionAction::Delete,
    ///             ],
    ///         },
    ///         TablePermission {
    ///             name: "posts".to_string(),
    ///             operations: vec![PermissionAction::Select],
    ///         },
    ///     ],
    /// });
    /// ```
    pub fn new() -> Self {
        let roles = DashMap::new();
        Self { roles: Arc::new(roles) }
    }

    /// 创建带有默认管理员角色的RBAC提供者
    ///
    /// 警告：此方法创建的管理员角色具有通配符权限，仅建议在开发/测试环境中使用。
    /// 生产环境应使用 `new()` 并显式配置所需的权限。
    pub fn with_default_admin() -> Self {
        let roles = DashMap::new();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty() {
        let p = RbacProvider::new();
        assert!(p.get_roles().is_empty());
    }

    #[test]
    fn with_default_admin_creates_admin_role() {
        let p = RbacProvider::with_default_admin();
        assert_eq!(p.get_roles(), vec!["admin"]);
        assert!(p.get_role_policy("admin").is_some());
    }

    #[test]
    fn add_role_and_get_policy() {
        let p = RbacProvider::new();
        p.add_role("editor".into(), RolePolicy {
            tables: vec![TablePermission {
                name: "posts".into(),
                operations: vec![PermissionAction::Select, PermissionAction::Insert],
            }],
        });
        let policy = p.get_role_policy("editor");
        assert!(policy.is_some());
    }

    #[test]
    fn check_access_granted() {
        let p = RbacProvider::new();
        p.add_role("user".into(), RolePolicy {
            tables: vec![TablePermission {
                name: "articles".into(),
                operations: vec![PermissionAction::Select],
            }],
        });
        assert!(p.check_access("user", "articles", PermissionAction::Select).unwrap());
        assert!(!p.check_access("user", "articles", PermissionAction::Insert).unwrap());
    }

    #[test]
    fn check_access_wildcard_table_granted() {
        let p = RbacProvider::new();
        p.add_role("admin".into(), RolePolicy {
            tables: vec![TablePermission {
                name: "*".into(),
                operations: vec![PermissionAction::Select, PermissionAction::Delete],
            }],
        });
        assert!(p.check_access("admin", "any_table", PermissionAction::Select).unwrap());
        assert!(p.check_access("admin", "any_table", PermissionAction::Delete).unwrap());
    }

    #[test]
    fn check_access_role_not_found() {
        let p = RbacProvider::new();
        assert!(p.check_access("ghost", "t", PermissionAction::Select).is_err());
    }

    #[test]
    fn get_roles_returns_all() {
        let p = RbacProvider::new();
        p.add_role("a".into(), RolePolicy::default());
        p.add_role("b".into(), RolePolicy::default());
        let mut roles = p.get_roles();
        roles.sort();
        assert_eq!(roles, vec!["a", "b"]);
    }

    #[test]
    fn default_is_empty() {
        let p = RbacProvider::default();
        assert!(p.get_roles().is_empty());
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
