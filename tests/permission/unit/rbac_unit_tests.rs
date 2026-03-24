// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! RBAC 权限提供者单元测试
//!
//! 测试 RbacProvider 的核心功能，包括：
//! - 角色创建和删除 (CRUD)
//! - 权限检查
//! - 通配符匹配
//! - 边界条件和错误处理

use dbnexus::access::permission::{
    PermissionAction, PermissionProvider, PermissionProviderError, RbacProvider, RolePolicy, TablePermission,
};

// ============================================================================
// RbacProvider CRUD 测试
// ============================================================================

/// TEST-RBAC-U-001: 创建空的 RBAC 提供者
#[test]
fn test_rbac_provider_new_empty() {
    let provider = RbacProvider::new();

    // 新创建的提供者应该没有任何角色
    let roles = provider.get_roles();
    assert!(roles.is_empty(), "New provider should have no roles");
}

/// TEST-RBAC-U-002: 添加角色策略
#[test]
fn test_rbac_provider_add_role() {
    let provider = RbacProvider::new();

    // 添加 admin 角色
    let admin_policy = RolePolicy {
        tables: vec![TablePermission {
            name: "*".to_string(),
            operations: vec![
                PermissionAction::Select,
                PermissionAction::Insert,
                PermissionAction::Update,
                PermissionAction::Delete,
            ],
        }],
    };
    provider.add_role("admin".to_string(), admin_policy);

    // 验证角色已添加
    let roles = provider.get_roles();
    assert_eq!(roles.len(), 1);
    assert!(roles.contains(&"admin".to_string()));

    // 验证可以获取角色策略
    let policy = provider.get_role_policy("admin");
    assert!(policy.is_some());
}

/// TEST-RBAC-U-003: 添加多个角色
#[test]
fn test_rbac_provider_add_multiple_roles() {
    let provider = RbacProvider::new();

    // 添加 admin 角色
    provider.add_role(
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

    // 添加 readonly 角色
    provider.add_role(
        "readonly".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "*".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    // 添加 user 角色
    provider.add_role(
        "user".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "users".to_string(),
                operations: vec![PermissionAction::Select, PermissionAction::Insert],
            }],
        },
    );

    // 验证所有角色都已添加
    let roles = provider.get_roles();
    assert_eq!(roles.len(), 3);
    assert!(roles.contains(&"admin".to_string()));
    assert!(roles.contains(&"readonly".to_string()));
    assert!(roles.contains(&"user".to_string()));
}

/// TEST-RBAC-U-004: 覆盖已存在的角色
#[test]
fn test_rbac_provider_override_role() {
    let provider = RbacProvider::new();

    // 添加初始角色
    provider.add_role(
        "test_role".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "table1".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    // 覆盖角色
    provider.add_role(
        "test_role".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "table2".to_string(),
                operations: vec![PermissionAction::Insert],
            }],
        },
    );

    // 验证角色被覆盖
    let policy = provider.get_role_policy("test_role").unwrap();
    assert_eq!(policy.tables.len(), 1);
    assert_eq!(policy.tables[0].name, "table2");
    assert_eq!(policy.tables[0].operations, vec![PermissionAction::Insert]);
}

/// TEST-RBAC-U-005: 获取不存在的角色策略
#[test]
fn test_rbac_provider_get_nonexistent_role() {
    let provider = RbacProvider::new();

    // 获取不存在的角色应该返回 None
    let policy = provider.get_role_policy("nonexistent");
    assert!(policy.is_none());
}

/// TEST-RBAC-U-006: 使用默认管理员创建提供者
#[test]
fn test_rbac_provider_with_default_admin() {
    let provider = RbacProvider::with_default_admin();

    // 应该有一个 admin 角色
    let roles = provider.get_roles();
    assert_eq!(roles.len(), 1);
    assert!(roles.contains(&"admin".to_string()));

    // admin 应该有通配符权限
    let policy = provider.get_role_policy("admin").unwrap();
    assert_eq!(policy.tables.len(), 1);
    assert_eq!(policy.tables[0].name, "*");
    assert_eq!(policy.tables[0].operations.len(), 4);
}

// ============================================================================
// 权限检查测试
// ============================================================================

