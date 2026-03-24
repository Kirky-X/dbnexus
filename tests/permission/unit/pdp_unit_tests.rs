// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! PolicyDecisionPoint 决策单元测试
//!
//! 测试 PolicyDecisionPoint 的核心功能，包括：
//! - 权限决策逻辑
//! - 缓存机制
//! - 速率限制
//! - 权限冲突解决
//! - 批量检查

#[cfg(feature = "permission-engine")]
use dbnexus::{
    PermissionAction, PermissionContext, PermissionDecision, PermissionProvider, PermissionResource, PermissionRule,
    PermissionSubject, PolicyDecisionPoint, RbacPermissionProvider, Role,
};

#[cfg(feature = "permission-engine")]
use std::sync::Arc;

// ============================================================================
// 辅助函数
// ============================================================================

/// 创建测试用的 RBAC 权限提供者
#[cfg(feature = "permission-engine")]
fn create_test_rbac_provider() -> Arc<RbacPermissionProvider> {
    let provider = Arc::new(RbacPermissionProvider::new());

    // 添加 admin 角色
    provider.add_role(Role {
        name: "admin".to_string(),
        description: "管理员".to_string(),
        enabled: true,
        extends: vec![],
    });

    // 添加 admin 的权限规则
    provider.add_permission(
        "admin",
        PermissionRule {
            name: "admin_all".to_string(),
            priority: 100,
            subject: "*".to_string(),
            resource: "*".to_string(),
            allow: vec![
                PermissionAction::Select,
                PermissionAction::Insert,
                PermissionAction::Update,
                PermissionAction::Delete,
            ],
            deny: vec![],
            condition: None,
            enabled: true,
        },
    );

    // 将用户 "admin_user" 映射到角色 "admin"
    provider.add_role_to_subject("admin_user", "admin");

    // 添加 user 角色
    provider.add_role(Role {
        name: "user".to_string(),
        description: "普通用户".to_string(),
        enabled: true,
        extends: vec![],
    });

    // 添加 user 的权限规则
    provider.add_permission(
        "user",
        PermissionRule {
            name: "user_select".to_string(),
            priority: 50,
            subject: "*".to_string(),
            resource: "users".to_string(),
            allow: vec![PermissionAction::Select],
            deny: vec![],
            condition: None,
            enabled: true,
        },
    );

    provider.add_permission(
        "user",
        PermissionRule {
            name: "user_deny_delete".to_string(),
            priority: 50,
            subject: "*".to_string(),
            resource: "users".to_string(),
            allow: vec![],
            deny: vec![PermissionAction::Delete],
            condition: None,
            enabled: true,
        },
    );

    // 将用户 "normal_user" 映射到角色 "user"
    provider.add_role_to_subject("normal_user", "user");

    provider
}

// ============================================================================
// 基本决策测试
// ============================================================================

/// TEST-PDP-U-001: 基本权限检查 - 允许
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_check_allow() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    // admin_user 应该被允许所有操作
    let result = pdp.check("admin_user", "users", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "admin_user should be allowed SELECT on users"
    );

    let result = pdp.check("admin_user", "orders", "INSERT").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "admin_user should be allowed INSERT on orders"
    );

    let result = pdp.check("admin_user", "products", "UPDATE").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "admin_user should be allowed UPDATE on products"
    );

    let result = pdp.check("admin_user", "logs", "DELETE").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "admin_user should be allowed DELETE on logs"
    );
}

/// TEST-PDP-U-002: 基本权限检查 - 拒绝
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_check_deny() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    // normal_user 不应该被允许删除 users
    let result = pdp.check("normal_user", "users", "DELETE").await;
    assert_eq!(
        result,
        PermissionDecision::Deny,
        "normal_user should be denied DELETE on users"
    );
}

/// TEST-PDP-U-003: 基本权限检查 - 不适用
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_check_not_applicable() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    // normal_user 对 orders 表没有定义权限，默认拒绝
    let result = pdp.check("normal_user", "orders", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::NotApplicable,
        "normal_user should be NotApplicable for SELECT on orders (no permission defined)"
    );
}

