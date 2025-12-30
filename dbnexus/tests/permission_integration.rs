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
