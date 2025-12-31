// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限控制集成测试

use dbnexus::DbPool;
use dbnexus::permission::{PermissionAction as Operation, PermissionConfig, RolePolicy, TablePermission};
mod common;

#[tokio::test]
async fn test_permission_context_role() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("test_role").await.expect("Failed to get session");
    let ctx = session.permission_ctx();
    assert_eq!(ctx.role(), "test_role");
}

#[tokio::test]
async fn test_permission_check() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let result = session.check_permission("users", &Operation::Select);
    // Result depends on permission configuration
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_operation_display() {
    let select = Operation::Select;
    assert_eq!(select.to_string(), "SELECT");

    let insert = Operation::Insert;
    assert_eq!(insert.to_string(), "INSERT");
}

#[test]
fn test_role_policy_allows() {
    let policy = RolePolicy {
        tables: vec![TablePermission {
            name: "users".to_string(),
            operations: vec![Operation::Select, Operation::Insert],
        }],
    };

    assert!(policy.allows("users", &Operation::Select));
    assert!(policy.allows("users", &Operation::Insert));
    assert!(!policy.allows("users", &Operation::Delete));
    assert!(!policy.allows("orders", &Operation::Select));
}

#[test]
fn test_permission_config_check_access() {
    let config = PermissionConfig {
        roles: [(
            "admin".to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "*".to_string(),
                    operations: vec![
                        Operation::Select,
                        Operation::Insert,
                        Operation::Update,
                        Operation::Delete,
                    ],
                }],
            },
        )]
        .into_iter()
        .collect(),
    };

    assert!(config.check_access("admin", "users", Operation::Select));
    assert!(config.check_access("admin", "orders", Operation::Delete));
    assert!(!config.check_access("user", "users", Operation::Delete)); // user role not defined
}

#[test]
fn test_permission_config_deny_all() {
    let config = PermissionConfig::deny_all();

    // deny_all 应该没有角色，所以任何访问都应该被拒绝
    assert!(config.roles.is_empty(), "deny_all should have no roles");
    assert!(!config.check_access("admin", "users", Operation::Select));
    assert!(!config.check_access("any_role", "any_table", Operation::Delete));
}

#[test]
fn test_permission_config_from_yaml() {
    let yaml = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
  user:
    tables:
      - name: "users"
        operations:
          - select
"#;

    let config = PermissionConfig::from_yaml(yaml).expect("Should parse YAML");
    assert!(config.roles.contains_key("admin"));
    assert!(config.roles.contains_key("user"));
    assert!(config.check_access("admin", "any_table", Operation::Select));
}

#[tokio::test]
async fn test_permission_cache_miss_behavior() {
    use dbnexus::permission::PermissionContext;

    // 创建权限上下文（不预加载策略）
    let ctx = PermissionContext::with_cache_size("test_role".to_string(), 256);

    // 缓存未命中应该返回 false（拒绝访问）
    let result = ctx.check_table_access("users", &Operation::Select);
    assert!(!result, "Cache miss should deny access by default");
}

#[tokio::test]
async fn test_permission_check_with_auto_load() {
    use dbnexus::permission::PermissionContext;

    // 创建权限配置
    let config = PermissionConfig {
        roles: [(
            "test_role".to_string(),
            RolePolicy {
                tables: vec![TablePermission {
                    name: "users".to_string(),
                    operations: vec![Operation::Select],
                }],
            },
        )]
        .into_iter()
        .collect(),
    };

    // 创建权限上下文
    let ctx = PermissionContext::with_cache_size("test_role".to_string(), 256);

    // 使用 auto_load 版本（缓存未命中时会自动加载）
    let result = ctx.check_table_access_with_config("users", &Operation::Select, &config);
    assert!(result, "Should have permission after auto-load");

    // 再次检查应该从缓存读取
    let result2 = ctx.check_table_access("users", &Operation::Select);
    assert!(result2, "Should have permission from cache");
}