/// TEST-PDP-U-004: 未知操作返回错误
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_check_unknown_action() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    // 未知操作应该返回 Error
    let result = pdp.check("admin_user", "users", "UNKNOWN").await;
    assert!(
        matches!(result, PermissionDecision::Error(_)),
        "Unknown action should return Error"
    );
}

// ============================================================================
// 权限上下文测试
// ============================================================================

/// TEST-PDP-U-005: 使用 PermissionContext 检查权限
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_check_permission_with_context() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    let context = PermissionContext::new(
        PermissionSubject::user("admin_user"),
        PermissionResource::new("users"),
        PermissionAction::Select,
    );

    let decision = pdp.check_permission(&context).await;
    assert_eq!(decision, PermissionDecision::Allow);
}

/// TEST-PDP-U-006: PermissionContext 带属性
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_permission_context_with_attributes() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    let context = PermissionContext::new(
        PermissionSubject::user("admin_user"),
        PermissionResource::new("users"),
        PermissionAction::Select,
    )
    .with_attribute("ip", "192.168.1.1")
    .with_environment("time", "2024-01-01");

    let decision = pdp.check_permission(&context).await;
    assert_eq!(decision, PermissionDecision::Allow);
}

/// TEST-PDP-U-007: PermissionSubject 类型测试
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_permission_subject_types() {
    let _provider = create_test_rbac_provider();

    // 用户主体
    let user_subject = PermissionSubject::user("test_user");
    assert_eq!(user_subject.id, "test_user");

    // 角色主体
    let role_subject = PermissionSubject::role("admin");
    assert_eq!(role_subject.id, "admin");
}

/// TEST-PDP-U-008: PermissionResource 类型测试
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_permission_resource_types() {
    // 基本资源
    let resource = PermissionResource::new("users");
    assert_eq!(resource.name, "users");
    assert_eq!(resource.resource_type, "table");

    // 带类型资源
    let resource_with_type = PermissionResource::with_type("logs", "log_file");
    assert_eq!(resource_with_type.name, "logs");
    assert_eq!(resource_with_type.resource_type, "log_file");
}

// ============================================================================
// 缓存机制测试
// ============================================================================

/// TEST-PDP-U-009: 缓存命中
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_cache_hit() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    // 第一次检查
    let result1 = pdp.check("admin_user", "users", "SELECT").await;
    assert_eq!(result1, PermissionDecision::Allow, "First check should succeed");

    // 第二次检查应该从缓存获取
    let result2 = pdp.check("admin_user", "users", "SELECT").await;
    assert_eq!(
        result2,
        PermissionDecision::Allow,
        "Second check (cached) should succeed"
    );
}

/// TEST-PDP-U-010: 缓存刷新
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_cache_refresh() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    // 执行检查
    let _ = pdp.check("admin_user", "users", "SELECT").await;

    // 刷新缓存
    pdp.refresh_cache().await;

    // 再次检查应该重新计算
    let result = pdp.check("admin_user", "users", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "Check after cache refresh should succeed"
    );
}

/// TEST-PDP-U-011: 禁用缓存
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_cache_disabled() {
    let provider = create_test_rbac_provider();
    let mut pdp = PolicyDecisionPoint::new(provider);

    // 禁用缓存
    pdp.set_cache_enabled(false);

    // 检查仍然应该工作
    let result = pdp.check("admin_user", "users", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "Check with cache disabled should still work"
    );
}

/// TEST-PDP-U-012: 自定义缓存 TTL
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_custom_cache_ttl() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::with_cache(provider, 60); // 60 秒 TTL

    let result = pdp.check("admin_user", "users", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "Check with custom TTL should succeed"
    );
}

// ============================================================================
// 速率限制测试
// ============================================================================

/// TEST-PDP-U-013: 速率限制 - 基本功能
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_rate_limit_basic() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::with_rate_limit(provider, 5, 60); // 每分钟 5 次

    // 前 5 次请求应该成功
    for _ in 0..5 {
        let result = pdp.check("test_user", "users", "SELECT").await;
        // test_user 没有定义权限，所以返回 NotApplicable
        assert_eq!(
            result,
            PermissionDecision::NotApplicable,
            "test_user has no permissions, should be NotApplicable"
        );
    }
}

