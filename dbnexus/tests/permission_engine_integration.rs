// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限引擎集成测试
//!
//! 测试可插拔权限引擎的完整功能，包括：
//! - YAML 权限提供者
//! - RBAC 权限提供者
//! - PolicyDecisionPoint 策略决策
//! - 权限缓存和刷新

use dbnexus::permission_engine::{
    PermissionAction, PermissionContext, PermissionDecision, PermissionProvider, PermissionResource, PermissionRule,
    PermissionSubject as Subject, PolicyDecisionPoint, RbacPermissionProvider, Role, YamlPermissionProvider,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
mod common;

/// 临时 YAML 权限配置文件内容
const TEST_PERMISSIONS_YAML: &str = r#"
# 测试权限配置
roles:
  admin:
    - name: "admin_all"
      priority: 0
      subject: "admin"
      resource: "*"
      allow: ["select", "insert", "update", "delete"]
      deny: []
      enabled: true
  
  manager:
    - name: "manager_users"
      priority: 0
      subject: "manager"
      resource: "users"
      allow: ["select", "insert", "update"]
      deny: []
      enabled: true
    - name: "manager_orders"
      priority: 0
      subject: "manager"
      resource: "orders"
      allow: ["select", "insert", "update", "delete"]
      deny: []
      enabled: true
  
  user:
    - name: "user_select_users"
      priority: 0
      subject: "user"
      resource: "users"
      allow: ["select"]
      deny: ["delete"]
      enabled: true
    - name: "user_select_products"
      priority: 0
      subject: "user"
      resource: "products"
      allow: ["select"]
      deny: []
      enabled: true
  
  guest:
    - name: "guest_select_products"
      priority: 0
      subject: "guest"
      resource: "products"
      allow: ["select"]
      deny: []
      enabled: true
"#;

/// TEST-PE-001: YAML 权限提供者创建测试
#[tokio::test]
async fn test_yaml_permission_provider_creation() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = YamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
        .expect("Failed to create YAML provider");

    assert_eq!(provider.name(), "yaml");
}

/// TEST-PE-002: YAML 权限检查测试
#[tokio::test]
async fn test_yaml_permission_check() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        YamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );

    // 加载配置
    provider.refresh().await.expect("Failed to refresh provider");

    // admin 可以访问所有操作
    let ctx = PermissionContext::new(
        Subject::role("admin"),
        PermissionResource::new("users"),
        PermissionAction::Select,
    );
    let decision = provider.check_permission(&ctx).await;
    assert_eq!(decision, PermissionDecision::Allow);

    let ctx = PermissionContext::new(
        Subject::role("admin"),
        PermissionResource::new("orders"),
        PermissionAction::Delete,
    );
    let decision = provider.check_permission(&ctx).await;
    assert_eq!(decision, PermissionDecision::Allow);

    // user 不能 DELETE users
    let ctx = PermissionContext::new(
        Subject::role("user"),
        PermissionResource::new("users"),
        PermissionAction::Delete,
    );
    let decision = provider.check_permission(&ctx).await;
    assert_eq!(decision, PermissionDecision::Deny);

    // user 可以 SELECT users
    let ctx = PermissionContext::new(
        Subject::role("user"),
        PermissionResource::new("users"),
        PermissionAction::Select,
    );
    let decision = provider.check_permission(&ctx).await;
    assert_eq!(decision, PermissionDecision::Allow);

    // guest 不能访问 users
    let ctx = PermissionContext::new(
        Subject::role("guest"),
        PermissionResource::new("users"),
        PermissionAction::Select,
    );
    let decision = provider.check_permission(&ctx).await;
    assert_eq!(decision, PermissionDecision::NotApplicable);
}

/// TEST-PE-003: RBAC 权限提供者测试
#[tokio::test]
async fn test_rbac_permission_provider() {
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

    // 添加 admin 的权限规则
    provider.add_permission(
        "admin",
        PermissionRule {
            name: "admin_all".to_string(),
            priority: 0,
            subject: "admin".to_string(),
            resource: "*".to_string(),
            allow: vec![PermissionAction::All],
            deny: vec![],
            condition: None,
            enabled: true,
        },
    );

    // admin 应该拥有所有权限
    let ctx = PermissionContext::new(
        Subject::role("admin"),
        PermissionResource::new("any_table"),
        PermissionAction::All,
    );
    let decision = provider.check_permission(&ctx).await;
    assert_eq!(decision, PermissionDecision::Allow);
}

/// TEST-PE-004: PolicyDecisionPoint 测试
#[tokio::test]
async fn test_policy_decision_point() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        YamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );

    // 加载配置
    provider.refresh().await.expect("Failed to refresh provider");

    let pdp = PolicyDecisionPoint::new(provider);

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
async fn test_permission_provider_refresh() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        YamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );

    // 刷新权限缓存
    let result = provider.refresh().await;
    assert!(result.is_ok(), "Refresh should succeed");
}

/// TEST-PE-006: 获取允许的资源列表测试
#[tokio::test]
async fn test_get_allowed_resources() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        YamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );

    // 加载配置
    provider.refresh().await.expect("Failed to refresh provider");

    // admin 可以访问所有资源
    let resources = provider.get_allowed_resources("admin").await;
    assert!(!resources.is_empty());

    // user 只能访问特定资源
    let resources = provider.get_allowed_resources("user").await;
    assert!(resources.iter().any(|r| r.name == "users"));
    assert!(!resources.iter().any(|r| r.name == "orders"));
}

/// TEST-PE-007: 获取允许的操作列表测试
#[tokio::test]
async fn test_get_allowed_actions() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        YamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );

    // 加载配置
    provider.refresh().await.expect("Failed to refresh provider");

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
async fn test_wildcard_resource_matching() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        YamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );

    // 加载配置
    provider.refresh().await.expect("Failed to refresh provider");

    // admin 可以访问任意表
    let ctx = PermissionContext::new(
        Subject::role("admin"),
        PermissionResource::new("any_unknown_table"),
        PermissionAction::Select,
    );
    let decision = provider.check_permission(&ctx).await;
    assert_eq!(decision, PermissionDecision::Allow);
}

/// TEST-PE-009: 多提供者优先级测试
#[tokio::test]
async fn test_multiple_providers_priority() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let yaml_provider = Arc::new(
        YamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
            .expect("Failed to create YAML provider"),
    );

    // 加载配置
    yaml_provider.refresh().await.expect("Failed to refresh provider");

    let pdp = PolicyDecisionPoint::new(yaml_provider);

    // 测试决策点
    let result = pdp.check("manager", "users", "UPDATE").await;
    assert_eq!(result, PermissionDecision::Allow);

    let result = pdp.check("manager", "orders", "DELETE").await;
    assert_eq!(result, PermissionDecision::Allow);
}

/// TEST-PE-010: 权限决策延迟测试
#[tokio::test]
async fn test_permission_decision_latency() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    std::fs::write(&perm_file, TEST_PERMISSIONS_YAML).expect("Failed to write test permissions");

    let provider = Arc::new(
        YamlPermissionProvider::new(perm_file.to_str().unwrap_or("permissions.yaml"))
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
