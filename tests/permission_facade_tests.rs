// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 权限统一门面测试
//!
//! 一处配置（RBAC + 字段脱敏 + RLS）经 PermissionFacade::apply 全局生效：
//! 出口脱敏、租户谓词注入、角色访问控制，以及二次 apply 的热换装。

#![cfg(all(
    feature = "permission-facade",
    feature = "sqlite",
    feature = "runtime-tokio-rustls"
))]

use dbnexus::{
    DbPool, PermissionFacade, PermissionFacadeConfig,
    access::data_protection::MaskStrategy,
    access::permission::{PermissionAction, PermissionConfig, RolePolicy, TablePermission},
};
use std::collections::HashMap;

/// admin 全权 + default 仅 orders Select 的 RBAC 配置
fn rbac_roles(default_tables: Vec<TablePermission>) -> HashMap<String, RolePolicy> {
    let mut roles = HashMap::new();
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
    roles.insert(
        "default".to_string(),
        RolePolicy {
            tables: default_tables,
        },
    );
    roles
}

async fn temp_pool(tag: &str) -> (DbPool, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("dbnexus_t421_{}_{}.db", tag, std::process::id()));
    let _ = std::fs::remove_file(&path);
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let pool = DbPool::new(&url).await.expect("pool");
    let admin = pool.get_session("admin").await.expect("admin");
    admin
        .execute_raw_ddl(
            "CREATE TABLE orders (id INTEGER PRIMARY KEY, email TEXT NOT NULL, \
tenant_id TEXT NOT NULL, amount REAL NOT NULL)",
        )
        .await
        .expect("create table");
    for (id, email, tenant, amount) in [
        (1, "alice@example.com", "t-100", "10.0"),
        (2, "bob@example.com", "t-200", "20.0"),
    ] {
        admin
            .execute_raw(&format!(
                "INSERT INTO orders (id, email, tenant_id, amount) VALUES ({id}, '{email}', '{tenant}', {amount})"
            ))
            .await
            .expect("insert");
    }
    (pool, path)
}

/// 一处配置全局生效：RBAC + 出口脱敏 + RLS 租户谓词
#[tokio::test]
async fn test_facade_single_config_global_effect() {
    let (pool, path) = temp_pool("global").await;

    let facade = PermissionFacade::new(&pool);
    facade
        .apply(
            PermissionFacadeConfig::new()
                .with_roles(rbac_roles(vec![TablePermission {
                    name: "orders".to_string(),
                    operations: vec![PermissionAction::Select],
                }]))
                .with_masking_rule("email", MaskStrategy::Hash)
                .with_rls_policy("orders", "tenant_id", "t-100"),
        )
        .await
        .expect("apply");

    // admin：不受 RLS 限制，但出口同样脱敏
    let admin_rows = pool
        .query_rows(
            "SELECT id, email, tenant_id FROM orders ORDER BY id",
            "admin",
        )
        .await
        .expect("admin query");
    assert_eq!(admin_rows.len(), 2, "admin 不受 RLS 限制");
    let email = admin_rows[0]["email"].as_str().unwrap();
    assert_eq!(email.len(), 64, "admin 出口脱敏（SHA-256 hex）");
    assert!(!email.contains('@'));

    // default 角色：RLS 谓词只放行 t-100 + 脱敏
    let default_rows = pool
        .query_rows("SELECT id, email, tenant_id FROM orders", "default")
        .await
        .expect("default query");
    assert_eq!(default_rows.len(), 1, "RLS 谓词应只放行 t-100");
    assert_eq!(default_rows[0]["tenant_id"], "t-100");
    let email = default_rows[0]["email"].as_str().unwrap();
    assert!(!email.contains('@'), "default 出口脱敏");

    let _ = std::fs::remove_file(&path);
}

/// 二次 apply 热换装：RLS 租户切换即时生效
#[tokio::test]
async fn test_facade_reapply_hot_swap() {
    let (pool, path) = temp_pool("swap").await;
    let facade = PermissionFacade::new(&pool);

    let base = || {
        PermissionFacadeConfig::new()
            .with_roles(rbac_roles(vec![TablePermission {
                name: "orders".to_string(),
                operations: vec![PermissionAction::Select],
            }]))
            .with_rls_policy("orders", "tenant_id", "t-100")
    };
    facade.apply(base()).await.expect("apply t-100");
    let rows = pool
        .query_rows("SELECT tenant_id FROM orders", "default")
        .await
        .expect("query t-100");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["tenant_id"], "t-100");

    // 热换装到 t-200
    facade
        .apply(PermissionFacadeConfig::new().with_rls_policy("orders", "tenant_id", "t-200"))
        .await
        .expect("apply t-200");
    let rows = pool
        .query_rows("SELECT tenant_id FROM orders", "default")
        .await
        .expect("query t-200");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["tenant_id"], "t-200", "RLS 租户应切换为 t-200");

    let _ = std::fs::remove_file(&path);
}

/// RBAC 收口：default 无 SELECT 权限的表被拒绝
#[tokio::test]
async fn test_facade_rbac_denies_unauthorized_table() {
    let (pool, path) = temp_pool("deny").await;
    let facade = PermissionFacade::new(&pool);

    // default 仅授予 audit 表（不存在）的权限 → orders 查询被拒
    facade
        .apply(
            PermissionFacadeConfig::new().with_roles(rbac_roles(vec![TablePermission {
                name: "audit".to_string(),
                operations: vec![PermissionAction::Select],
            }])),
        )
        .await
        .expect("apply");

    let err = pool
        .query_rows("SELECT id FROM orders", "default")
        .await
        .unwrap_err();
    assert!(
        format!("{err}").to_lowercase().contains("denied") || format!("{err}").contains("权限"),
        "default 对 orders 应被拒绝，实际: {err}"
    );

    let _ = std::fs::remove_file(&path);
}