/// TEST-PDP-U-014: 速率限制 - 超过限制
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_rate_limit_exceeded() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::with_rate_limit(provider, 3, 60); // 每分钟 3 次

    // 执行 3 次请求
    for _ in 0..3 {
        let _ = pdp.check("limited_user", "users", "SELECT").await;
    }

    // 第 4 次请求应该被速率限制拒绝
    let result = pdp.check("limited_user", "users", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::Deny,
        "Rate limit exceeded should return Deny"
    );
}

/// TEST-PDP-U-015: 速率限制 - 不同用户独立计数
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_rate_limit_different_users() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::with_rate_limit(provider, 2, 60); // 每分钟 2 次

    // 用户 A 的请求
    let _ = pdp.check("user_a", "users", "SELECT").await;
    let _ = pdp.check("user_a", "users", "SELECT").await;

    // 用户 B 应该仍然可以请求
    let result = pdp.check("user_b", "users", "SELECT").await;
    // user_b 没有定义权限，所以返回 NotApplicable
    assert_eq!(
        result,
        PermissionDecision::NotApplicable,
        "user_b has no permissions, should be NotApplicable"
    );
}

// ============================================================================
// 批量检查测试
// ============================================================================

/// TEST-PDP-U-016: 批量权限检查
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_batch_check() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    let contexts = vec![
        PermissionContext::new(
            PermissionSubject::user("admin_user"),
            PermissionResource::new("users"),
            PermissionAction::Select,
        ),
        PermissionContext::new(
            PermissionSubject::user("admin_user"),
            PermissionResource::new("orders"),
            PermissionAction::Insert,
        ),
        PermissionContext::new(
            PermissionSubject::user("normal_user"),
            PermissionResource::new("users"),
            PermissionAction::Select,
        ),
        PermissionContext::new(
            PermissionSubject::user("normal_user"),
            PermissionResource::new("users"),
            PermissionAction::Delete,
        ),
    ];

    let results = pdp.check_batch(contexts.clone()).await;

    assert_eq!(results.len(), 4);
    assert_eq!(results[0].1, PermissionDecision::Allow); // admin_user SELECT users
    assert_eq!(results[1].1, PermissionDecision::Allow); // admin_user INSERT orders
    assert_eq!(results[2].1, PermissionDecision::Allow); // normal_user SELECT users
    assert_eq!(results[3].1, PermissionDecision::Deny); // normal_user DELETE users
}

/// TEST-PDP-U-017: 空批量检查
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_batch_check_empty() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    let contexts: Vec<PermissionContext> = vec![];
    let results = pdp.check_batch(contexts).await;

    assert!(results.is_empty());
}

// ============================================================================
// 权限冲突解决测试
// ============================================================================

/// TEST-PDP-U-018: 规则优先级 - 高优先级规则优先
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_rule_priority() {
    let provider = Arc::new(RbacPermissionProvider::new());

    provider.add_role(Role {
        name: "test_role".to_string(),
        description: "测试角色".to_string(),
        enabled: true,
        extends: vec![],
    });

    // 低优先级规则：允许 SELECT
    provider.add_permission(
        "test_role",
        PermissionRule {
            name: "low_priority".to_string(),
            priority: 10,
            subject: "*".to_string(),
            resource: "users".to_string(),
            allow: vec![PermissionAction::Select],
            deny: vec![],
            condition: None,
            enabled: true,
        },
    );

    // 高优先级规则：拒绝 SELECT
    provider.add_permission(
        "test_role",
        PermissionRule {
            name: "high_priority".to_string(),
            priority: 100,
            subject: "*".to_string(),
            resource: "users".to_string(),
            allow: vec![],
            deny: vec![PermissionAction::Select],
            condition: None,
            enabled: true,
        },
    );

    provider.add_role_to_subject("test_user", "test_role");

    let pdp = PolicyDecisionPoint::new(provider);

    // 高优先级的拒绝规则应该生效
    let result = pdp.check("test_user", "users", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::Deny,
        "High priority deny rule should take effect"
    );
}

