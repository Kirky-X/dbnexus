// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! Advanced RBAC 权限提供者实现
//!
//! 提供基于角色的访问控制（RBAC）的高级权限提供者，支持角色继承。

use super::{PermissionAction, PermissionProvider, PermissionProviderError, RolePolicy, TablePermission};

use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;

/// 高级 RBAC 权限提供者
///
/// 实现基于角色的访问控制，支持角色继承（role inheritance）。
/// 子角色可以继承父角色的权限，形成权限层级结构。
#[derive(Debug, Clone)]
pub struct AdvancedRbacProvider {
    /// 基础角色策略存储
    roles: Arc<DashMap<String, RolePolicy>>,
    /// 角色继承关系映射：子角色 -> 父角色列表
    role_hierarchy: Arc<DashMap<String, Vec<String>>>,
    /// 已计算的角色继承缓存：角色 -> 所有继承的角色（包括自身）
    inherited_roles_cache: Arc<DashMap<String, HashSet<String>>>,
    /// 缓存的最大条目数
    cache_capacity: usize,
}

impl AdvancedRbacProvider {
    /// 创建新的高级 RBAC 提供者
    pub fn new() -> Self {
        let roles = DashMap::new();
        let role_hierarchy = DashMap::new();
        let inherited_roles_cache = DashMap::new();

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

        // 添加读写角色
        roles.insert(
            "readwrite".to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "*".to_string(),
                    operations: vec![
                        PermissionAction::Select,
                        PermissionAction::Insert,
                        PermissionAction::Update,
                    ],
                }],
            },
        );

        Self {
            roles: Arc::new(roles),
            role_hierarchy: Arc::new(role_hierarchy),
            inherited_roles_cache: Arc::new(inherited_roles_cache),
            cache_capacity: 1000,
        }
    }

    /// 添加角色策略
    pub fn add_role(&self, role: String, policy: RolePolicy) {
        self.roles.insert(role.clone(), policy);
        // 清除相关缓存
        self.inherited_roles_cache.remove(&role);
    }

    /// 添加角色继承关系
    ///
    /// # Arguments
    ///
    /// * `child_role` - 子角色名称
    /// * `parent_roles` - 父角色列表（这些角色的权限将被继承）
    ///
    /// # Example
    ///
    /// ```ignore
    /// let provider = AdvancedRbacProvider::new();
    /// // manager 继承 admin 的权限
    /// provider.add_role_inheritance("manager", vec!["admin".to_string()]);
    /// ```
    pub fn add_role_inheritance(&self, child_role: String, parent_roles: Vec<String>) {
        self.role_hierarchy.insert(child_role.clone(), parent_roles);
        // 清除相关缓存
        self.inherited_roles_cache.remove(&child_role);
    }

    /// 批量设置角色继承关系
    ///
    /// 一次性设置多个角色的继承关系，用于配置复杂的权限层级。
    ///
    /// # Arguments
    ///
    /// * `inheritances` - 继承关系映射：子角色 -> 父角色列表
    pub fn set_role_inheritances(&self, inheritances: Vec<(String, Vec<String>)>) {
        for (child, parents) in inheritances {
            self.role_hierarchy.insert(child.clone(), parents);
            self.inherited_roles_cache.remove(&child);
        }
    }

    /// 获取角色的所有继承角色（包括自身）
    ///
    /// 使用深度优先搜索遍历角色继承图，返回所有可继承的父角色。
    /// 结果会被缓存以提高性能。
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称
    ///
    /// # Returns
    ///
    /// 所有继承角色的集合（包括自身）
    fn get_inherited_roles(&self, role: &str) -> HashSet<String> {
        // 检查缓存
        if let Some(cached) = self.inherited_roles_cache.get(role) {
            return cached.clone();
        }

        // 计算所有继承角色
        let mut inherited = HashSet::new();
        inherited.insert(role.to_string());

        // 递归获取父角色
        self.collect_parent_roles(role, &mut inherited);

        // 缓存结果
        if self.inherited_roles_cache.len() < self.cache_capacity {
            self.inherited_roles_cache.insert(role.to_string(), inherited.clone());
        }

        inherited
    }

    /// 递归收集父角色
    fn collect_parent_roles(&self, role: &str, collected: &mut HashSet<String>) {
        if let Some(parents) = self.role_hierarchy.get(role) {
            for parent in parents.iter() {
                if !collected.contains(parent) {
                    collected.insert(parent.clone());
                    self.collect_parent_roles(parent, collected);
                }
            }
        }
    }

    /// 检查角色是否有继承关系
    pub fn has_inheritance(&self, role: &str) -> bool {
        self.role_hierarchy.contains_key(role)
    }

    /// 获取指定角色的所有直接父角色
    pub fn get_direct_parents(&self, role: &str) -> Option<Vec<String>> {
        self.role_hierarchy.get(role).map(|p| p.clone())
    }

    /// 移除角色的继承关系
    pub fn remove_inheritance(&self, role: &str) -> bool {
        let removed = self.role_hierarchy.remove(role).is_some();
        if removed {
            self.inherited_roles_cache.remove(role);
        }
        removed
    }

    /// 清除继承缓存
    pub fn clear_cache(&self) {
        self.inherited_roles_cache.clear();
    }

    /// 获取继承缓存大小
    pub fn cache_size(&self) -> usize {
        self.inherited_roles_cache.len()
    }

    /// 检查角色是否有权限（考虑继承）
    ///
    /// 此方法会检查角色本身及其所有父角色的权限。
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称
    /// * `table` - 表名
    /// * `operation` - 操作类型
    ///
    /// # Returns
    ///
    /// - `Ok(true)` - 有权限
    /// - `Ok(false)` - 无权限
    /// - `Err(_)` - 检查失败
    fn check_role_access(
        &self,
        role: &str,
        table: &str,
        operation: PermissionAction,
    ) -> Result<bool, PermissionProviderError> {
        match self.roles.get(role) {
            Some(policy) => {
                for table_perm in &policy.tables {
                    if table_perm.name == "*" || table_perm.name == table {
                        if table_perm.operations.contains(&operation) {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }
            None => Ok(false), // 角色不存在，不报错，继续检查其他继承角色
        }
    }
}

impl Default for AdvancedRbacProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionProvider for AdvancedRbacProvider {
    fn get_role_policy(&self, role: &str) -> Option<RolePolicy> {
        self.roles.get(role).map(|p| p.clone())
    }

    fn check_access(
        &self,
        role: &str,
        table: &str,
        operation: PermissionAction,
    ) -> Result<bool, PermissionProviderError> {
        // 获取角色的所有继承角色
        let inherited_roles = self.get_inherited_roles(role);

        // 逐个检查每个继承角色的权限
        for inherited_role in inherited_roles.iter() {
            match self.check_role_access(inherited_role, table, operation.clone()) {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(e) => return Err(e),
            }
        }

        Ok(false)
    }

    fn get_roles(&self) -> Vec<String> {
        self.roles.iter().map(|r| r.key().clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-ADV-001: 基础角色权限检查
    #[test]
    fn test_advanced_rbac_basic_check() {
        let provider = AdvancedRbacProvider::new();

        // Admin 应该可以访问所有表的所有操作
        assert!(
            provider
                .check_access("admin", "users", PermissionAction::Select)
                .unwrap()
        );
        assert!(
            provider
                .check_access("admin", "users", PermissionAction::Insert)
                .unwrap()
        );
        assert!(
            provider
                .check_access("admin", "users", PermissionAction::Update)
                .unwrap()
        );
        assert!(
            provider
                .check_access("admin", "users", PermissionAction::Delete)
                .unwrap()
        );

        // Readonly 应该只能 SELECT
        assert!(
            provider
                .check_access("readonly", "users", PermissionAction::Select)
                .unwrap()
        );
        assert!(
            !provider
                .check_access("readonly", "users", PermissionAction::Insert)
                .unwrap()
        );
        assert!(
            !provider
                .check_access("readonly", "users", PermissionAction::Update)
                .unwrap()
        );
        assert!(
            !provider
                .check_access("readonly", "users", PermissionAction::Delete)
                .unwrap()
        );
    }

    /// TEST-ADV-002: 角色继承 - 子角色继承父角色权限
    #[test]
    fn test_role_inheritance() {
        let provider = AdvancedRbacProvider::new();

        // 设置 manager 继承 admin 的权限
        provider.add_role_inheritance("manager".to_string(), vec!["admin".to_string()]);

        // Manager 应该拥有 admin 的所有权限
        assert!(
            provider
                .check_access("manager", "users", PermissionAction::Select)
                .unwrap()
        );
        assert!(
            provider
                .check_access("manager", "users", PermissionAction::Insert)
                .unwrap()
        );
        assert!(
            provider
                .check_access("manager", "users", PermissionAction::Delete)
                .unwrap()
        );
    }

    /// TEST-ADV-003: 多重继承 - 角色继承多个父角色
    #[test]
    fn test_multiple_inheritance() {
        let provider = AdvancedRbacProvider::new();

        // 设置 senior 继承 readonly 和 readwrite 的权限
        provider.add_role_inheritance(
            "senior".to_string(),
            vec!["readonly".to_string(), "readwrite".to_string()],
        );

        // Senior 应该拥有 readonly 的 SELECT 权限
        assert!(
            provider
                .check_access("senior", "users", PermissionAction::Select)
                .unwrap()
        );

        // Senior 应该拥有 readwrite 的 INSERT/UPDATE 权限
        assert!(
            provider
                .check_access("senior", "users", PermissionAction::Insert)
                .unwrap()
        );
        assert!(
            provider
                .check_access("senior", "users", PermissionAction::Update)
                .unwrap()
        );

        // Senior 不应该有 DELETE 权限（因为没有继承 admin）
        assert!(
            !provider
                .check_access("senior", "users", PermissionAction::Delete)
                .unwrap()
        );
    }

    /// TEST-ADV-004: 继承链 - 多层角色继承
    #[test]
    fn test_inheritance_chain() {
        let provider = AdvancedRbacProvider::new();

        // 设置继承链：junior -> mid -> senior -> admin
        provider.add_role_inheritance("junior".to_string(), vec!["mid".to_string()]);
        provider.add_role_inheritance("mid".to_string(), vec!["senior".to_string()]);
        provider.add_role_inheritance("senior".to_string(), vec!["admin".to_string()]);

        // Junior 应该最终继承 admin 的所有权限
        assert!(
            provider
                .check_access("junior", "users", PermissionAction::Select)
                .unwrap()
        );
        assert!(
            provider
                .check_access("junior", "users", PermissionAction::Insert)
                .unwrap()
        );
        assert!(
            provider
                .check_access("junior", "users", PermissionAction::Delete)
                .unwrap()
        );
    }

    /// TEST-ADV-005: 循环继承检测
    #[test]
    fn test_circular_inheritance() {
        let provider = AdvancedRbacProvider::new();

        // 设置循环：A -> B -> A
        provider.add_role_inheritance("role_a".to_string(), vec!["role_b".to_string()]);
        provider.add_role_inheritance("role_b".to_string(), vec!["role_a".to_string()]);

        // 添加一个实际的策略
        provider.add_role(
            "role_a".to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "test".to_string(),
                    operations: vec![PermissionAction::Select],
                }],
            },
        );

        // 应该能够处理循环而不陷入无限递归
        let result = provider.check_access("role_a", "test", PermissionAction::Select);
        assert!(result.is_ok());
    }

    /// TEST-ADV-006: get_inherited_roles 方法
    #[test]
    fn test_get_inherited_roles() {
        let provider = AdvancedRbacProvider::new();

        // 设置继承关系
        provider.add_role_inheritance("child".to_string(), vec!["parent1".to_string(), "parent2".to_string()]);

        let inherited = provider.get_inherited_roles("child");
        assert!(inherited.contains(&"child".to_string()));
        assert!(inherited.contains(&"parent1".to_string()));
        assert!(inherited.contains(&"parent2".to_string()));
        assert_eq!(inherited.len(), 3);
    }

    /// TEST-ADV-007: has_inheritance 方法
    #[test]
    fn test_has_inheritance() {
        let provider = AdvancedRbacProvider::new();

        assert!(!provider.has_inheritance("admin"));

        provider.add_role_inheritance("manager".to_string(), vec!["admin".to_string()]);
        assert!(provider.has_inheritance("manager"));
    }

    /// TEST-ADV-008: remove_inheritance 方法
    #[test]
    fn test_remove_inheritance() {
        let provider = AdvancedRbacProvider::new();

        provider.add_role_inheritance("manager".to_string(), vec!["admin".to_string()]);
        assert!(provider.has_inheritance("manager"));

        let removed = provider.remove_inheritance("manager");
        assert!(removed);
        assert!(!provider.has_inheritance("manager"));
    }

    /// TEST-ADV-009: 缓存功能
    #[test]
    fn test_cache_functionality() {
        let provider = AdvancedRbacProvider::new();

        // 初始缓存为空
        assert_eq!(provider.cache_size(), 0);

        // 首次调用会填充缓存
        provider
            .check_access("admin", "users", PermissionAction::Select)
            .unwrap();

        // 缓存应该有内容了
        assert!(provider.cache_size() > 0);

        // 清除缓存
        provider.clear_cache();
        assert_eq!(provider.cache_size(), 0);
    }

    /// TEST-ADV-010: 自定义角色添加
    #[test]
    fn test_custom_role_addition() {
        let provider = AdvancedRbacProvider::new();

        // 添加自定义角色
        provider.add_role(
            "custom_role".to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "custom_table".to_string(),
                    operations: vec![PermissionAction::Select, PermissionAction::Update],
                }],
            },
        );

        // 检查自定义角色权限
        assert!(
            provider
                .check_access("custom_role", "custom_table", PermissionAction::Select)
                .unwrap()
        );
        assert!(
            provider
                .check_access("custom_role", "custom_table", PermissionAction::Update)
                .unwrap()
        );
        assert!(
            !provider
                .check_access("custom_role", "custom_table", PermissionAction::Insert)
                .unwrap()
        );
        assert!(
            !provider
                .check_access("custom_role", "custom_table", PermissionAction::Delete)
                .unwrap()
        );
    }

    /// TEST-ADV-011: 继承自定义角色
    #[test]
    fn test_inherit_custom_role() {
        let provider = AdvancedRbacProvider::new();

        // 添加自定义角色
        provider.add_role(
            "custom_role".to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "custom_table".to_string(),
                    operations: vec![PermissionAction::Select],
                }],
            },
        );

        // 让另一个角色继承自定义角色
        provider.add_role_inheritance("inherited_role".to_string(), vec!["custom_role".to_string()]);

        // 验证继承
        assert!(
            provider
                .check_access("inherited_role", "custom_table", PermissionAction::Select)
                .unwrap()
        );
    }

    /// TEST-ADV-012: get_roles 方法
    #[test]
    fn test_get_roles() {
        let provider = AdvancedRbacProvider::new();

        let roles = provider.get_roles();
        assert!(roles.contains(&"admin".to_string()));
        assert!(roles.contains(&"readonly".to_string()));
        assert!(roles.contains(&"readwrite".to_string()));
    }

    /// TEST-ADV-013: 批量设置继承关系
    #[test]
    fn test_batch_inheritances() {
        let provider = AdvancedRbacProvider::new();

        provider.set_role_inheritances(vec![
            ("role_a".to_string(), vec!["admin".to_string()]),
            ("role_b".to_string(), vec!["readonly".to_string()]),
            ("role_c".to_string(), vec!["role_a".to_string(), "role_b".to_string()]),
        ]);

        // Role C 应该继承 Role A 和 Role B 的权限
        assert!(
            provider
                .check_access("role_c", "users", PermissionAction::Insert)
                .unwrap()
        ); // from role_a -> admin
        assert!(
            provider
                .check_access("role_c", "users", PermissionAction::Select)
                .unwrap()
        ); // from role_b -> readonly
    }

    /// TEST-ADV-014: 未定义角色的访问拒绝
    #[test]
    fn test_undefined_role_denied() {
        let provider = AdvancedRbacProvider::new();

        // 未定义的 Role 不应该有权限
        assert!(
            !provider
                .check_access("undefined_role", "users", PermissionAction::Select)
                .unwrap()
        );
    }

    /// TEST-ADV-015: 通配符表名匹配
    #[test]
    fn test_wildcard_table_matching() {
        let provider = AdvancedRbacProvider::new();

        provider.add_role_inheritance("manager".to_string(), vec!["admin".to_string()]);

        // Admin 有 "*" 权限，应该可以访问任何表
        assert!(
            provider
                .check_access("manager", "any_table", PermissionAction::Select)
                .unwrap()
        );
        assert!(
            provider
                .check_access("manager", "another_table", PermissionAction::Insert)
                .unwrap()
        );
        assert!(
            provider
                .check_access("manager", "third_table", PermissionAction::Delete)
                .unwrap()
        );
    }
}
