// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Advanced RBAC 权限提供者实现
//!
//! 提供基于角色的访问控制（RBAC）的高级权限提供者，支持角色继承。

use super::{PermissionAction, PermissionProvider, PermissionProviderError, RolePolicy, TablePermission};

use dashmap::DashMap;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

/// 最大继承深度限制
///
/// 防止深度继承链导致栈溢出或性能下降。
/// 当继承链深度超过此限制时，将记录警告日志并停止继续遍历。
const MAX_INHERITANCE_DEPTH: usize = 10;

/// 默认继承角色缓存容量
const DEFAULT_INHERITED_ROLES_CACHE_CAPACITY: usize = 1000;

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
    /// 创建新的高级 RBAC 提供者（使用默认缓存容量）
    pub fn new() -> Self {
        Self::with_cache_capacity(DEFAULT_INHERITED_ROLES_CACHE_CAPACITY)
    }

    /// 创建新的高级 RBAC 提供者（使用自定义缓存容量）
    ///
    /// # Arguments
    ///
    /// * `cache_capacity` - 继承角色缓存的最大条目数
    pub fn with_cache_capacity(cache_capacity: usize) -> Self {
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
            cache_capacity,
        }
    }

    /// 从 DbConfig 创建高级 RBAC 提供者
    ///
    /// 使用配置化的缓存容量。
    ///
    /// # Arguments
    ///
    /// * `config` - 数据库配置引用
    pub fn from_config(config: &crate::foundation::config::DbConfig) -> Self {
        // 使用 policy_cache_capacity 作为继承角色缓存的容量
        // 这是一个合理的默认值，因为角色数量通常远小于权限策略数量
        let cache_capacity = config.cache_config.policy_cache_capacity as usize;
        Self::with_cache_capacity(cache_capacity)
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
    /// 使用广度优先搜索（BFS）迭代遍历角色继承图，返回所有可继承的父角色。
    /// 结果会被缓存以提高性能。
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称
    ///
    /// # Returns
    ///
    /// 所有继承角色的集合（包括自身）
    pub fn get_inherited_roles(&self, role: &str) -> HashSet<String> {
        // 检查缓存
        if let Some(cached) = self.inherited_roles_cache.get(role) {
            return cached.clone();
        }

        // 使用 BFS 迭代方式计算所有继承角色
        let mut inherited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((role.to_string(), 0));

        while let Some((current, depth)) = queue.pop_front() {
            // 检查深度限制
            if depth > MAX_INHERITANCE_DEPTH {
                // 角色继承深度超过限制，停止遍历
                continue;
            }

            // 只有当角色尚未被收集时才处理
            if inherited.insert(current.clone()) {
                // 获取当前角色的父角色并加入队列
                if let Some(parents) = self.role_hierarchy.get(&current) {
                    for parent in parents.iter() {
                        queue.push_back((parent.clone(), depth + 1));
                    }
                }
            }
        }

        // 缓存结果
        if self.inherited_roles_cache.len() < self.cache_capacity {
            self.inherited_roles_cache.insert(role.to_string(), inherited.clone());
        }

        inherited
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
                    if (table_perm.name == "*" || table_perm.name == table)
                        && table_perm.operations.contains(&operation)
                    {
                        return Ok(true);
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
        assert!(inherited.iter().any(|role| role == "child"));
        assert!(inherited.iter().any(|role| role == "parent1"));
        assert!(inherited.iter().any(|role| role == "parent2"));
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

    /// TEST-ADV-016: 继承深度限制 - 深度在限制内
    #[test]
    fn test_inheritance_depth_within_limit() {
        let provider = AdvancedRbacProvider::new();

        // 创建 10 层继承链（深度 = 10，正好在限制内）
        // level_0 -> level_1 -> ... -> level_9 -> admin
        for i in 0..10 {
            provider.add_role(
                format!("level_{}", i),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: format!("table_{}", i),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            );

            if i == 0 {
                provider.add_role_inheritance(format!("level_{}", i), vec!["admin".to_string()]);
            } else {
                provider.add_role_inheritance(format!("level_{}", i), vec![format!("level_{}", i - 1)]);
            }
        }

        // level_9 应该能够继承 admin 的权限（深度 = 10，在限制内）
        assert!(
            provider
                .check_access("level_9", "users", PermissionAction::Select)
                .unwrap()
        );
    }

    /// TEST-ADV-017: 继承深度限制 - 超过限制
    #[test]
    fn test_inheritance_depth_exceeds_limit() {
        let provider = AdvancedRbacProvider::new();

        // 创建 12 层继承链（深度超过限制）
        // level_0 -> level_1 -> ... -> level_11 -> admin
        for i in 0..12 {
            provider.add_role(
                format!("deep_{}", i),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: format!("table_{}", i),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            );

            if i == 0 {
                provider.add_role_inheritance(format!("deep_{}", i), vec!["admin".to_string()]);
            } else {
                provider.add_role_inheritance(format!("deep_{}", i), vec![format!("deep_{}", i - 1)]);
            }
        }

        // deep_11 的继承链深度为 12，超过限制 10
        // 应该只能继承到深度 10 的角色，不会崩溃
        let result = provider.check_access("deep_11", "users", PermissionAction::Select);
        assert!(result.is_ok());
    }

    /// TEST-ADV-018: get_inherited_roles 返回自身
    #[test]
    fn test_get_inherited_roles_returns_self() {
        let provider = AdvancedRbacProvider::new();

        // 没有继承关系的角色应该只返回自身
        let inherited = provider.get_inherited_roles("admin");
        assert!(inherited.contains("admin"));
        assert_eq!(inherited.len(), 1);
    }

    /// TEST-ADV-019: 循环继承不导致无限循环（迭代方式）
    #[test]
    fn test_circular_inheritance_no_infinite_loop_iterative() {
        let provider = AdvancedRbacProvider::new();

        // 创建一个复杂的循环继承结构
        provider.add_role_inheritance("a".to_string(), vec!["b".to_string()]);
        provider.add_role_inheritance("b".to_string(), vec!["c".to_string()]);
        provider.add_role_inheritance("c".to_string(), vec!["d".to_string()]);
        provider.add_role_inheritance("d".to_string(), vec!["a".to_string()]);

        // 添加权限
        provider.add_role(
            "a".to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "test_table".to_string(),
                    operations: vec![PermissionAction::Select],
                }],
            },
        );

        // 应该能够处理循环而不陷入无限循环
        let result = provider.check_access("a", "test_table", PermissionAction::Select);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}
