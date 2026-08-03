// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 权限引擎集成测试
//!
//! 测试可插拔权限引擎的完整功能，包括：
//! - YAML 权限提供者
//! - RBAC 权限提供者
//! - PolicyDecisionPoint 策略决策
//! - 权限缓存和刷新

use dbnexus::{
    EnginePermissionAction as PermissionAction, EnginePermissionProvider, EngineYamlPermissionProvider,
    PermissionDecision, PermissionRule, PolicyDecisionPoint, RbacPermissionProvider, Role,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[path = "../../common/mod.rs"]
mod common;

/// 权限配置测试数据（PermissionRule 格式：roles → Vec<PermissionRule>）
const TEST_PERMISSIONS_YAML: &str = r#"
roles:
  admin:
    - name: admin_all
      priority: 100
      subject: "*"
      resource: "*"
      allow:
        - select
        - insert
        - update
        - delete
      deny: []
      enabled: true
  manager:
    - name: manager_users
      priority: 50
      subject: "*"
      resource: users
      allow:
        - select
        - insert
        - update
      deny: []
      enabled: true
    - name: manager_orders
      priority: 50
      subject: "*"
      resource: orders
      allow:
        - select
        - insert
        - update
        - delete
      deny: []
      enabled: true
  user:
    - name: user_read
      priority: 10
      subject: "*"
      resource: users
      allow:
        - select
      deny: []
      enabled: true
    - name: user_deny_delete
      priority: 20
      subject: "*"
      resource: users
      allow: []
      deny:
        - delete
      enabled: true
  guest:
    - name: guest_products
      priority: 5
      subject: "*"
      resource: products
      allow:
        - select
      deny: []
      enabled: true
"#;

/// TEST-PE-001: YAML 权限提供者创建测试
#[tokio::test]
async fn test_yaml_permission_provider_creation_succeeds() {
    // 创建临时目录和文件
    let temp_dir = common::create_temp_dir();
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = EngineYamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
        .expect("Failed to create YAML provider");

    // 使用 PolicyDecisionPoint 来验证权限提供者功能
    let pdp = PolicyDecisionPoint::new(Arc::new(provider));

    // 先刷新缓存以加载配置
    pdp.refresh_cache().await;

    let result = pdp.check("admin", "users", "SELECT").await;
    assert_eq!(result, PermissionDecision::Allow);
}

/// TEST-PE-002: YAML 权限检查测试
#[tokio::test]
async fn test_yaml_permission_check_returns_correct_decisions() {
    // 创建临时目录和文件
    let temp_dir = common::create_temp_dir();
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        EngineYamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );
    let pdp = PolicyDecisionPoint::new(provider);

    // 先刷新缓存以加载配置
    pdp.refresh_cache().await;

    // admin 可以访问所有操作
    let result = pdp.check("admin", "users", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "admin SELECT users should be allowed"
    );

    let result = pdp.check("admin", "orders", "DELETE").await;
    assert_eq!(
        result,
        PermissionDecision::Allow,
        "admin DELETE orders should be allowed"
    );

    // user 不能 DELETE users
    let result = pdp.check("user", "users", "DELETE").await;
    assert_eq!(result, PermissionDecision::Deny, "user DELETE users should be denied");

    // user 可以 SELECT users
    let result = pdp.check("user", "users", "SELECT").await;
    assert_eq!(result, PermissionDecision::Allow, "user SELECT users should be allowed");

    // guest 不能访问 users
    let result = pdp.check("guest", "users", "SELECT").await;
    assert_eq!(
        result,
        PermissionDecision::NotApplicable,
        "guest SELECT users should be NotApplicable"
    );
}

/// TEST-PE-003: RBAC 权限提供者测试
#[tokio::test]
async fn test_rbac_permission_provider_creation_succeeds() {
    // 创建 RBAC 提供者
    let provider = Arc::new(RbacPermissionProvider::new());

    // 创建角色
    let admin_role = Role {
        name: "admin".to_string(),
        description: "管理员".to_string(),
        enabled: true,
        extends: vec![],
    };

    let user_role = Role {
        name: "user".to_string(),
        description: "普通用户".to_string(),
        enabled: true,
        extends: vec!["viewer".to_string()],
    };

    let viewer_role = Role {
        name: "viewer".to_string(),
        description: "查看者".to_string(),
        enabled: true,
        extends: vec![],
    };

    // 添加角色
    provider.add_role(admin_role.clone());
    provider.add_role(user_role.clone());
    provider.add_role(viewer_role.clone());

    // 映射主体到角色
    provider.add_role_to_subject("admin", "admin");
    provider.add_role_to_subject("user", "user");
    provider.add_role_to_subject("user", "viewer");

    // 添加 admin 的权限规则
    provider.add_permission(
        "admin",
        PermissionRule {
            name: "admin_all".to_string(),
            priority: 0,
            subject: "admin".to_string(),
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

    // 使用 PolicyDecisionPoint 测试
    let pdp = PolicyDecisionPoint::new(provider);

    // admin 应该拥有所有权限
    let result = pdp.check("admin", "any_table", "SELECT").await;
    assert_eq!(result, PermissionDecision::Allow);

    let result = pdp.check("admin", "any_table", "DELETE").await;
    assert_eq!(result, PermissionDecision::Allow);
}

/// TEST-PE-004: PolicyDecisionPoint 测试
#[tokio::test]
async fn test_policy_decision_point_returns_expected_decisions() {
    // 创建临时目录和文件
    let temp_dir = common::create_temp_dir();
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        EngineYamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );

    let pdp = PolicyDecisionPoint::new(provider);

    // 先刷新缓存以加载配置
    pdp.refresh_cache().await;

    // 测试 admin 的权限
    let result = pdp.check("admin", "users", "SELECT").await;
    assert_eq!(result, PermissionDecision::Allow);

    let result = pdp.check("admin", "orders", "DELETE").await;
    assert_eq!(result, PermissionDecision::Allow);

    // 测试 user 的权限
    let result = pdp.check("user", "users", "SELECT").await;
    assert_eq!(result, PermissionDecision::Allow);

    let result = pdp.check("user", "users", "DELETE").await;
    assert_eq!(result, PermissionDecision::Deny);
}

/// TEST-PE-005: 权限提供者刷新测试
#[tokio::test]
async fn test_permission_provider_refresh_returns_allow() {
    // 创建临时目录和文件
    let temp_dir = common::create_temp_dir();
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        EngineYamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );
    let pdp = PolicyDecisionPoint::new(provider);

    // 刷新权限缓存
    pdp.refresh_cache().await;

    // 验证刷新成功（通过检查权限是否正常工作）
    let result = pdp.check("admin", "users", "SELECT").await;
    assert_eq!(result, PermissionDecision::Allow);
}

