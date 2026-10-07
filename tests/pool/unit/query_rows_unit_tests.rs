// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 统一行查询 API（query_rows）与 scatter-gather 行查询测试
//!
//! 需要 sqlite + sql-parser + runtime-tokio-rustls feature。

#![cfg(all(
    feature = "runtime-tokio-rustls",
    feature = "sqlite",
    feature = "sql-parser"
))]

// scatter-gather 跨分片行查询（门禁：无 scatter-gather feature 时其余
// 用例仍需可编译运行，故按 feature 隔离）
#[cfg(feature = "scatter-gather")]
use dbnexus::{
    AggregateFunction, OrderKey, PartialFailurePolicy, ScatterGatherExecutor, ShardRouter,
};

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
    let rows = pool
        .query_rows("SELECT id, val FROM t_qr ORDER BY id", "admin")
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

#[cfg(feature = "scatter-gather")]
#[tokio::test]
async fn test_scatter_query_rows_with_aggregate() {
    use std::sync::Arc;
    use std::time::Duration;

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
// query_rows 出口脱敏 + RLS 谓词注入（data-protection feature）
// ============================================================================

#[cfg(all(feature = "data-protection", feature = "permission"))]
#[tokio::test]
async fn test_query_rows_masking_and_rls() {
    use dbnexus::access::data_protection::{
        DataProtection, MaskStrategy, MaskingEngine, RlsEngine,
    };
    use std::sync::Arc;

    let db_path = std::env::temp_dir().join(format!("dbnexus_dp_{}.db", std::process::id()));
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
        masking: Some(Arc::new(
            MaskingEngine::new().rule("email", MaskStrategy::Hash),
        )),
        rls: Some(Arc::new(RlsEngine::new().policy(
            "orders",
            "tenant_id",
            "t-100",
        ))),
    })
    .await;

    // admin（管理通道）：不注入 RLS，但出口仍脱敏
    let rows = pool
        .query_rows(
            "SELECT id, email, tenant_id FROM orders ORDER BY id",
            "admin",
        )
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

/// scatter_query_rows_merged：跨分片全局归并 + 全局分页
///
/// 既有用例只覆盖不带 `order_by` 的 `scatter_query_rows`；归并分支
/// （`merge_shard_rows` + `apply_global_pagination`）此前无执行。本用例断言
/// 跨分片全局有序，且分页切片与全量序列一致（分页不得按分片局部切片）。
#[cfg(feature = "scatter-gather")]
#[tokio::test]
async fn test_scatter_query_rows_merged_orders_and_paginates_globally() {
    use std::sync::Arc;
    use std::time::Duration;

    let tmp0 = std::env::temp_dir().join(format!("dbnexus_t401_m0_{}.db", std::process::id()));
    let tmp1 = std::env::temp_dir().join(format!("dbnexus_t401_m1_{}.db", std::process::id()));
    let url0 = format!("sqlite:{}?mode=rwc", tmp0.display());
    let url1 = format!("sqlite:{}?mode=rwc", tmp1.display());

    let mut router = ShardRouter::with_strategy("hash", 2);
    router.register_shard(0, "shard_0".to_string(), url0.clone());
    let pool0 = Arc::new(dbnexus::DbPool::new(&url0).await.unwrap());
    let admin0 = pool0.get_session("admin").await.unwrap();
    admin0
        .execute_raw_ddl("CREATE TABLE t_m (val REAL NOT NULL)")
        .await
        .unwrap();
    // 分片内乱序，确保归并而非拼接生效
    admin0
        .execute_raw("INSERT INTO t_m (val) VALUES (5.0), (1.0)")
        .await
        .unwrap();
    router.set_pool(0, pool0).unwrap();

    router.register_shard(1, "shard_1".to_string(), url1.clone());
    let pool1 = Arc::new(dbnexus::DbPool::new(&url1).await.unwrap());
    let admin1 = pool1.get_session("admin").await.unwrap();
    admin1
        .execute_raw_ddl("CREATE TABLE t_m (val REAL NOT NULL)")
        .await
        .unwrap();
    admin1
        .execute_raw("INSERT INTO t_m (val) VALUES (4.0), (2.0), (3.0)")
        .await
        .unwrap();
    router.set_pool(1, pool1).unwrap();

    let executor = ScatterGatherExecutor::new(
        Arc::new(router),
        Duration::from_secs(5),
        PartialFailurePolicy::BestEffort,
    );

    // 全量归并（升序）
    let full = executor
        .scatter_query_rows_merged(
            "SELECT val FROM t_m",
            "admin",
            None,
            &[OrderKey::asc("val")],
            100,
            0,
        )
        .await
        .unwrap();
    let vals: Vec<f64> = full
        .merged_rows
        .iter()
        .map(|r| r["val"].as_f64().unwrap())
        .collect();
    assert_eq!(vals.len(), 5, "两分片共 5 行应全部归并: {vals:?}");
    assert!(
        vals.windows(2).all(|w| w[0] <= w[1]),
        "跨分片应全局升序而非分片内有序: {vals:?}"
    );

    // 全局分页：切片与全量序列一致（不是按分片各自的 offset）
    let page = executor
        .scatter_query_rows_merged(
            "SELECT val FROM t_m",
            "admin",
            None,
            &[OrderKey::asc("val")],
            2,
            1,
        )
        .await
        .unwrap();
    let page_vals: Vec<f64> = page
        .merged_rows
        .iter()
        .map(|r| r["val"].as_f64().unwrap())
        .collect();
    assert_eq!(
        page_vals,
        vals[1..3].to_vec(),
        "全局分页切片应与全量序列一致"
    );

    let _ = std::fs::remove_file(&tmp0);
    let _ = std::fs::remove_file(&tmp1);
}

