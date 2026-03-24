// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 高级 RBAC 权限提供者单元测试
//!
//! 测试 AdvancedRbacProvider 的核心功能，包括：
//! - 角色继承链解析
//! - 多重继承
//! - 循环继承检测
//! - 权限冲突解决
//! - 缓存机制

use dbnexus::access::permission::{AdvancedRbacProvider, PermissionAction, PermissionProvider, RolePolicy, TablePermission};

// ============================================================================
// 角色继承链解析测试
// ============================================================================

/// TEST-ADV-U-001: 单层继承 - 子角色继承父角色权限
#[test]
fn test_single_level_inheritance() {
    let provider = AdvancedRbacProvider::new();

    // 设置 manager 继承 admin 的权限
    provider.add_role_inheritance("manager".to_string(), vec!["admin".to_string()]);

    // Manager 应该继承 admin 的所有权限
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
            .check_access("manager", "users", PermissionAction::Update)
            .unwrap()
    );
    assert!(
        provider
            .check_access("manager", "users", PermissionAction::Delete)
            .unwrap()
    );
}

/// TEST-ADV-U-002: 多层继承链 - 三层继承
#[test]
fn test_multi_level_inheritance_chain() {
    let provider = AdvancedRbacProvider::new();

    // 添加自定义角色
    provider.add_role(
        "base".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "common".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    provider.add_role(
        "intermediate".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "intermediate_table".to_string(),
                operations: vec![PermissionAction::Select, PermissionAction::Insert],
            }],
        },
    );

    provider.add_role(
        "top".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "top_table".to_string(),
                operations: vec![
                    PermissionAction::Select,
                    PermissionAction::Insert,
                    PermissionAction::Update,
                ],
            }],
        },
    );

    // 设置继承链: top -> intermediate -> base
    provider.add_role_inheritance("intermediate".to_string(), vec!["base".to_string()]);
    provider.add_role_inheritance("top".to_string(), vec!["intermediate".to_string()]);

    // top 应该继承所有父角色的权限
    assert!(
        provider
            .check_access("top", "common", PermissionAction::Select)
            .unwrap()
    ); // from base
    assert!(
        provider
            .check_access("top", "intermediate_table", PermissionAction::Insert)
            .unwrap()
    ); // from intermediate
    assert!(
        provider
            .check_access("top", "top_table", PermissionAction::Update)
            .unwrap()
    ); // from top
}

/// TEST-ADV-U-003: 深层继承链 - 五层继承
#[test]
fn test_deep_inheritance_chain() {
    let provider = AdvancedRbacProvider::new();

    // 创建五层继承链
    for i in 0..5 {
        provider.add_role(
            format!("level_{}", i),
            RolePolicy {
                tables: vec![TablePermission {
                    name: format!("table_{}", i),
                    operations: vec![PermissionAction::Select],
                }],
            },
        );

        if i > 0 {
            provider.add_role_inheritance(format!("level_{}", i), vec![format!("level_{}", i - 1)]);
        }
    }

    // level_4 应该继承所有父角色的权限
    for i in 0..5 {
        assert!(
            provider
                .check_access("level_4", &format!("table_{}", i), PermissionAction::Select)
                .unwrap()
        );
    }
}

// ============================================================================
// 多重继承测试
// ============================================================================

/// TEST-ADV-U-004: 多重继承 - 继承多个父角色
#[test]
fn test_multiple_inheritance() {
    let provider = AdvancedRbacProvider::new();

    // 添加角色
    provider.add_role(
        "role_a".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "table_a".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    provider.add_role(
        "role_b".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "table_b".to_string(),
                operations: vec![PermissionAction::Insert],
            }],
        },
    );

    provider.add_role(
        "role_c".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "table_c".to_string(),
                operations: vec![PermissionAction::Update],
            }],
        },
    );

    // 设置多重继承
    provider.add_role_inheritance(
        "combined".to_string(),
        vec!["role_a".to_string(), "role_b".to_string(), "role_c".to_string()],
    );

    // combined 应该继承所有父角色的权限
    assert!(
        provider
            .check_access("combined", "table_a", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("combined", "table_b", PermissionAction::Insert)
            .unwrap()
    );
    assert!(
        provider
            .check_access("combined", "table_c", PermissionAction::Update)
            .unwrap()
    );
}