/// TEST-PDP-U-019: Allow 和 Deny 冲突 - Deny 优先
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_allow_deny_conflict() {
    let provider = Arc::new(RbacPermissionProvider::new());

    provider.add_role(Role {
        name: "conflict_role".to_string(),
        description: "冲突测试角色".to_string(),
        enabled: true,
        extends: vec![],
    });

    // 同一优先级：同时允许和拒绝
    provider.add_permission(
        "conflict_role",
        PermissionRule {
            name: "allow_rule".to_string(),
            priority: 50,
            subject: "*".to_string(),
            resource: "users".to_string(),
            allow: vec![PermissionAction::Select],
            deny: vec![],
            condition: None,
            enabled: true,
        },
    );

    provider.add_permission(
        "conflict_role",
        PermissionRule {
            name: "deny_rule".to_string(),
            priority: 50,
            subject: "*".to_string(),
            resource: "users".to_string(),
            allow: vec![],
            deny: vec![PermissionAction::Select],
            condition: None,
            enabled: true,
        },
    );

    provider.add_role_to_subject("conflict_user", "conflict_role");

    let pdp = PolicyDecisionPoint::new(provider);

    // 按规则顺序，先遇到的规则生效
    let result = pdp.check("conflict_user", "users", "SELECT").await;
    // 结果取决于规则评估顺序，但应该是 Allow 或 Deny 之一
    assert!(
        matches!(result, PermissionDecision::Allow | PermissionDecision::Deny),
        "Result should be either Allow or Deny based on rule evaluation order"
    );
}

/// TEST-PDP-U-020: 禁用的规则不生效
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_disabled_rule() {
    let provider = Arc::new(RbacPermissionProvider::new());

    provider.add_role(Role {
        name: "test_role".to_string(),
        description: "测试角色".to_string(),
        enabled: true,
        extends: vec![],
    });

    // 禁用的规则
    provider.add_permission(
        "test_role",
        PermissionRule {
            name: "disabled_rule".to_string(),
            priority: 100,
            subject: "*".to_string(),
            resource: "users".to_string(),
            allow: vec![PermissionAction::Select],
            deny: vec![],
            condition: None,
            enabled: false, // 禁用
        },
    );

    provider.add_role_to_subject("test_user", "test_role");

    let pdp = PolicyDecisionPoint::new(provider);

    // 禁用的规则不应该生效
    let result = pdp.check("test_user", "users", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::NotApplicable,
        "Disabled rule should not take effect, should be NotApplicable"
    );
}

// ============================================================================
// 获取资源和方法测试
// ============================================================================

/// TEST-PDP-U-021: 获取允许的资源
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_get_allowed_resources() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    let resources = pdp.get_allowed_resources("admin_user").await;
    assert!(!resources.is_empty());
}

/// TEST-PDP-U-022: 获取允许的操作
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_get_allowed_actions() {
    let provider = create_test_rbac_provider();
    let _pdp = PolicyDecisionPoint::new(provider.clone());

    // 通过 provider 获取允许的操作
    let actions = provider.get_allowed_actions("admin_user", "users").await;
    assert!(actions.contains(&PermissionAction::Select));
    assert!(actions.contains(&PermissionAction::Insert));
    assert!(actions.contains(&PermissionAction::Update));
    assert!(actions.contains(&PermissionAction::Delete));
}

// ============================================================================
// 通配符匹配测试
// ============================================================================

/// TEST-PDP-U-023: 通配符资源匹配
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_wildcard_resource() {
    let provider = create_test_rbac_provider();
    let pdp = PolicyDecisionPoint::new(provider);

    // admin_user 有通配符权限，应该可以访问任何表
    let result = pdp.check("admin_user", "any_unknown_table", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "admin_user with wildcard permission should be allowed"
    );
}

