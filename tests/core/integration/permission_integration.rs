// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 权限控制集成测试
//!
//! 配置解析通过 confers 库

use dbnexus::DbPool;
use dbnexus::access::permission::{PermissionAction as Operation, PermissionConfig, RolePolicy, TablePermission};
use dbnexus::foundation::config::ConfigError;

#[path = "../../common/mod.rs"]
mod common;

/// 使用 serde_json 直接解析 JSON 配置（测试用）
#[cfg(feature = "confers")]
fn parse_json_config(json: &str) -> Result<PermissionConfig, ConfigError> {
    serde_json::from_str(json)
        .map_err(|e| ConfigError::InvalidFormat(format!("JSON deserialize error: {}", e)))
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
#[allow(clippy::unwrap_used)]
async fn test_permission_context_role() {
    let (config, _temp_dir) = common::get_test_config_with_permissions(true);
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    // 使用配置中定义的 admin 角色
    let session = pool.get_session("admin").await.expect("Failed to get session");
    // 加载权限策略到缓存
    let perm_path = pool
        .config()
        .permissions_path
        .clone()
        .expect("Missing permissions path");
    let perm_content = r#"
{
  "roles": {
    "admin": {
      "tables": [
        {
          "name": "*",
          "operations": ["select", "insert", "update", "delete"]
        }
      ]
    }
  }
}
"#;
    std::fs::write(&perm_path, perm_content).expect("Failed to write permissions file");
    let perm_config = parse_json_config(&std::fs::read_to_string(&perm_path).unwrap()).expect("Failed to parse permission JSON");
    session
        .permission_ctx()
        .load_policy(&perm_config)
        .await
        .expect("Failed to load policy");
    let ctx = session.permission_ctx();
    assert_eq!(ctx.role(), "admin");
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
#[allow(clippy::unwrap_used)]
async fn test_permission_check() {
    let (config, _temp_dir) = common::get_test_config_with_permissions(true);
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    // admin 角色有所有权限
    let session = pool.get_session("admin").await.expect("Failed to get session");
    // 加载权限策略到缓存
    let perm_path = pool
        .config()
        .permissions_path
        .as_ref()
        .expect("Missing permissions path")
        .clone();
    let perm_content = r#"
{
  "roles": {
    "admin": {
      "tables": [
        {
          "name": "*",
          "operations": ["select", "insert", "update", "delete"]
        }
      ]
    }
  }
}
"#;
    std::fs::write(&perm_path, perm_content).expect("Failed to write permissions file");
    let perm_config = parse_json_config(&std::fs::read_to_string(&perm_path).unwrap()).expect("Failed to parse permission JSON");
    session
        .permission_ctx()
        .load_policy(&perm_config)
        .await
        .expect("Failed to load policy");
    let result = session.check_permission("unknown_table", &Operation::Select).await;
    // admin 可以访问所有表，所以应该成功
    assert!(result.is_ok(), "admin should have SELECT permission on any table");
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

#[cfg(feature = "confers")]
#[test]
fn test_permission_config_from_yaml() {
    let json = r#"
{
  "roles": {
    "admin": {
      "tables": [
        {
          "name": "*",
          "operations": ["select", "insert", "update", "delete"]
        }
      ]
    },
    "user": {
      "tables": [
        {
          "name": "users",
          "operations": ["select"]
        }
      ]
    }
  }
}
"#;

    let config = parse_json_config(json).expect("Should parse JSON");
    assert!(config.roles.contains_key("admin"));
    assert!(config.roles.contains_key("user"));
    assert!(config.check_access("admin", "any_table", Operation::Select));
}

#[tokio::test]
async fn test_permission_cache_miss_behavior() {
    use dbnexus::access::permission::PermissionContext;

    // 创建权限上下文（不预加载策略）
    let ctx = PermissionContext::with_cache_size("test_role".to_string(), 256)
        .await
        .expect("Failed to create permission context");

    // 缓存未命中应该返回 false（拒绝访问）
    let result = ctx.check_table_access("users", &Operation::Select).await;
    assert!(!result, "Cache miss should deny access by default");
}

#[tokio::test]
async fn test_permission_check_with_auto_load() {
    use dbnexus::access::permission::PermissionContext;

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
    let ctx: PermissionContext = PermissionContext::with_cache_size("test_role".to_string(), 256)
        .await
        .expect("Failed to create permission context");

    // 手动加载策略到缓存
    ctx.load_policy(&config).await.expect("Failed to load policy");

    // 现在应该有权访问
    let result = ctx.check_table_access("users", &Operation::Select).await;
    assert!(result, "Should have permission after loading policy");

    // 再次检查应该从缓存读取
    let result2 = ctx.check_table_access("users", &Operation::Select).await;
    assert!(result2, "Should have permission from cache");
}