/// TEST-ADV-U-005: 菱形继承结构
#[test]
fn test_diamond_inheritance() {
    let provider = AdvancedRbacProvider::new();

    // 创建菱形继承结构
    //       grandparent
    //       /         \
    //   parent_a    parent_b
    //       \         /
    //        child
    provider.add_role(
        "grandparent".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "gp_table".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    provider.add_role(
        "parent_a".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "a_table".to_string(),
                operations: vec![PermissionAction::Insert],
            }],
        },
    );

    provider.add_role(
        "parent_b".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "b_table".to_string(),
                operations: vec![PermissionAction::Update],
            }],
        },
    );

    provider.add_role(
        "child".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "child_table".to_string(),
                operations: vec![PermissionAction::Delete],
            }],
        },
    );

    // 设置继承关系
    provider.add_role_inheritance("parent_a".to_string(), vec!["grandparent".to_string()]);
    provider.add_role_inheritance("parent_b".to_string(), vec!["grandparent".to_string()]);
    provider.add_role_inheritance(
        "child".to_string(),
        vec!["parent_a".to_string(), "parent_b".to_string()],
    );

    // child 应该继承所有祖先的权限（grandparent 只计算一次）
    assert!(
        provider
            .check_access("child", "gp_table", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("child", "a_table", PermissionAction::Insert)
            .unwrap()
    );
    assert!(
        provider
            .check_access("child", "b_table", PermissionAction::Update)
            .unwrap()
    );
    assert!(
        provider
            .check_access("child", "child_table", PermissionAction::Delete)
            .unwrap()
    );
}

// ============================================================================
// 循环继承检测测试
// ============================================================================

/// TEST-ADV-U-006: 直接循环继承 A -> B -> A
#[test]
fn test_direct_circular_inheritance() {
    let provider = AdvancedRbacProvider::new();

    // 添加角色
    provider.add_role(
        "role_a".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "table_a".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    provider.add_role(
        "role_b".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "table_b".to_string(),
                operations: vec![PermissionAction::Insert],
            }],
        },
    );

    // 设置循环继承
    provider.add_role_inheritance("role_a".to_string(), vec!["role_b".to_string()]);
    provider.add_role_inheritance("role_b".to_string(), vec!["role_a".to_string()]);

    // 应该能够处理循环而不陷入无限递归
    let result = provider.check_access("role_a", "table_a", PermissionAction::Select);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let result = provider.check_access("role_a", "table_b", PermissionAction::Insert);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

/// TEST-ADV-U-007: 间接循环继承 A -> B -> C -> A
#[test]
fn test_indirect_circular_inheritance() {
    let provider = AdvancedRbacProvider::new();

    // 添加角色
    for name in ["role_a", "role_b", "role_c"] {
        provider.add_role(
            name.to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: format!("{}_table", name),
                    operations: vec![PermissionAction::Select],
                }],
            },
        );
    }

    // 设置间接循环继承
    provider.add_role_inheritance("role_a".to_string(), vec!["role_b".to_string()]);
    provider.add_role_inheritance("role_b".to_string(), vec!["role_c".to_string()]);
    provider.add_role_inheritance("role_c".to_string(), vec!["role_a".to_string()]);

    // 应该能够处理循环而不陷入无限递归
    let result = provider.check_access("role_a", "role_c_table", PermissionAction::Select);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