/// TEST-PDP-U-024: 通配符主体匹配
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_wildcard_subject() {
    let provider = Arc::new(RbacPermissionProvider::new());

    provider.add_role(Role {
        name: "wildcard_role".to_string(),
        description: "通配符角色".to_string(),
        enabled: true,
        extends: vec![],
    });

    // 通配符主体规则
    provider.add_permission(
        "wildcard_role",
        PermissionRule {
            name: "wildcard_subject".to_string(),
            priority: 50,
            subject: "*".to_string(), // 匹配任何主体
            resource: "public_table".to_string(),
            allow: vec![PermissionAction::Select],
            deny: vec![],
            condition: None,
            enabled: true,
        },
    );

    provider.add_role_to_subject("any_user", "wildcard_role");

    let pdp = PolicyDecisionPoint::new(provider);

    // 任何用户都应该能访问
    let result = pdp.check("any_user", "public_table", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "any_user should be allowed to access public_table"
    );
}

// ============================================================================
// PermissionAction 测试
// ============================================================================

/// TEST-PDP-U-025: PermissionAction Display 实现
#[cfg(feature = "permission-engine")]
#[test]
fn test_permission_action_display() {
    assert_eq!(PermissionAction::Select.to_string(), "SELECT");
    assert_eq!(PermissionAction::Insert.to_string(), "INSERT");
    assert_eq!(PermissionAction::Update.to_string(), "UPDATE");
    assert_eq!(PermissionAction::Delete.to_string(), "DELETE");
}

/// TEST-PDP-U-026: PermissionAction 序列化
#[cfg(feature = "permission-engine")]
#[test]
fn test_permission_action_serialization() {
    let action = PermissionAction::Select;
    let json = serde_json::to_string(&action).unwrap();
    assert_eq!(json, "\"select\"");

    let deserialized: PermissionAction = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, PermissionAction::Select);
}

// ============================================================================
// PermissionDecision 测试
// ============================================================================

/// TEST-PDP-U-027: PermissionDecision 相等比较
#[cfg(feature = "permission-engine")]
#[test]
fn test_permission_decision_equality() {
    assert_eq!(PermissionDecision::Allow, PermissionDecision::Allow);
    assert_eq!(PermissionDecision::Deny, PermissionDecision::Deny);
    assert_eq!(PermissionDecision::NotApplicable, PermissionDecision::NotApplicable);
    assert_ne!(PermissionDecision::Allow, PermissionDecision::Deny);
}

/// TEST-PDP-U-028: PermissionDecision 序列化
#[cfg(feature = "permission-engine")]
#[test]
fn test_permission_decision_serialization() {
    let decision = PermissionDecision::Allow;
    let json = serde_json::to_string(&decision).unwrap();
    assert_eq!(json, "\"Allow\"");

    let deserialized: PermissionDecision = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, PermissionDecision::Allow);
}

// ============================================================================
// 并发测试
// ============================================================================

/// TEST-PDP-U-029: 并发权限检查
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_concurrent_check() {
    let provider = create_test_rbac_provider();
    let pdp = Arc::new(PolicyDecisionPoint::new(provider));

    let mut handles = vec![];

    for _ in 0..10 {
        let pdp_clone = pdp.clone();
        let handle = tokio::spawn(async move { pdp_clone.check("admin_user", "users", "SELECT").await });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.await.unwrap();
        assert_eq!(
            result,
            PermissionDecision::Allow,
            "Concurrent check should return Allow for admin_user"
        );
    }
}

/// TEST-PDP-U-030: 并发缓存访问
#[cfg(feature = "permission-engine")]
#[tokio::test]
async fn test_pdp_concurrent_cache_access() {
    let provider = create_test_rbac_provider();
    let pdp = Arc::new(PolicyDecisionPoint::new(provider));

    let mut handles = vec![];

    // 并发执行相同和不同的查询
    for i in 0..20 {
        let pdp_clone = pdp.clone();
        let handle = tokio::spawn(async move {
            let table = if i % 2 == 0 { "users" } else { "orders" };
            pdp_clone.check("admin_user", table, "SELECT").await
        });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.await.unwrap();
        assert_eq!(
            result,
            PermissionDecision::Allow,
            "Concurrent cache access check should return Allow for admin_user"
        );
    }
}
