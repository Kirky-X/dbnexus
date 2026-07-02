// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! DuckDB 连接池集成测试
//!
//! 覆盖：
//! - DuckDB 连接池创建（`:memory:` / `duckdb::memory:` URL）
//! - DuckDB Session DDL/DML/查询（execute_duckdb_raw / execute_duckdb）
//! - DuckDB 连接健康检查
//! - SeaORM 方法在 DuckDB 连接上的错误行为
//! - DuckDB 并发 Session 访问
//!
//! 所有测试使用 DuckDB 内存数据库，需要 `duckdb` feature。

#![cfg(feature = "duckdb")]

use dbnexus::{DbConfig, DbPool};

// ============================================================================
// 辅助函数
// ============================================================================

/// 创建 DuckDB 内存数据库连接池
async fn make_duckdb_pool() -> DbPool {
    let config = DbConfig {
        url: "duckdb::memory:".to_string(),
        max_connections: 5,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 5000,
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    DbPool::with_config(config)
        .await
        .expect("Failed to create DuckDB pool")
}

// ============================================================================
// 连接池创建测试
// ============================================================================

/// TEST-U-DDB-001: DuckDB 内存数据库连接池应成功创建
#[tokio::test]
async fn test_duckdb_pool_creation_memory() {
    let pool = make_duckdb_pool().await;
    let status = pool.status();
    // pool-warmup 未启用时连接按需创建，total 可能为 0
    // 验证池已创建且状态不变量成立
    assert_eq!(
        status.total,
        status.active + status.idle,
        "total should equal active + idle"
    );
}

/// TEST-U-DDB-002: DuckDB 通过 DbConfig 创建连接池应成功
#[tokio::test]
async fn test_duckdb_pool_creation_with_config() {
    let config = DbConfig {
        url: "duckdb::memory:".to_string(),
        max_connections: 3,
        min_connections: 1,
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await;
    assert!(pool.is_ok(), "DuckDB pool creation with config should succeed");
}

// ============================================================================
// Session DDL/DML/查询测试
// ============================================================================

/// TEST-U-DDB-003: DuckDB Session 应支持 CREATE TABLE / INSERT / SELECT
#[tokio::test]
async fn test_duckdb_session_ddl_and_query() {
    let pool = make_duckdb_pool().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // CREATE TABLE
    let ddl_result = session
        .execute_duckdb_raw("CREATE TABLE test_users (id INTEGER PRIMARY KEY, name VARCHAR)")
        .await;
    assert!(ddl_result.is_ok(), "CREATE TABLE should succeed: {:?}", ddl_result.err());

    // INSERT
    let insert_result = session
        .execute_duckdb_raw("INSERT INTO test_users VALUES (1, 'Alice')")
        .await;
    assert!(insert_result.is_ok(), "INSERT should succeed: {:?}", insert_result.err());

    let insert_result2 = session
        .execute_duckdb_raw("INSERT INTO test_users VALUES (2, 'Bob')")
        .await;
    assert!(insert_result2.is_ok(), "Second INSERT should succeed");

    // SELECT
    let rows = session
        .execute_duckdb("SELECT id, name FROM test_users ORDER BY id")
        .await
        .expect("SELECT should succeed");

    assert_eq!(rows.len(), 2, "Should have 2 rows");
    assert_eq!(rows[0].column_count(), 2, "Should have 2 columns");

    // 验证第一行数据
    let name = rows[0].get("name").expect("Should have 'name' column");
    if let duckdb::types::Value::Text(s) = name {
        assert_eq!(s, "Alice", "First row name should be Alice");
    } else {
        panic!("Expected Text value, got {:?}", name);
    }
}

/// TEST-U-DDB-004: DuckDB Session 应支持聚合查询
#[tokio::test]
async fn test_duckdb_session_aggregate_query() {
    let pool = make_duckdb_pool().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_duckdb_raw("CREATE TABLE orders (id INTEGER, amount DOUBLE)")
        .await
        .expect("CREATE TABLE should succeed");

    session
        .execute_duckdb_raw("INSERT INTO orders VALUES (1, 100.5), (2, 200.0), (3, 50.0)")
        .await
        .expect("INSERT should succeed");

    let rows = session
        .execute_duckdb("SELECT COUNT(*) AS cnt, SUM(amount) AS total FROM orders")
        .await
        .expect("Aggregate query should succeed");

    assert_eq!(rows.len(), 1, "Aggregate should return 1 row");
    let count = rows[0].get("cnt").expect("Should have 'cnt' column");
    if let duckdb::types::Value::BigInt(n) = count {
        assert_eq!(*n, 3, "Count should be 3");
    } else {
        panic!("Expected BigInt for count, got {:?}", count);
    }
}

// ============================================================================
// 健康检查测试
// ============================================================================

/// TEST-U-DDB-005: DuckDB 连接健康检查应返回 true
#[tokio::test]
async fn test_duckdb_pool_health_check() {
    let pool = make_duckdb_pool().await;

    // 通过 get_session 验证连接可用性（内部会创建连接）
    let session = pool.get_session("admin").await;
    assert!(session.is_ok(), "get_session should succeed with healthy connection");

    // 验证 Session 可以执行查询
    let session = session.unwrap();
    let rows = session
        .execute_duckdb("SELECT 1 AS health")
        .await
        .expect("Health check query should succeed");
    assert_eq!(rows.len(), 1, "Health check should return 1 row");
}

// ============================================================================
// 错误行为测试
// ============================================================================

/// TEST-U-DDB-006: SeaORM 方法在 DuckDB 连接上应返回错误
#[tokio::test]
async fn test_duckdb_session_seaorm_method_fails() {
    let pool = make_duckdb_pool().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // execute_raw_ddl 内部调用 self.connection()?.execute_unprepared()
    // connection() 调用 as_sea_orm()，对 DuckDB 连接返回错误
    let result = session.execute_raw_ddl("CREATE TABLE test (id INTEGER)").await;
    assert!(
        result.is_err(),
        "SeaORM execute_raw_ddl should fail on DuckDB connection"
    );

    let err = result.unwrap_err();
    assert!(
        format!("{err}").contains("SeaORM") || format!("{err}").contains("Operation requires"),
        "Error should mention SeaORM requirement, got: {err}"
    );
}

/// TEST-U-DDB-007: DuckDB Session 事务操作应返回错误（事务需要 SeaORM 连接）
#[tokio::test]
async fn test_duckdb_session_transaction_fails() {
    let pool = make_duckdb_pool().await;
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // begin_transaction 内部调用 self.connection()?.begin()
    // 对 DuckDB 连接，connection() 返回错误
    let result = session.begin_transaction().await;
    assert!(
        result.is_err(),
        "begin_transaction should fail on DuckDB connection (requires SeaORM)"
    );
}

// ============================================================================
// 并发测试
// ============================================================================

/// TEST-U-DDB-008: DuckDB 连接池应支持并发 Session 访问
///
/// 注意：DuckDB `:memory:` 数据库按连接隔离，每个新连接是独立的内存数据库。
/// 为确保所有 Session 共享同一数据库，使用 `max_connections: 1` 强制连接复用。
#[tokio::test(flavor = "multi_thread")]
async fn test_duckdb_pool_concurrent_sessions() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // max_connections=1 确保所有 session 复用同一连接（共享 :memory: 数据库）
    let config = DbConfig {
        url: "duckdb::memory:".to_string(),
        max_connections: 1,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 10000,
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = Arc::new(
        DbPool::with_config(config)
            .await
            .expect("Failed to create DuckDB pool"),
    );

    // 预先创建表（使用第一个 session，释放后连接回到池中供后续复用）
    {
        let session = pool.get_session("admin").await.expect("Failed to get setup session");
        session
            .execute_duckdb_raw("CREATE TABLE concurrent_test (id INTEGER, value INTEGER)")
            .await
            .expect("CREATE TABLE should succeed");
    }

    let success_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for i in 0..4 {
        let pool_clone = pool.clone();
        let success_clone = success_count.clone();
        handles.push(tokio::spawn(async move {
            let session = match pool_clone.get_session("admin").await {
                Ok(s) => s,
                Err(_) => return,
            };

            let sql = format!("INSERT INTO concurrent_test VALUES ({i}, {i} * 10)");
            if session.execute_duckdb_raw(&sql).await.is_ok() {
                success_clone.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for handle in handles {
        handle.await.expect("Task panicked");
    }

    assert_eq!(
        success_count.load(Ordering::SeqCst),
        4,
        "All concurrent inserts should succeed"
    );

    // 验证数据（复用同一连接，能看到所有插入的行）
    let session = pool.get_session("admin").await.expect("Failed to get verification session");
    let rows = session
        .execute_duckdb("SELECT COUNT(*) AS cnt FROM concurrent_test")
        .await
        .expect("COUNT query should succeed");

    let count = rows[0].get("cnt").expect("Should have 'cnt' column");
    if let duckdb::types::Value::BigInt(n) = count {
        assert_eq!(*n, 4, "Should have 4 rows after concurrent inserts");
    } else {
        panic!("Expected BigInt for count, got {:?}", count);
    }
}

/// TEST-U-DDB-009: DuckDB 连接池 status 应正确反映连接状态
#[tokio::test]
async fn test_duckdb_pool_status_invariants() {
    let pool = make_duckdb_pool().await;
    let status = pool.status();

    // 验证连接池状态不变量
    assert!(
        status.total >= status.active,
        "total ({}) should be >= active ({})",
        status.total,
        status.active
    );
    assert_eq!(
        status.total,
        status.active + status.idle,
        "total ({}) should equal active ({}) + idle ({})",
        status.total,
        status.active,
        status.idle
    );
}