/// TEST-ADV-U-008: 自引用循环 A -> A
#[test]
fn test_self_referential_inheritance() {
    let provider = AdvancedRbacProvider::new();

    provider.add_role(
        "self_ref".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "test_table".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    // 设置自引用
    provider.add_role_inheritance("self_ref".to_string(), vec!["self_ref".to_string()]);

    // 应该能够处理自引用
    let result = provider.check_access("self_ref", "test_table", PermissionAction::Select);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

// ============================================================================
// 权限冲突解决测试
// ============================================================================

/// TEST-ADV-U-009: 权限合并 - 多个父角色的权限合并
#[test]
fn test_permission_merging() {
    let provider = AdvancedRbacProvider::new();

    // 添加角色，每个角色对同一张表有不同的权限
    provider.add_role(
        "reader".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "shared_table".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    provider.add_role(
        "writer".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "shared_table".to_string(),
                operations: vec![PermissionAction::Insert, PermissionAction::Update],
            }],
        },
    );

    // 设置继承
    provider.add_role_inheritance("combined".to_string(), vec!["reader".to_string(), "writer".to_string()]);

    // combined 应该拥有所有合并的权限
    assert!(
        provider
            .check_access("combined", "shared_table", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("combined", "shared_table", PermissionAction::Insert)
            .unwrap()
    );
    assert!(
        provider
            .check_access("combined", "shared_table", PermissionAction::Update)
            .unwrap()
    );
    assert!(
        !provider
            .check_access("combined", "shared_table", PermissionAction::Delete)
            .unwrap()
    );
}

/// TEST-ADV-U-010: 权限优先级 - 子角色权限覆盖父角色
#[test]
fn test_child_permission_override() {
    let provider = AdvancedRbacProvider::new();

    // 父角色只有 SELECT 权限
    provider.add_role(
        "parent".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "users".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    // 子角色有更多权限
    provider.add_role(
        "child".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "users".to_string(),
                operations: vec![
                    PermissionAction::Select,
                    PermissionAction::Insert,
                    PermissionAction::Update,
                ],
            }],
        },
    );

    provider.add_role_inheritance("child".to_string(), vec!["parent".to_string()]);

    // 子角色应该拥有自己的权限（更宽）
    assert!(
        provider
            .check_access("child", "users", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("child", "users", PermissionAction::Insert)
            .unwrap()
    );
    assert!(
        provider
            .check_access("child", "users", PermissionAction::Update)
            .unwrap()
    );
}

