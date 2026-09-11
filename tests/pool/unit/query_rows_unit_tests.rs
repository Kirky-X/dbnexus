// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! T401：统一行查询 API（query_rows）与 scatter-gather 行查询测试
//!
//! 需要 sqlite + sql-parser + runtime-tokio-rustls feature。

#![cfg(all(
    feature = "runtime-tokio-rustls",
    feature = "sqlite",
    feature = "sql-parser"
))]

use std::sync::Arc;
use std::time::Duration;

use dbnexus::{AggregateFunction, PartialFailurePolicy, ScatterGatherExecutor, ShardRouter};

#[tokio::test]
async fn test_query_rows_returns_data_rows() {
    // sqlite::memory: 的每个池化连接是独立内存库，跨连接建表不可见——用临时文件库
    let db_path = std::env::temp_dir().join(format!("dbnexus_t401_{}.db", std::process::id()));
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = dbnexus::DbPool::new(&url).await.unwrap();

    // admin 会话执行建表与写入
    let admin = pool.get_session("admin").await.unwrap();
    admin
        .execute_raw_ddl("CREATE TABLE t_qr (id INTEGER PRIMARY KEY, val REAL NOT NULL)")
        .await
        .unwrap();
    admin
        .execute_raw("INSERT INTO t_qr (id, val) VALUES (1, 10.0)")
        .await
        .unwrap();
    admin
        .execute_raw("INSERT INTO t_qr (id, val) VALUES (2, 20.0)")
        .await
        .unwrap();

    // 统一行查询：返回真实数据行
    let rows = pool.query_rows("SELECT id, val FROM t_qr ORDER BY id", "admin")
        .await
        .unwrap();
    assert_eq!(rows.len(), 2, "应返回 2 行数据");
    assert_eq!(rows[0]["id"], 1);
    assert_eq!(rows[0]["val"], 10.0);
    assert_eq!(rows[1]["val"], 20.0);

    // 非 SELECT 一律拒绝
    let err = pool
        .query_rows("DELETE FROM t_qr", "admin")
        .await
        .unwrap_err();
    assert!(
        format!("{err}").contains("SELECT"),
        "query_rows 非 SELECT 应被拒绝，实际: {err}"
    );
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_scatter_query_rows_with_aggregate() {
    // 两个独立临时文件库作为分片（sqlite::memory: 每连接独立，跨连接建表不可见）
    let tmp0 = std::env::temp_dir().join(format!("dbnexus_t401_s0_{}.db", std::process::id()));
    let tmp1 = std::env::temp_dir().join(format!("dbnexus_t401_s1_{}.db", std::process::id()));
    let url0 = format!("sqlite:{}?mode=rwc", tmp0.display());
    let url1 = format!("sqlite:{}?mode=rwc", tmp1.display());
    let mut router = ShardRouter::with_strategy("hash", 2);
    router.register_shard(0, "shard_0".to_string(), url0.clone());
    let pool0 = Arc::new(dbnexus::DbPool::new(&url0).await.unwrap());
    let admin0 = pool0.get_session("admin").await.unwrap();
    admin0
        .execute_raw_ddl("CREATE TABLE t_s (val REAL NOT NULL)")
        .await
        .unwrap();
    admin0
        .execute_raw("INSERT INTO t_s (val) VALUES (1.0), (2.0)")
        .await
        .unwrap();
    router.set_pool(0, pool0).unwrap();

    router.register_shard(1, "shard_1".to_string(), url1.clone());
    let pool1 = Arc::new(dbnexus::DbPool::new(&url1).await.unwrap());
    let admin1 = pool1.get_session("admin").await.unwrap();
    admin1
        .execute_raw_ddl("CREATE TABLE t_s (val REAL NOT NULL)")
        .await
        .unwrap();
    admin1
        .execute_raw("INSERT INTO t_s (val) VALUES (3.0)")
        .await
        .unwrap();
    router.set_pool(1, pool1).unwrap();

    let executor = ScatterGatherExecutor::new(
        Arc::new(router),
        Duration::from_secs(5),
        PartialFailurePolicy::BestEffort,
    );

    // 行查询 + SUM 跨分片聚合
    let result = executor
        .scatter_query_rows(
            "SELECT val FROM t_s",
            "admin",
            Some(&AggregateFunction::Sum("val".to_string())),
        )
        .await
        .unwrap();

    assert_eq!(result.shard_row_counts.len(), 2, "两分片都应返回行数");
    let total_rows: usize = result.shard_rows.iter().map(|(_, r)| r.len()).sum();
    assert_eq!(total_rows, 3, "跨分片应取回 3 行数据");
    assert!(
        result.shard_rows.iter().all(|(_, r)| !r.is_empty()),
        "数据行应真实存在而非空"
    );

    match result.aggregated {
        Some(dbnexus::AggregateValue::Sum(v)) => assert!((v - 6.0).abs() < 1e-9),
        other => panic!("expected Sum(6.0), got {:?}", other),
    }
    let _ = std::fs::remove_file(&tmp0);
    let _ = std::fs::remove_file(&tmp1);
}

// ============================================================================
// T403/T404：query_rows 出口脱敏 + RLS 谓词注入（data-protection feature）
// ============================================================================

#[cfg(all(feature = "data-protection", feature = "permission"))]
#[tokio::test]
async fn test_query_rows_masking_and_rls() {
    use dbnexus::{
        access::data_protection::{DataProtection, MaskStrategy, MaskingEngine, RlsEngine},
        DbPool,
    };
    use std::sync::Arc;

    let db_path =
        std::env::temp_dir().join(format!("dbnexus_dp_{}.db", std::process::id()));
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = Arc::new(dbnexus::DbPool::new(&url).await.unwrap());

    let admin = pool.get_session("admin").await.unwrap();
    admin
        .execute_raw_ddl("CREATE TABLE orders (id INTEGER PRIMARY KEY, email TEXT NOT NULL, tenant_id TEXT NOT NULL, amount REAL NOT NULL)")
        .await
        .unwrap();
    admin
        .execute_raw("INSERT INTO orders (id, email, tenant_id, amount) VALUES (1, 'alice@example.com', 't-100', 10.0)")
        .await
        .unwrap();
    admin
        .execute_raw("INSERT INTO orders (id, email, tenant_id, amount) VALUES (2, 'bob@example.com', 't-200', 20.0)")
        .await
        .unwrap();

    // 为 default 角色授予 orders 表 select 权限（RLS 谓词由此角色触发）
    let mut roles = std::collections::HashMap::new();
    roles.insert(
        "admin".to_string(),
        dbnexus::access::permission::RolePolicy {
            tables: vec![dbnexus::access::permission::TablePermission {
                name: "*".to_string(),
                operations: vec![
                    dbnexus::access::permission::PermissionAction::Select,
                    dbnexus::access::permission::PermissionAction::Insert,
                    dbnexus::access::permission::PermissionAction::Update,
                    dbnexus::access::permission::PermissionAction::Delete,
                ],
            }],
        },
    );
    roles.insert(
        "default".to_string(),
        dbnexus::access::permission::RolePolicy {
            tables: vec![dbnexus::access::permission::TablePermission {
                name: "orders".to_string(),
                operations: vec![dbnexus::access::permission::PermissionAction::Select],
            }],
        },
    );
    pool.set_permission_config(dbnexus::access::permission::PermissionConfig { roles })
        .await
        .unwrap();

    // 配置：email 脱敏（哈希）+ orders 表 tenant_id RLS（仅 t-100 可见）
    pool.set_data_protection(DataProtection {
        masking: Some(Arc::new(MaskingEngine::new().rule("email", MaskStrategy::Hash))),
        rls: Some(Arc::new(RlsEngine::new().policy("orders", "tenant_id", "t-100"))),
    })
    .await;

    // admin（管理通道）：不注入 RLS，但出口仍脱敏
    let rows = pool
        .query_rows("SELECT id, email, tenant_id FROM orders ORDER BY id", "admin")
        .await
        .unwrap();
    assert_eq!(rows.len(), 2, "admin 不受 RLS 限制");
    let email = rows[0]["email"].as_str().unwrap();
    assert_eq!(email.len(), 64, "admin 出口同样脱敏");
    assert!(!email.contains('@'));
    assert_eq!(rows[0]["tenant_id"], "t-100");

    // 非 admin（无权限配置的默认策略允许 default 角色）→ RLS 注入生效
    let rows_rls = pool
        .query_rows("SELECT id, tenant_id FROM orders", "default")
        .await
        .unwrap();
    assert_eq!(rows_rls.len(), 1, "RLS 谓词应只放行 t-100");
    assert_eq!(rows_rls[0]["tenant_id"], "t-100");

    let _ = std::fs::remove_file(&db_path);
}