/// TEST-RBAC-U-007: 基本权限检查 - 允许
#[test]
fn test_rbac_check_access_allowed() {
    let provider = RbacProvider::new();

    provider.add_role(
        "user".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "users".to_string(),
                operations: vec![PermissionAction::Select, PermissionAction::Insert],
            }],
        },
    );

    // 检查允许的操作
    let result = provider.check_access("user", "users", PermissionAction::Select);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let result = provider.check_access("user", "users", PermissionAction::Insert);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

/// TEST-RBAC-U-008: 基本权限检查 - 拒绝
#[test]
fn test_rbac_check_access_denied() {
    let provider = RbacProvider::new();

    provider.add_role(
        "user".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "users".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    // 检查拒绝的操作
    let result = provider.check_access("user", "users", PermissionAction::Delete);
    assert!(result.is_ok());
    assert!(!result.unwrap());

    let result = provider.check_access("user", "users", PermissionAction::Update);
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

/// TEST-RBAC-U-009: 不存在的角色权限检查
#[test]
fn test_rbac_check_access_nonexistent_role() {
    let provider = RbacProvider::new();

    // 检查不存在的角色应该返回错误
    let result = provider.check_access("ghost", "users", PermissionAction::Select);
    assert!(result.is_err());

    match result {
        Err(PermissionProviderError::RoleNotFound(role)) => assert_eq!(role, "ghost"),
        _ => panic!("Expected RoleNotFound error"),
    }
}

/// TEST-RBAC-U-010: 不存在的表权限检查
#[test]
fn test_rbac_check_access_nonexistent_table() {
    let provider = RbacProvider::new();

    provider.add_role(
        "user".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "users".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    // 检查不存在的表应该返回 false（无权限）
    let result = provider.check_access("user", "orders", PermissionAction::Select);
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

// ============================================================================
// 通配符匹配测试
// ============================================================================

/// TEST-RBAC-U-011: 通配符表名匹配
#[test]
fn test_rbac_wildcard_table_matching() {
    let provider = RbacProvider::new();

    provider.add_role(
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

    // 通配符应该匹配任何表名
    assert!(
        provider
            .check_access("admin", "users", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("admin", "orders", PermissionAction::Insert)
            .unwrap()
    );
    assert!(
        provider
            .check_access("admin", "products", PermissionAction::Update)
            .unwrap()
    );
    assert!(
        provider
            .check_access("admin", "logs", PermissionAction::Delete)
            .unwrap()
    );
    assert!(
        provider
            .check_access("admin", "any_table_name", PermissionAction::Select)
            .unwrap()
    );
}

/// TEST-RBAC-U-012: 混合通配符和精确表名
#[test]
fn test_rbac_mixed_wildcard_and_exact() {
    let provider = RbacProvider::new();

    provider.add_role(
        "mixed_role".to_string(),
        RolePolicy {
            tables: vec![
                TablePermission {
                    name: "users".to_string(),
                    operations: vec![PermissionAction::Select, PermissionAction::Insert],
                },
                TablePermission {
                    name: "*".to_string(),
                    operations: vec![PermissionAction::Select],
                },
            ],
        },
    );

    // users 表应该有 SELECT 和 INSERT 权限
    assert!(
        provider
            .check_access("mixed_role", "users", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("mixed_role", "users", PermissionAction::Insert)
            .unwrap()
    );

    // 其他表只有 SELECT 权限
    assert!(
        provider
            .check_access("mixed_role", "orders", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        !provider
            .check_access("mixed_role", "orders", PermissionAction::Insert)
            .unwrap()
    );
}

// ============================================================================
// 边界条件和错误处理测试
// ============================================================================

/// TEST-RBAC-U-013: 空操作列表
#[test]
fn test_rbac_empty_operations() {
    let provider = RbacProvider::new();

    provider.add_role(
        "empty_ops".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "users".to_string(),
                operations: vec![],
            }],
        },
    );

    // 空操作列表意味着没有任何权限
    assert!(
        !provider
            .check_access("empty_ops", "users", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        !provider
            .check_access("empty_ops", "users", PermissionAction::Insert)
            .unwrap()
    );
}

/// TEST-RBAC-U-014: 空表权限列表
#[test]
fn test_rbac_empty_tables() {
    let provider = RbacProvider::new();

    provider.add_role("empty_tables".to_string(), RolePolicy { tables: vec![] });

    // 空表列表意味着没有任何权限
    assert!(
        !provider
            .check_access("empty_tables", "users", PermissionAction::Select)
            .unwrap()
    );
}

/// TEST-RBAC-U-015: 多表权限检查
#[test]
fn test_rbac_multiple_table_permissions() {
    let provider = RbacProvider::new();

    provider.add_role(
        "multi_table".to_string(),
        RolePolicy {
            tables: vec![
                TablePermission {
                    name: "users".to_string(),
                    operations: vec![PermissionAction::Select],
                },
                TablePermission {
                    name: "orders".to_string(),
                    operations: vec![PermissionAction::Select, PermissionAction::Insert],
                },
                TablePermission {
                    name: "products".to_string(),
                    operations: vec![
                        PermissionAction::Select,
                        PermissionAction::Insert,
                        PermissionAction::Update,
                    ],
                },
            ],
        },
    );

    // users 表
    assert!(
        provider
            .check_access("multi_table", "users", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        !provider
            .check_access("multi_table", "users", PermissionAction::Insert)
            .unwrap()
    );

    // orders 表
    assert!(
        provider
            .check_access("multi_table", "orders", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("multi_table", "orders", PermissionAction::Insert)
            .unwrap()
    );
    assert!(
        !provider
            .check_access("multi_table", "orders", PermissionAction::Delete)
            .unwrap()
    );

    // products 表
    assert!(
        provider
            .check_access("multi_table", "products", PermissionAction::Update)
            .unwrap()
    );
    assert!(
        !provider
            .check_access("multi_table", "products", PermissionAction::Delete)
            .unwrap()
    );
}

/// TEST-RBAC-U-016: 角色名称大小写敏感
#[test]
fn test_rbac_role_name_case_sensitive() {
    let provider = RbacProvider::new();

    provider.add_role(
        "Admin".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "users".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    // 角色名称应该区分大小写
    assert!(provider.get_role_policy("Admin").is_some());
    assert!(provider.get_role_policy("admin").is_none());
    assert!(provider.get_role_policy("ADMIN").is_none());
}

/// TEST-RBAC-U-017: 表名大小写敏感
#[test]
fn test_rbac_table_name_case_sensitive() {
    let provider = RbacProvider::new();

    provider.add_role(
        "user".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "Users".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    // 表名应该区分大小写
    assert!(
        provider
            .check_access("user", "Users", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        !provider
            .check_access("user", "users", PermissionAction::Select)
            .unwrap()
    );
}

/// TEST-RBAC-U-018: 特殊字符表名
#[test]
fn test_rbac_special_character_table_names() {
    let provider = RbacProvider::new();

    provider.add_role(
        "special".to_string(),
        RolePolicy {
            tables: vec![
                TablePermission {
                    name: "user_data".to_string(),
                    operations: vec![PermissionAction::Select],
                },
                TablePermission {
                    name: "order-items".to_string(),
                    operations: vec![PermissionAction::Select],
                },
                TablePermission {
                    name: "table.with.dots".to_string(),
                    operations: vec![PermissionAction::Select],
                },
            ],
        },
    );

    // 特殊字符表名应该正常工作
    assert!(
        provider
            .check_access("special", "user_data", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("special", "order-items", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("special", "table.with.dots", PermissionAction::Select)
            .unwrap()
    );
}

/// TEST-RBAC-U-019: has_role 方法测试
#[test]
fn test_rbac_has_role() {
    let provider = RbacProvider::new();

    provider.add_role(
        "test_role".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "users".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    assert!(provider.has_role("test_role"));
    assert!(!provider.has_role("nonexistent_role"));
}

/// TEST-RBAC-U-020: 并发访问测试
#[test]
fn test_rbac_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let provider = Arc::new(RbacProvider::new());

    // 添加初始角色
    provider.add_role(
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

    let mut handles = vec![];

    // 启动多个线程进行并发读取
    for i in 0..10 {
        let p = provider.clone();
        let handle = thread::spawn(move || {
            let result = p.check_access("admin", "users", PermissionAction::Select);
            assert!(result.is_ok());
            assert!(result.unwrap());
            i
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }
}
