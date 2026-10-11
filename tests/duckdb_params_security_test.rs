// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! DuckDB 参数化查询 + 事务批量执行回归测试。
//!
//! 覆盖 alphalloy 统一 DuckDB 迁移依赖的三个安全关键能力：
//! 1. `execute_with_params` / `query_with_params`：参数绑定（值不会被解析为 SQL）
//! 2. sqlparser 安全门对 DuckDB 方言的接受度（ON CONFLICT / INSERT..SELECT..WHERE /
//!    CREATE SEQUENCE / RETURNING）
//! 3. `execute_transaction`：多语句原子性（失败回滚）

#![cfg(all(feature = "duckdb", feature = "sql-parser"))]

use dbnexus::DbPool;
use dbnexus::database::DuckValue;

/// 单连接池（max=min=1）：模拟 alphalloy 对文件型 DuckDB 的独占共享模式。
async fn single_conn_pool(url: &str) -> DbPool {
    let config = dbnexus::DbConfig {
        url: url.to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 1,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    DbPool::with_config(config)
        .await
        .expect("pool creation failed")
}

#[tokio::test]
async fn params_roundtrip_and_injection_safety() {
    let pool = single_conn_pool("duckdb::memory:").await;
    let session = pool.get_session("admin").await.expect("session");

    session
        .execute_duckdb_raw("CREATE TABLE t_users (name VARCHAR, age INTEGER, score DOUBLE)")
        .await
        .expect("create table");

    // 参数化插入（含单引号注入载荷：应作为字面量存储而非 SQL 执行）
    let injected = "r').payload; DROP TABLE t_users;--";
    let affected = session
        .execute_duckdb_raw_with_params(
            "INSERT INTO t_users (name, age, score) VALUES (?, ?, ?)",
            vec![
                DuckValue::Text(injected.to_string()),
                DuckValue::Int(30),
                DuckValue::Double(1.5),
            ],
        )
        .await
        .expect("parameterized insert");
    assert_eq!(affected.rows_affected, 1);

    // 参数化查询 + 表仍存在（注入未生效）
    let rows = session
        .execute_duckdb_with_params(
            "SELECT name, age, score FROM t_users WHERE age > ?",
            vec![DuckValue::Int(18)],
        )
        .await
        .expect("parameterized query");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("name"),
        Some(&DuckValue::Text(injected.to_string()))
    );
    assert_eq!(rows[0].get("age"), Some(&DuckValue::Int(30)));
    assert_eq!(rows[0].get("score"), Some(&DuckValue::Double(1.5)));

    let count = session
        .execute_duckdb("SELECT COUNT(*) AS c FROM t_users")
        .await
        .expect("count");
    assert_eq!(count[0].get("c"), Some(&DuckValue::BigInt(1)));
}

#[tokio::test]
async fn sqlparser_gate_accepts_duckdb_dialect() {
    let pool = single_conn_pool("duckdb::memory:").await;
    let session = pool.get_session("admin").await.expect("session");

    // DuckDB 特有语法逐条过安全门（任何一条被 sqlparser 拒绝都会让业务 store 失败）
    session
        .execute_duckdb_raw("CREATE SEQUENCE IF NOT EXISTS seq_test START 1")
        .await
        .expect("CREATE SEQUENCE should pass the safety gate");
    session
        .execute_duckdb_raw("CREATE TABLE t_conflict (k VARCHAR PRIMARY KEY, v INTEGER)")
        .await
        .expect("create table");
    // ON CONFLICT DO UPDATE（upsert）
    session
        .execute_duckdb_raw_with_params(
            "INSERT INTO t_conflict (k, v) VALUES (?, ?) ON CONFLICT (k) DO UPDATE SET v = excluded.v",
            vec![DuckValue::Text("a".into()), DuckValue::Int(1)],
        )
        .await
        .expect("ON CONFLICT upsert should pass the safety gate");
    session
        .execute_duckdb_raw_with_params(
            "INSERT INTO t_conflict (k, v) VALUES (?, ?) ON CONFLICT (k) DO UPDATE SET v = excluded.v",
            vec![DuckValue::Text("a".into()), DuckValue::Int(2)],
        )
        .await
        .expect("second upsert with the same key");
    // INSERT OR REPLACE（nav_history upsert 语法）
    session
        .execute_duckdb_raw("CREATE TABLE t_replace (k VARCHAR PRIMARY KEY, v INTEGER)")
        .await
        .expect("create table t_replace");
    session
        .execute_duckdb_raw_with_params(
            "INSERT OR REPLACE INTO t_replace (k, v) VALUES (?, ?)",
            vec![DuckValue::Text("x".into()), DuckValue::Int(9)],
        )
        .await
        .expect("INSERT OR REPLACE should pass the safety gate");
    // INSERT ... SELECT ... WHERE NOT EXISTS（default 组合延迟创建语法）
    session
        .execute_duckdb_raw_with_params(
            "INSERT INTO t_conflict (k, v) SELECT ?, ? WHERE NOT EXISTS (SELECT 1 FROM t_conflict WHERE k = ?)",
            vec![
                DuckValue::Text("b".into()),
                DuckValue::Int(3),
                DuckValue::Text("b".into()),
            ],
        )
        .await
        .expect("INSERT..SELECT..WHERE NOT EXISTS should pass the safety gate");
    // RETURNING 子句（走查询路径，返回插入行；业务层取自增 id 依赖此能力）
    let rows = session
        .execute_duckdb_with_params(
            "INSERT INTO t_conflict (k, v) VALUES (?, ?) RETURNING k",
            vec![DuckValue::Text("c".into()), DuckValue::Int(4)],
        )
        .await
        .expect("RETURNING should pass the safety gate");
    assert_eq!(rows.len(), 1, "RETURNING should return the inserted row");

    let final_rows = session
        .execute_duckdb("SELECT v FROM t_conflict WHERE k = 'a'")
        .await
        .expect("query");
    assert_eq!(
        final_rows[0].get("v"),
        Some(&DuckValue::Int(2)),
        "the upsert should overwrite to 2"
    );
}

