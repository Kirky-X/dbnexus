// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
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

use dbnexus::database::DuckValue;
use dbnexus::foundation::PoolConfig;
use dbnexus::{DbConfig, DbError, DbPool};

// 辅助函数

/// 创建 DuckDB 内存数据库连接池
async fn make_duckdb_pool() -> DbPool {
    let config = DbConfig {
        url: "duckdb::memory:".to_string(),
        pool_config: PoolConfig {
            max_connections: 5,
            min_connections: 1,
            idle_timeout: 300,
            acquire_timeout: 5000,
        },
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    DbPool::with_config(config)
        .await
        .expect("Failed to create DuckDB pool")
}

// 连接池创建测试

/// DuckDB 内存数据库连接池应成功创建
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

/// DuckDB 通过 DbConfig 创建连接池应成功
#[tokio::test]
async fn test_duckdb_pool_creation_with_config() {
    let config = DbConfig {
        url: "duckdb::memory:".to_string(),
        pool_config: PoolConfig {
            max_connections: 3,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await;
    assert!(
        pool.is_ok(),
        "DuckDB pool creation with config should succeed"
    );
}

// Session DDL/DML/查询测试

/// DuckDB Session 应支持 CREATE TABLE / INSERT / SELECT
#[tokio::test]
async fn test_duckdb_session_ddl_and_query() {
    let pool = make_duckdb_pool().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    // CREATE TABLE
    let ddl_result = session
        .execute_duckdb_raw("CREATE TABLE test_users (id INTEGER PRIMARY KEY, name VARCHAR)")
        .await;
    assert!(
        ddl_result.is_ok(),
        "CREATE TABLE should succeed: {:?}",
        ddl_result.err()
    );

    // INSERT
    let insert_result = session
        .execute_duckdb_raw("INSERT INTO test_users VALUES (1, 'Alice')")
        .await;
    assert!(
        insert_result.is_ok(),
        "INSERT should succeed: {:?}",
        insert_result.err()
    );

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

    let name = rows[0].get("name").expect("Should have 'name' column");
    if let duckdb::types::Value::Text(s) = name {
        assert_eq!(s, "Alice", "First row name should be Alice");
    } else {
        panic!("Expected Text value, got {:?}", name);
    }
}

/// DuckDB Session 应支持聚合查询
#[tokio::test]
async fn test_duckdb_session_aggregate_query() {
    let pool = make_duckdb_pool().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

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

// 健康检查测试

/// DuckDB 连接健康检查应返回 true
#[tokio::test]
async fn test_duckdb_pool_health_check() {
    let pool = make_duckdb_pool().await;

    // 通过 get_session 验证连接可用性（内部会创建连接）
    let session = pool.get_session("admin").await;
    assert!(
        session.is_ok(),
        "get_session should succeed with healthy connection"
    );

    let session = session.unwrap();
    let rows = session
        .execute_duckdb("SELECT 1 AS health")
        .await
        .expect("Health check query should succeed");
    assert_eq!(rows.len(), 1, "Health check should return 1 row");
}

// 错误行为测试

/// SeaORM 方法在 DuckDB 连接上应返回错误
#[tokio::test]
async fn test_duckdb_session_seaorm_method_fails() {
    let pool = make_duckdb_pool().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    // execute_raw_ddl 内部调用 self.connection()?.execute_unprepared()
    // connection() 调用 as_sea_orm()，对 DuckDB 连接返回错误
    let result = session
        .execute_raw_ddl("CREATE TABLE test (id INTEGER)")
        .await;
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

/// DuckDB Session 事务操作应返回错误（事务需要 SeaORM 连接）
#[tokio::test]
async fn test_duckdb_session_transaction_fails() {
    let pool = make_duckdb_pool().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    // begin_transaction 内部调用 self.connection()?.begin()
    // 对 DuckDB 连接，connection() 返回错误
    let result = session.begin_transaction().await;
    assert!(
        result.is_err(),
        "begin_transaction should fail on DuckDB connection (requires SeaORM)"
    );
}

// 并发测试

/// DuckDB 连接池应支持并发 Session 访问
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
        pool_config: PoolConfig {
            max_connections: 1,
            min_connections: 1,
            idle_timeout: 300,
            acquire_timeout: 10000,
        },
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
        let session = pool
            .get_session("admin")
            .await
            .expect("Failed to get setup session");
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
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get verification session");
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

/// DuckDB 连接池 status 应正确反映连接状态
#[tokio::test]
async fn test_duckdb_pool_status_invariants() {
    let pool = make_duckdb_pool().await;
    let status = pool.status();

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

// Session 安全链测试

/// 非 admin role 执行 DDL 应被拒绝
#[tokio::test]
async fn test_duckdb_non_admin_ddl_rejected() {
    let pool = make_duckdb_pool().await;
    // 无权限配置时，非 admin/system 角色在 get_session 阶段即被拒绝
    let session_result = pool.get_session("viewer").await;
    assert!(
        session_result.is_err(),
        "non-admin role 'viewer' should be rejected without permission config"
    );
    let err = format!("{}", session_result.err().unwrap());
    assert!(
        err.contains("not allowed") || err.contains("permission"),
        "error should mention permission: {err}"
    );
}

/// admin role 执行 DDL 应成功
#[tokio::test]
async fn test_duckdb_admin_ddl_allowed() {
    let pool = make_duckdb_pool().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get admin session");

    session
        .execute_duckdb_raw("CREATE TABLE admin_table (id INTEGER PRIMARY KEY, data VARCHAR)")
        .await
        .expect("admin DDL should succeed");

    session
        .execute_duckdb_raw("INSERT INTO admin_table VALUES (1, 'test')")
        .await
        .expect("insert after admin DDL should succeed");

    let rows = session
        .execute_duckdb("SELECT data FROM admin_table WHERE id = 1")
        .await
        .expect("select should succeed");
    assert_eq!(rows.len(), 1, "should see 1 row");
}

/// execute_duckdb 对 DDL 语句应被拒绝（只读查询方法）
#[tokio::test]
async fn test_duckdb_query_rejects_ddl() {
    let pool = make_duckdb_pool().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    // execute_duckdb 是只读查询方法，应拒绝 DDL
    let result = session
        .execute_duckdb("CREATE TABLE should_fail (id INTEGER)")
        .await;
    assert!(
        result.is_err(),
        "execute_duckdb should reject DDL even for admin"
    );
}

// 数据类型集成测试

/// DuckDB 应支持多种数据类型
#[tokio::test]
async fn test_duckdb_multiple_data_types() {
    let pool = make_duckdb_pool().await;
    let session = pool
        .get_session("admin")
        .await
        .expect("Failed to get session");

    session
        .execute_duckdb_raw(
            "CREATE TABLE types_test (id INTEGER, name VARCHAR, score DOUBLE, active BOOLEAN)",
        )
        .await
        .expect("create table");

    session
        .execute_duckdb_raw(
            "INSERT INTO types_test VALUES (1, 'Alice', 95.5, true), (2, 'Bob', NULL, false)",
        )
        .await
        .expect("insert");

    let rows = session
        .execute_duckdb("SELECT id, name, score, active FROM types_test ORDER BY id")
        .await
        .expect("select");

    assert_eq!(rows.len(), 2, "should have 2 rows");
    assert_eq!(rows[0].column_count(), 4, "should have 4 columns");

    let name = rows[0].get("name").expect("should have name");
    if let duckdb::types::Value::Text(s) = name {
        assert_eq!(s, "Alice");
    } else {
        panic!("Expected Text, got {:?}", name);
    }

    let score = rows[1].get("score").expect("should have score");
    assert!(
        matches!(score, duckdb::types::Value::Null),
        "Bob's score should be NULL, got {:?}",
        score
    );
}

// 既有连接接入（with_existing_duckdb_connection）

/// 从既有 DuckDB 连接构建池：池大小按参数复制连接，副本共享同一数据库
#[tokio::test]
async fn test_pool_from_existing_duckdb_connection() {
    let conn = duckdb::Connection::open_in_memory().expect("open in-memory duckdb");
    let pool =
        DbPool::with_existing_duckdb_connection(conn, 2).expect("pool from an existing connection");

    assert_eq!(
        pool.status().total,
        2,
        "the pool should be filled with pool_size connections"
    );

    let session = pool.get_session("admin").await.expect("admin session");
    session
        .execute_duckdb_raw("CREATE TABLE shared_t (id INTEGER)")
        .await
        .expect("create table on the shared database");
    session
        .execute_duckdb_raw("INSERT INTO shared_t VALUES (7)")
        .await
        .expect("insert on the shared database");
    let rows = session
        .execute_duckdb("SELECT id FROM shared_t")
        .await
        .expect("select on the shared database");
    assert_eq!(rows.len(), 1);
    drop(session);

    // try_clone 共享底层数据库实例：换一条池内连接仍读到同一张表
    let session = pool.get_session("admin").await.expect("second session");
    let rows = session
        .execute_duckdb("SELECT id FROM shared_t")
        .await
        .expect("select through another pooled connection");
    assert_eq!(
        rows.len(),
        1,
        "cloned connections must share the same database"
    );
}

/// pool_size = 0 收敛为单连接池（与其他构造入口同口径）
#[tokio::test]
async fn test_pool_from_existing_duckdb_connection_clamps_zero() {
    let conn = duckdb::Connection::open_in_memory().expect("open in-memory duckdb");
    let pool = DbPool::with_existing_duckdb_connection(conn, 0).expect("pool with clamped size");
    assert_eq!(pool.status().total, 1, "pool_size 0 must clamp to 1");
}

// 权限门（非 admin 会话的 DuckDB 安全链）

const ANALYST_YAML: &str = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations: ["select", "insert", "update", "delete"]
  analyst:
    tables:
      - name: "users"
        operations: ["select"]
"#;

/// 写入临时权限文件并返回路径（唯一名：并发用例不共享文件）
fn write_analyst_permissions() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "dbnexus_duckdb_perm_{}_{}.yaml",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, ANALYST_YAML).expect("write permissions yaml");
    path
}

/// 带 analyst 角色权限配置的 DuckDB 池（单连接：`duckdb::memory:` 按连接隔离）
async fn make_duckdb_pool_with_analyst() -> (std::path::PathBuf, DbPool) {
    let path = write_analyst_permissions();
    let config = DbConfig {
        url: "duckdb::memory:".to_string(),
        permissions_path: Some(path.to_string_lossy().to_string()),
        pool_config: PoolConfig {
            max_connections: 1,
            min_connections: 1,
            idle_timeout: 300,
            acquire_timeout: 10000,
        },
        ..Default::default()
    };
    let pool = DbPool::with_config(config)
        .await
        .expect("duckdb pool with permission config");
    (path, pool)
}

/// 非 admin 会话的 DuckDB 安全门：允许的表放行、未列出表/DDL/无法解析语句全部拒绝
#[tokio::test]
async fn test_duckdb_analyst_permission_gates() {
    let (perm_path, pool) = make_duckdb_pool_with_analyst().await;

    let admin = pool.get_session("admin").await.expect("admin session");
    admin
        .execute_duckdb_raw("CREATE TABLE users (id INTEGER)")
        .await
        .expect("create users");
    admin
        .execute_duckdb_raw("CREATE TABLE secrets (id INTEGER)")
        .await
        .expect("create secrets");
    drop(admin);

    let analyst = pool.get_session("analyst").await.expect("analyst session");

    // 允许的表：参数化查询经安全门 → 表级权限通过
    let rows = analyst
        .execute_duckdb_with_params("SELECT id FROM users WHERE id = ?", vec![DuckValue::Int(1)])
        .await
        .expect("a permitted parameterized select must pass");
    assert!(rows.is_empty(), "users is empty after creation");

    // 未列出的表：非参数化查询路径拒绝
    let err = analyst
        .execute_duckdb("SELECT * FROM secrets")
        .await
        .expect_err("an unlisted table must be denied");
    assert!(matches!(err, DbError::Permission(_)), "got {err:?}");

    // 未列出的表：参数化 DML 路径同样拒绝
    let err = analyst
        .execute_duckdb_with_params(
            "SELECT id FROM secrets WHERE id = ?",
            vec![DuckValue::Int(1)],
        )
        .await
        .expect_err("an unlisted table must be denied on the parameterized path");
    assert!(matches!(err, DbError::Permission(_)), "got {err:?}");

    // DDL：非 admin 在两条 raw 路径上均被拒绝
    let err = analyst
        .execute_duckdb_raw("CREATE TABLE denied_t (id INTEGER)")
        .await
        .expect_err("non-admin DDL must be denied");
    assert!(matches!(err, DbError::Permission(_)), "got {err:?}");
    let err = analyst
        .execute_duckdb_raw_with_params("CREATE TABLE denied_t2 (id INTEGER)", vec![])
        .await
        .expect_err("non-admin DDL must be denied on the parameterized path");
    assert!(matches!(err, DbError::Permission(_)), "got {err:?}");

    // 无表名（SELECT 1）与解析失败：非 admin 无法完成权限检查 → 拒绝
    for sql in ["SELECT 1", "SELEC * FROM users"] {
        let err = analyst
            .execute_duckdb(sql)
            .await
            .expect_err("a statement without a resolvable table must be denied");
        assert!(
            matches!(err, DbError::Permission(_)),
            "sql {sql}: got {err:?}"
        );
    }

    // 同两条路径经安全门（参数化入口）：门内同样拒绝
    for sql in ["SELECT 1", "SELEC * FROM users"] {
        let err = analyst
            .execute_duckdb_with_params(sql, vec![])
            .await
            .expect_err("the gate must deny unresolvable statements for non-admin roles");
        assert!(
            matches!(err, DbError::Permission(_)),
            "sql {sql}: got {err:?}"
        );
    }

    let _ = std::fs::remove_file(perm_path);
}

/// admin 会话经参数化 raw 路径执行 DDL：走 DdlGuard 放行后落库
#[tokio::test]
async fn test_duckdb_admin_ddl_through_parameterized_path() {
    let (perm_path, pool) = make_duckdb_pool_with_analyst().await;
    let admin = pool.get_session("admin").await.expect("admin session");

    admin
        .execute_duckdb_raw_with_params("CREATE TABLE gate_t (id INTEGER)", vec![])
        .await
        .expect("admin DDL must pass the security gate");
    admin
        .execute_duckdb_raw("INSERT INTO gate_t VALUES (1)")
        .await
        .expect("insert after DDL");
    let rows = admin
        .execute_duckdb("SELECT id FROM gate_t")
        .await
        .expect("select");
    assert_eq!(rows.len(), 1);

    let _ = std::fs::remove_file(perm_path);
}

// 失败路径：查询错误统一映射为连接错误（不 panic）

/// 空表标识符（无法提取有效表名）：安全门按权限错误拒绝，不进入执行
#[tokio::test]
async fn test_duckdb_empty_table_identifier_rejected() {
    let pool = make_duckdb_pool().await;
    let session = pool.get_session("admin").await.expect("session");

    let sql = "SELECT * FROM \"\"";
    let err = session
        .execute_duckdb(sql)
        .await
        .expect_err("an empty table identifier must be rejected");
    assert!(matches!(err, DbError::Permission(_)), "got {err:?}");

    let err = session
        .execute_duckdb_raw(sql)
        .await
        .expect_err("an empty table identifier must be rejected on the raw path");
    assert!(matches!(err, DbError::Permission(_)), "got {err:?}");

    let err = session
        .execute_duckdb_with_params(sql, vec![])
        .await
        .expect_err("an empty table identifier must be rejected on the parameterized path");
    assert!(matches!(err, DbError::Permission(_)), "got {err:?}");
}

/// 缺失表 / 非法 SQL：查询失败经映射返回错误
#[tokio::test]
async fn test_duckdb_query_failures_are_mapped() {
    let pool = make_duckdb_pool().await;
    let session = pool.get_session("admin").await.expect("session");

    let err = session
        .execute_duckdb("SELECT * FROM missing_table")
        .await
        .expect_err("a missing table must fail the query");
    assert!(format!("{err}").contains("DuckDB"), "got {err}");

    let err = session
        .execute_duckdb_with_params(
            "SELECT * FROM missing_table WHERE id = ?",
            vec![DuckValue::Int(1)],
        )
        .await
        .expect_err("a missing table must fail the parameterized query");
    assert!(format!("{err}").contains("DuckDB"), "got {err}");
}

// 配置校验与自动迁移跳过

/// 非法池配置（max_connections = 0）在建立连接前被拒绝
#[tokio::test]
async fn test_pool_rejects_invalid_config() {
    let config = DbConfig {
        url: "duckdb::memory:".to_string(),
        pool_config: PoolConfig {
            max_connections: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let err = match DbPool::with_config(config).await {
        Ok(_) => panic!("max_connections = 0 must be rejected before connecting"),
        Err(err) => err,
    };
    assert!(
        format!("{err}").contains("max_connections"),
        "the error should name the invalid key, got {err}"
    );
}

/// 启用自动迁移但目录不存在：池创建跳过迁移而非报错
#[cfg(feature = "auto-migrate")]
#[tokio::test]
async fn test_auto_migrate_skips_missing_directory() {
    let missing =
        std::env::temp_dir().join(format!("dbnexus_missing_migrations_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&missing);

    let config = DbConfig {
        url: "duckdb::memory:".to_string(),
        auto_migrate: true,
        migrations_dir: Some(missing),
        ..Default::default()
    };
    // DuckDB 池不支持 run_migrations（迁移器要求 SeaORM 连接），
    // 本用例只锚池创建阶段的跳过语义：目录缺失不得让 with_config 失败
    let pool = DbPool::with_config(config)
        .await
        .expect("pool creation must not fail when the migrations dir is absent");
    assert_eq!(
        pool.status().total,
        pool.status().active + pool.status().idle
    );
}