/// scatter 部分失败策略：BestEffort 返回部分结果 + 失败分片信息；Fail 整体失败
///
/// 构造「一个正常分片 + 一个指向不可达库的分片」，断言两种策略的语义差异
/// （既有用例只覆盖全部分片成功的路径）。
#[cfg(feature = "scatter-gather")]
#[tokio::test]
async fn test_scatter_partial_failure_policies() {
    use std::sync::Arc;
    use std::time::Duration;

    let tmp0 = std::env::temp_dir().join(format!("dbnexus_pf_s0_{}.db", std::process::id()));
    let tmp1 = std::env::temp_dir().join(format!("dbnexus_pf_s1_{}.db", std::process::id()));
    let url0 = format!("sqlite:{}?mode=rwc", tmp0.display());
    let url1 = format!("sqlite:{}?mode=rwc", tmp1.display());

    let mut router = ShardRouter::with_strategy("hash", 2);
    router.register_shard(0, "shard_0".to_string(), url0.clone());
    let pool0 = Arc::new(dbnexus::DbPool::new(&url0).await.unwrap());
    let admin0 = pool0.get_session("admin").await.unwrap();
    admin0
        .execute_raw_ddl("CREATE TABLE t_pf (val REAL NOT NULL)")
        .await
        .unwrap();
    admin0
        .execute_raw("INSERT INTO t_pf (val) VALUES (1.0)")
        .await
        .unwrap();
    router.set_pool(0, pool0).unwrap();

    // 分片 1：合法但未建表的库——查询 "no such table" 即分片失败
    router.register_shard(1, "shard_1".to_string(), url1.clone());
    let pool1 = Arc::new(dbnexus::DbPool::new(&url1).await.unwrap());
    router.set_pool(1, pool1).unwrap();

    let router = Arc::new(router);

    // BestEffort：部分成功仍返回结果，失败分片显性列出
    let best = ScatterGatherExecutor::new(
        Arc::clone(&router),
        Duration::from_secs(5),
        PartialFailurePolicy::BestEffort,
    );
    let result = best
        .scatter_query_rows("SELECT val FROM t_pf", "admin", None)
        .await
        .expect("BestEffort 应返回部分结果");
    assert_eq!(result.failed_shards.len(), 1, "应列出 1 个失败分片");
    assert_eq!(result.failed_shards[0].shard_id, 1, "失败分片应为 shard_1");
    assert!(!result.shard_rows.is_empty(), "正常分片的数据行应被保留");

    // Fail：任一分片失败则整体失败（错误信息含失败分片数）
    let strict = ScatterGatherExecutor::new(
        Arc::clone(&router),
        Duration::from_secs(5),
        PartialFailurePolicy::Fail,
    );
    let err = strict
        .scatter_query_rows("SELECT val FROM t_pf", "admin", None)
        .await
        .expect_err("Fail 策略下应整体失败");
    assert!(
        err.contains("1 shard(s) failed"),
        "错误信息应含失败分片数，实际: {err}"
    );

    // 兼容入口与 scatter_query_rows 同语义
    let compat = best
        .scatter_query("SELECT val FROM t_pf", "admin")
        .await
        .expect("兼容入口应可用");
    assert!(compat.failed_shards.len() <= 1, "兼容入口语义一致");

    let _ = std::fs::remove_file(&tmp0);
}

/// scatter 超时：collect 阶段超时必须显性返回错误（不得静默返回半成品）
#[cfg(feature = "scatter-gather")]
#[tokio::test]
async fn test_scatter_timeout_is_explicit() {
    use std::sync::Arc;
    use std::time::Duration;

    let tmp0 = std::env::temp_dir().join(format!("dbnexus_to_s0_{}.db", std::process::id()));
    let url0 = format!("sqlite:{}?mode=rwc", tmp0.display());
    let mut router = ShardRouter::with_strategy("hash", 1);
    router.register_shard(0, "shard_0".to_string(), url0.clone());
    let pool0 = Arc::new(dbnexus::DbPool::new(&url0).await.unwrap());
    let admin0 = pool0.get_session("admin").await.unwrap();
    admin0
        .execute_raw_ddl("CREATE TABLE t_to (val REAL NOT NULL)")
        .await
        .unwrap();
    router.set_pool(0, pool0).unwrap();

    // 1 纳秒超时：collect 必然超时 → 显性错误
    let executor = ScatterGatherExecutor::new(
        Arc::new(router),
        Duration::from_nanos(1),
        PartialFailurePolicy::BestEffort,
    );
    let err = executor
        .scatter_query_rows("SELECT val FROM t_to", "admin", None)
        .await
        .expect_err("超时必须显性报错");
    assert!(
        err.contains("timed out"),
        "超时错误信息应可辨识，实际: {err}"
    );

    let _ = std::fs::remove_file(&tmp0);
}