#[tokio::test]
async fn transaction_batch_atomic_commit_and_rollback() {
    let pool = single_conn_pool("duckdb::memory:").await;
    let session = pool.get_session("admin").await.expect("session");

    session
        .execute_duckdb_raw("CREATE TABLE t_ledger (id INTEGER, amount DOUBLE)")
        .await
        .expect("create table");

    // 提交路径：3 条语句同事务全部生效
    let results = session
        .execute_duckdb_transaction(vec![
            (
                "INSERT INTO t_ledger (id, amount) VALUES (?, ?)".to_string(),
                vec![DuckValue::Int(1), DuckValue::Double(10.0)],
            ),
            (
                "INSERT INTO t_ledger (id, amount) VALUES (?, ?)".to_string(),
                vec![DuckValue::Int(2), DuckValue::Double(20.0)],
            ),
            (
                "DELETE FROM t_ledger WHERE id = ?".to_string(),
                vec![DuckValue::Int(1)],
            ),
        ])
        .await
        .expect("transaction commit");
    assert_eq!(results.len(), 3);
    assert_eq!(results[1].rows_affected, 1);
    assert_eq!(results[2].rows_affected, 1);

    let rows = session
        .execute_duckdb("SELECT COUNT(*) AS c FROM t_ledger")
        .await
        .expect("query");
    assert_eq!(
        rows[0].get("c"),
        Some(&DuckValue::BigInt(1)),
        "1 row should remain after commit"
    );

    // 回滚路径：第 2 条语句失败（引用不存在的表）→ 第 1 条的插入被回滚
    let err = session
        .execute_duckdb_transaction(vec![
            (
                "INSERT INTO t_ledger (id, amount) VALUES (?, ?)".to_string(),
                vec![DuckValue::Int(99), DuckValue::Double(1.0)],
            ),
            (
                "DELETE FROM t_nonexistent WHERE id = ?".to_string(),
                vec![DuckValue::Int(1)],
            ),
        ])
        .await;
    assert!(err.is_err(), "a failed transaction should return Err");

    let rows = session
        .execute_duckdb("SELECT COUNT(*) AS c FROM t_ledger WHERE id = 99")
        .await
        .expect("query");
    assert_eq!(
        rows[0].get("c"),
        Some(&DuckValue::BigInt(0)),
        "id=99 should not exist after rollback"
    );
}

#[tokio::test]
async fn file_pool_single_connection_survives_sequential_sessions() {
    // 文件库 + max=1：多个 session 顺序获取/归还，不触发二次 open 文件锁冲突
    let dir = std::env::temp_dir().join(format!("dbnexus_param_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create a temporary directory");
    let db_path = dir.join("file_pool_test.duckdb");
    let url = format!("duckdb://{}", db_path.display());

    let pool = single_conn_pool(&url).await;
    for i in 0..3 {
        let session = pool.get_session("admin").await.expect("session");
        session
            .execute_duckdb_raw("CREATE TABLE IF NOT EXISTS t_seq (v INTEGER)")
            .await
            .expect("create table");
        session
            .execute_duckdb_raw_with_params(
                "INSERT INTO t_seq (v) VALUES (?)",
                vec![DuckValue::Int(i)],
            )
            .await
            .expect("insert");
        drop(session);
    }
    let session = pool.get_session("admin").await.expect("session");
    let rows = session
        .execute_duckdb("SELECT COUNT(*) AS c FROM t_seq")
        .await
        .expect("query");
    assert_eq!(rows[0].get("c"), Some(&DuckValue::BigInt(3)));
    drop(session);
    drop(pool);
    let _ = std::fs::remove_dir_all(&dir);
}