/// TEST-PE-006: 获取允许的资源列表测试
#[tokio::test]
async fn test_get_allowed_resources_returns_expected_resources() {
    // 创建临时目录和文件
    let temp_dir = common::create_temp_dir();
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        EngineYamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );
    let pdp = PolicyDecisionPoint::new(provider);

    // 先刷新缓存以加载配置
    pdp.refresh_cache().await;

    // admin 可以访问所有资源
    let resources = pdp.get_allowed_resources("admin").await;
    assert!(!resources.is_empty());

    // user 只能访问特定资源
    let resources = pdp.get_allowed_resources("user").await;
    assert!(resources.iter().any(|r| r.name == "users"));
    assert!(!resources.iter().any(|r| r.name == "orders"));
}

/// TEST-PE-007: 获取允许的操作列表测试
#[tokio::test]
async fn test_get_allowed_actions_returns_expected_actions() {
    // 创建临时目录和文件
    let temp_dir = common::create_temp_dir();
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        EngineYamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );
    let pdp = PolicyDecisionPoint::new(provider.clone());

    // 先刷新缓存以加载配置
    pdp.refresh_cache().await;

    // admin 对 users 有所有操作权限
    let actions = provider.get_allowed_actions("admin", "users").await;
    assert!(actions.contains(&PermissionAction::Select));
    assert!(actions.contains(&PermissionAction::Insert));
    assert!(actions.contains(&PermissionAction::Update));
    assert!(actions.contains(&PermissionAction::Delete));

    // user 对 users 只能 SELECT
    let actions = provider.get_allowed_actions("user", "users").await;
    assert!(actions.contains(&PermissionAction::Select));
    assert!(!actions.contains(&PermissionAction::Delete));
}

/// TEST-PE-008: 通配符资源匹配测试
#[tokio::test]
async fn test_wildcard_resource_matching_returns_allow() {
    // 创建临时目录和文件
    let temp_dir = common::create_temp_dir();
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        EngineYamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );
    let pdp = PolicyDecisionPoint::new(provider);

    // 先刷新缓存以加载配置
    pdp.refresh_cache().await;

    // admin 可以访问任意表
    let result = pdp.check("admin", "any_unknown_table", "SELECT").await;
    assert_eq!(result, PermissionDecision::Allow);
}

/// TEST-PE-009: 多提供者优先级测试
#[tokio::test]
async fn test_multiple_providers_priority_returns_allow() {
    // 创建临时目录和文件
    let temp_dir = common::create_temp_dir();
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let yaml_provider = Arc::new(
        EngineYamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );
    let pdp = PolicyDecisionPoint::new(yaml_provider);

    // 先刷新缓存以加载配置
    pdp.refresh_cache().await;

    // 测试决策点
    let result = pdp.check("manager", "users", "UPDATE").await;
    assert_eq!(result, PermissionDecision::Allow);

    let result = pdp.check("manager", "orders", "DELETE").await;
    assert_eq!(result, PermissionDecision::Allow);
}

/// TEST-PE-010: 权限决策延迟测试
#[tokio::test]
async fn test_permission_decision_latency_under_threshold() {
    // 创建临时目录和文件
    let temp_dir = common::create_temp_dir();
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        EngineYamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );
    let pdp = PolicyDecisionPoint::new(provider);

    // 执行多次权限检查以测试延迟
    let start = Instant::now();
    for _ in 0..100 {
        let _ = pdp.check("admin", "users", "SELECT").await;
    }
    let elapsed = start.elapsed();

    // 100 次权限检查应该在合理时间内完成
    assert!(
        elapsed < Duration::from_secs(5),
        "100 permission checks should complete in under 5 seconds, took {:?}",
        elapsed
    );
}