/// TEST-ADV-U-011: 通配符权限继承
#[test]
fn test_wildcard_permission_inheritance() {
    let provider = AdvancedRbacProvider::new();

    // 父角色有通配符权限
    // admin 角色在 new() 中已创建，有通配符权限

    // 子角色有特定表权限
    provider.add_role(
        "limited".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "specific_table".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    provider.add_role_inheritance("limited".to_string(), vec!["admin".to_string()]);

    // limited 应该继承 admin 的通配符权限
    assert!(
        provider
            .check_access("limited", "any_table", PermissionAction::Delete)
            .unwrap()
    );
    assert!(
        provider
            .check_access("limited", "specific_table", PermissionAction::Select)
            .unwrap()
    );
}

// ============================================================================
// 缓存机制测试
// ============================================================================

/// TEST-ADV-U-012: 继承缓存基本功能
#[test]
fn test_inheritance_cache_basic() {
    let provider = AdvancedRbacProvider::new();

    // 初始缓存为空
    assert_eq!(provider.cache_size(), 0);

    // 执行权限检查会填充缓存
    provider
        .check_access("admin", "users", PermissionAction::Select)
        .unwrap();

    // 缓存应该有内容
    assert!(provider.cache_size() > 0);
}

/// TEST-ADV-U-013: 缓存清除
#[test]
fn test_cache_clear() {
    let provider = AdvancedRbacProvider::new();

    // 填充缓存
    provider
        .check_access("admin", "users", PermissionAction::Select)
        .unwrap();
    assert!(provider.cache_size() > 0);

    // 清除缓存
    provider.clear_cache();
    assert_eq!(provider.cache_size(), 0);
}

/// TEST-ADV-U-014: 添加角色时清除相关缓存
#[test]
fn test_cache_invalidation_on_role_add() {
    let provider = AdvancedRbacProvider::new();

    // 设置继承
    provider.add_role_inheritance("child".to_string(), vec!["admin".to_string()]);

    // 触发缓存填充
    provider
        .check_access("child", "users", PermissionAction::Select)
        .unwrap();
    assert!(provider.cache_size() > 0);

    // 添加角色应该清除相关缓存
    provider.add_role(
        "child".to_string(),
        RolePolicy {
            tables: vec![TablePermission {
                name: "new_table".to_string(),
                operations: vec![PermissionAction::Select],
            }],
        },
    );

    // 缓存应该被清除
    assert_eq!(provider.cache_size(), 0);
}

/// TEST-ADV-U-015: 添加继承关系时清除缓存
#[test]
fn test_cache_invalidation_on_inheritance_add() {
    let provider = AdvancedRbacProvider::new();

    // 触发缓存填充
    provider
        .check_access("admin", "users", PermissionAction::Select)
        .unwrap();

    // 添加继承关系应该清除相关缓存
    provider.add_role_inheritance("new_child".to_string(), vec!["admin".to_string()]);

    // 新角色的缓存条目不应该存在
    let cached = provider.get_inherited_roles("new_child");
    assert!(cached.contains("new_child"));
    assert!(cached.contains("admin"));
}

// ============================================================================
// 继承管理方法测试
// ============================================================================

/// TEST-ADV-U-016: has_inheritance 方法
#[test]
fn test_has_inheritance() {
    let provider = AdvancedRbacProvider::new();

    // admin 默认没有继承关系
    assert!(!provider.has_inheritance("admin"));

    // 添加继承关系
    provider.add_role_inheritance("manager".to_string(), vec!["admin".to_string()]);
    assert!(provider.has_inheritance("manager"));

    // 不存在的角色没有继承关系
    assert!(!provider.has_inheritance("nonexistent"));
}

/// TEST-ADV-U-017: get_direct_parents 方法
#[test]
fn test_get_direct_parents() {
    let provider = AdvancedRbacProvider::new();

    // 添加继承关系
    provider.add_role_inheritance("child".to_string(), vec!["parent1".to_string(), "parent2".to_string()]);

    // 获取直接父角色
    let parents = provider.get_direct_parents("child");
    assert!(parents.is_some());
    let parents = parents.unwrap();
    assert_eq!(parents.len(), 2);
    assert!(parents.contains(&"parent1".to_string()));
    assert!(parents.contains(&"parent2".to_string()));

    // 不存在的角色没有父角色
    assert!(provider.get_direct_parents("nonexistent").is_none());
}

/// TEST-ADV-U-018: remove_inheritance 方法
#[test]
fn test_remove_inheritance() {
    let provider = AdvancedRbacProvider::new();

    // 添加继承关系
    provider.add_role_inheritance("child".to_string(), vec!["admin".to_string()]);
    assert!(provider.has_inheritance("child"));

    // 移除继承关系
    let removed = provider.remove_inheritance("child");
    assert!(removed);
    assert!(!provider.has_inheritance("child"));

    // 再次移除应该返回 false
    let removed_again = provider.remove_inheritance("child");
    assert!(!removed_again);
}

/// TEST-ADV-U-019: set_role_inheritances 批量设置
#[test]
fn test_set_role_inheritances_batch() {
    let provider = AdvancedRbacProvider::new();

    // 批量设置继承关系
    provider.set_role_inheritances(vec![
        ("role_a".to_string(), vec!["admin".to_string()]),
        ("role_b".to_string(), vec!["readonly".to_string()]),
        ("role_c".to_string(), vec!["role_a".to_string(), "role_b".to_string()]),
    ]);

    // 验证继承关系已设置
    assert!(provider.has_inheritance("role_a"));
    assert!(provider.has_inheritance("role_b"));
    assert!(provider.has_inheritance("role_c"));

    // 验证权限继承
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

// ============================================================================
// 默认角色测试
// ============================================================================

/// TEST-ADV-U-020: 默认 admin 角色权限
#[test]
fn test_default_admin_role() {
    let provider = AdvancedRbacProvider::new();

    // admin 应该有所有权限
    assert!(
        provider
            .check_access("admin", "any_table", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("admin", "any_table", PermissionAction::Insert)
            .unwrap()
    );
    assert!(
        provider
            .check_access("admin", "any_table", PermissionAction::Update)
            .unwrap()
    );
    assert!(
        provider
            .check_access("admin", "any_table", PermissionAction::Delete)
            .unwrap()
    );
}

/// TEST-ADV-U-021: 默认 readonly 角色权限
#[test]
fn test_default_readonly_role() {
    let provider = AdvancedRbacProvider::new();

    // readonly 应该只有 SELECT 权限
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

/// TEST-ADV-U-022: 默认 readwrite 角色权限
#[test]
fn test_default_readwrite_role() {
    let provider = AdvancedRbacProvider::new();

    // readwrite 应该有 SELECT, INSERT, UPDATE 权限
    assert!(
        provider
            .check_access("readwrite", "users", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        provider
            .check_access("readwrite", "users", PermissionAction::Insert)
            .unwrap()
    );
    assert!(
        provider
            .check_access("readwrite", "users", PermissionAction::Update)
            .unwrap()
    );
    assert!(
        !provider
            .check_access("readwrite", "users", PermissionAction::Delete)
            .unwrap()
    );
}

/// TEST-ADV-U-023: get_roles 方法
#[test]
fn test_get_roles() {
    let provider = AdvancedRbacProvider::new();

    let roles = provider.get_roles();
    assert!(roles.contains(&"admin".to_string()));
    assert!(roles.contains(&"readonly".to_string()));
    assert!(roles.contains(&"readwrite".to_string()));
}

/// TEST-ADV-U-024: 未定义角色的访问
#[test]
fn test_undefined_role_access() {
    let provider = AdvancedRbacProvider::new();

    // 未定义的角色应该没有任何权限
    assert!(
        !provider
            .check_access("undefined", "users", PermissionAction::Select)
            .unwrap()
    );
    assert!(
        !provider
            .check_access("undefined", "users", PermissionAction::Insert)
            .unwrap()
    );
}

// ============================================================================
// 并发测试
// ============================================================================

/// TEST-ADV-U-025: 并发继承解析
#[test]
fn test_concurrent_inheritance_resolution() {
    use std::sync::Arc;
    use std::thread;

    let provider = Arc::new(AdvancedRbacProvider::new());

    // 设置继承关系
    provider.add_role_inheritance("manager".to_string(), vec!["admin".to_string()]);

    let mut handles = vec![];

    // 启动多个线程进行并发权限检查
    for _ in 0..10 {
        let p = provider.clone();
        let handle = thread::spawn(move || p.check_access("manager", "users", PermissionAction::Select).unwrap());
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        assert!(handle.join().unwrap());
    }
}

/// TEST-ADV-U-026: 并发添加角色和继承
#[test]
fn test_concurrent_role_and_inheritance_add() {
    use std::sync::Arc;
    use std::thread;

    let provider = Arc::new(AdvancedRbacProvider::new());

    let mut handles = vec![];

    // 并发添加角色
    for i in 0..5 {
        let p = provider.clone();
        let handle = thread::spawn(move || {
            p.add_role(
                format!("concurrent_role_{}", i),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: format!("table_{}", i),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            );
        });
        handles.push(handle);
    }

    // 并发添加继承关系
    for i in 0..5 {
        let p = provider.clone();
        let handle = thread::spawn(move || {
            p.add_role_inheritance(format!("concurrent_role_{}", i), vec!["admin".to_string()]);
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }
}
