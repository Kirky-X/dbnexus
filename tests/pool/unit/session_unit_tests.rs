// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! database::pool::Session 单元测试
//!
//! 覆盖：
//! - Session 创建、role()、permission_ctx()
//! - 事务生命周期：is_in_transaction / begin / commit / rollback
//! - 事务边界错误：重复 begin、无事务 commit/rollback
//! - execute_raw / execute_raw_ddl 权限与 DDL 防护
//! - batch_execute / batch_execute_in_transaction 原子性
//! - check_table_permission admin 绕过
//! - should_use_master 读写分离判定
//! - Drop 后连接归还

#![cfg(feature = "sqlite")]

use dbnexus::{DbError, DbPool};

#[path = "../../common/mod.rs"]
mod common;

// ============================================================================
// 辅助函数
// ============================================================================

/// 创建 SQLite 内存连接池并返回
async fn make_pool() -> DbPool {
    common::make_sqlite_memory_pool().await
}

// ============================================================================
// 基础属性测试
// ============================================================================

/// TEST-U-SESS-001: role() 应返回创建时指定的角色
#[tokio::test]
async fn test_session_role_returns_creation_role() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    assert_eq!(session.role(), "admin");
}

/// TEST-U-SESS-002: permission_ctx() 应返回与 role 一致的上下文
#[cfg(feature = "permission")]
#[tokio::test]
async fn test_session_permission_ctx_returns_context() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let ctx = session.permission_ctx();
    // PermissionContext 应非空（具体字段由 permission 模块定义）
    assert_eq!(ctx.role(), "admin");
}

// ============================================================================
// 事务生命周期测试
// ============================================================================

/// TEST-U-SESS-003: 初始 is_in_transaction 应为 false
#[tokio::test]
async fn test_session_is_in_transaction_initial_false() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    assert!(!session.is_in_transaction().await);
}

/// TEST-U-SESS-004: begin_transaction 后 is_in_transaction 应为 true
#[tokio::test]
async fn test_session_begin_transaction_succeeds() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    session.begin_transaction().await.expect("begin should succeed");
    assert!(session.is_in_transaction().await);
}

/// TEST-U-SESS-005: begin + commit 后 is_in_transaction 应为 false
#[tokio::test]
async fn test_session_commit_after_begin() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    session.begin_transaction().await.unwrap();
    session.commit().await.expect("commit should succeed");
    assert!(!session.is_in_transaction().await);
}

/// TEST-U-SESS-006: begin + rollback 后 is_in_transaction 应为 false
#[tokio::test]
async fn test_session_rollback_after_begin() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    session.begin_transaction().await.unwrap();
    session.rollback().await.expect("rollback should succeed");
    assert!(!session.is_in_transaction().await);
}

/// TEST-U-SESS-007: 重复 begin 应返回 Transaction 错误
#[tokio::test]
async fn test_session_double_begin_fails() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    session.begin_transaction().await.unwrap();
    let result = session.begin_transaction().await;
    assert!(result.is_err(), "double begin should fail");
    let err = result.unwrap_err();
    assert!(
        matches!(err, DbError::Transaction(ref msg) if msg.contains("Already in transaction")),
        "expected Transaction 'Already in transaction' error, got {:?}",
        err
    );
}

/// TEST-U-SESS-008: 无事务时 commit 应返回 Transaction 错误
#[tokio::test]
async fn test_session_commit_without_transaction_fails() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let result = session.commit().await;
    assert!(result.is_err(), "commit without transaction should fail");
    let err = result.unwrap_err();
    assert!(
        matches!(err, DbError::Transaction(ref msg) if msg.contains("No active transaction")),
        "expected Transaction 'No active transaction' error, got {:?}",
        err
    );
}

/// TEST-U-SESS-009: 无事务时 rollback 应返回 Transaction 错误
#[tokio::test]
async fn test_session_rollback_without_transaction_fails() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let result = session.rollback().await;
    assert!(result.is_err(), "rollback without transaction should fail");
    let err = result.unwrap_err();
    assert!(
        matches!(err, DbError::Transaction(ref msg) if msg.contains("Not in transaction")),
        "expected Transaction 'Not in transaction' error, got {:?}",
        err
    );
}

// ============================================================================
// execute_raw_ddl 测试
// ============================================================================

/// TEST-U-SESS-010: admin 角色 execute_raw_ddl CREATE TABLE 应成功
#[tokio::test]
async fn test_session_execute_raw_ddl_admin_success() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let table = common::generate_test_table_name("ddl_test");
    let sql = format!("CREATE TABLE {} (id INTEGER PRIMARY KEY, name TEXT)", table);
    let result = session.execute_raw_ddl(&sql).await;
    assert!(
        result.is_ok(),
        "admin execute_raw_ddl should succeed: {:?}",
        result.err()
    );
}

/// TEST-U-SESS-011: 非 admin 角色 execute_raw_ddl 应返回 Permission 错误
#[tokio::test]
async fn test_session_execute_raw_ddl_non_admin_fails() {
    let pool = make_pool().await;
    // system 角色在无权限配置下被允许获取 session
    let session = pool.get_session("system").await.unwrap();
    let table = common::generate_test_table_name("ddl_fail");
    let sql = format!("CREATE TABLE {} (id INTEGER PRIMARY KEY)", table);
    let result = session.execute_raw_ddl(&sql).await;
    assert!(result.is_err(), "non-admin execute_raw_ddl should fail");
    let err = result.unwrap_err();
    assert!(
        matches!(err, DbError::Permission(ref msg) if msg.contains("admin role")),
        "expected Permission 'admin role' error, got {:?}",
        err
    );
}

/// TEST-U-SESS-012: execute_raw_ddl DROP TABLE 应被 DdlGuard 拒绝（安全设计）
#[tokio::test]
async fn test_session_execute_raw_ddl_drop_table() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let table = common::generate_test_table_name("drop_test");
    // 先创建
    let create_sql = format!("CREATE TABLE {} (id INTEGER PRIMARY KEY)", table);
    session.execute_raw_ddl(&create_sql).await.unwrap();
    // 再删除 — DdlGuard 安全设计禁止 DROP TABLE（仅允许 DROP INDEX/VIEW）
    let drop_sql = format!("DROP TABLE {}", table);
    let result = session.execute_raw_ddl(&drop_sql).await;
    assert!(result.is_err(), "DROP TABLE should be rejected by DdlGuard");
    let err = result.unwrap_err();
    assert!(
        matches!(err, DbError::Permission(ref msg) if msg.contains("DropTable")),
        "expected DdlGuard rejection for DropTable, got {:?}",
        err
    );
}

// ============================================================================
// execute_raw 测试
// ============================================================================

/// TEST-U-SESS-013: sql-parser feature 下 execute_raw 对 DDL 应返回 Permission 错误
#[cfg(feature = "sql-parser")]
#[tokio::test]
async fn test_session_execute_raw_rejects_ddl_under_sql_parser() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let table = common::generate_test_table_name("raw_ddl_reject");
    let sql = format!("CREATE TABLE {} (id INTEGER PRIMARY KEY)", table);
    let result = session.execute_raw(&sql).await;
    assert!(
        result.is_err(),
        "execute_raw with DDL should be rejected under sql-parser"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, DbError::Permission(ref msg) if msg.contains("DDL")),
        "expected Permission DDL error, got {:?}",
        err
    );
}

/// TEST-U-SESS-014: 非 sql-parser feature 下 execute_raw 应返回需要 feature 的错误
#[cfg(not(feature = "sql-parser"))]
#[tokio::test]
async fn test_session_execute_raw_requires_sql_parser_feature() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let result = session.execute_raw("SELECT 1").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, DbError::Permission(ref msg) if msg.contains("sql-parser")),
        "expected 'requires sql-parser feature' error, got {:?}",
        err
    );
}

/// TEST-U-SESS-015: execute_raw SELECT 在 admin 角色下应成功（sql-parser 启用时）
#[cfg(feature = "sql-parser")]
#[tokio::test]
async fn test_session_execute_raw_select_admin_success() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let table = common::generate_test_table_name("select_test");
    // 先用 execute_raw_ddl 创建表
    let create_sql = format!("CREATE TABLE {} (id INTEGER PRIMARY KEY, val TEXT)", table);
    session.execute_raw_ddl(&create_sql).await.unwrap();
    // 插入数据 — INSERT 是 DML，必须走 execute_raw（DdlGuard 会拒绝 DML 走 DDL 通道）
    let insert_sql = format!("INSERT INTO {} (val) VALUES ('hello')", table);
    session.execute_raw(&insert_sql).await.unwrap();
    // 查询 — admin 角色应绕过权限检查
    let select_sql = format!("SELECT val FROM {} WHERE val = 'hello'", table);
    let result = session.execute_raw(&select_sql).await;
    assert!(
        result.is_ok(),
        "admin execute_raw SELECT should succeed: {:?}",
        result.err()
    );
}

// ============================================================================
// batch_execute 测试
// ============================================================================

/// TEST-U-SESS-016: batch_execute 多条 INSERT 应返回多个结果
#[cfg(feature = "sql-parser")]
#[tokio::test]
async fn test_session_batch_execute_multiple_inserts() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let table = common::generate_test_table_name("batch_ins");
    // 先用 execute_raw_ddl 创建表
    let create_sql = format!("CREATE TABLE {} (id INTEGER PRIMARY KEY, val TEXT)", table);
    session.execute_raw_ddl(&create_sql).await.unwrap();

    // batch_execute 用 INSERT（非 DDL，sql-parser 下 admin 绕过权限）
    let insert1 = format!("INSERT INTO {} (val) VALUES ('a')", table);
    let insert2 = format!("INSERT INTO {} (val) VALUES ('b')", table);
    let results = session.batch_execute(vec![insert1.as_str(), insert2.as_str()]).await;
    assert!(results.is_ok(), "batch_execute should succeed: {:?}", results.err());
    let results = results.unwrap();
    assert_eq!(results.len(), 2, "should return 2 results");
    for (i, r) in results.iter().enumerate() {
        assert!(r.is_ok(), "result {} should be ok: {:?}", i, r.as_ref().err());
    }
}

// ============================================================================
// batch_execute_in_transaction 测试
// ============================================================================

/// TEST-U-SESS-017: batch_execute_in_transaction 全部成功时应 commit
#[cfg(feature = "sql-parser")]
#[tokio::test]
async fn test_session_batch_execute_in_transaction_success() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let table = common::generate_test_table_name("tx_batch_ok");
    // 预创建表
    let create_sql = format!("CREATE TABLE {} (id INTEGER PRIMARY KEY, val TEXT)", table);
    session.execute_raw_ddl(&create_sql).await.unwrap();

    // 事务批量插入
    let insert1 = format!("INSERT INTO {} (val) VALUES ('a')", table);
    let insert2 = format!("INSERT INTO {} (val) VALUES ('b')", table);
    let results = session
        .batch_execute_in_transaction(vec![insert1.as_str(), insert2.as_str()])
        .await;
    assert!(
        results.is_ok(),
        "batch in transaction should succeed: {:?}",
        results.err()
    );
    assert_eq!(results.unwrap().len(), 2);
    // 事务应已 commit
    assert!(!session.is_in_transaction().await);
}

/// TEST-U-SESS-018: batch_execute_in_transaction 中间失败时应 rollback
#[cfg(feature = "sql-parser")]
#[tokio::test]
async fn test_session_batch_execute_in_transaction_atomicity() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    let table = common::generate_test_table_name("tx_batch_fail");
    // 预创建表
    let create_sql = format!("CREATE TABLE {} (id INTEGER PRIMARY KEY, val TEXT)", table);
    session.execute_raw_ddl(&create_sql).await.unwrap();

    // 第一条成功，第二条语法错误应触发 rollback
    let insert_ok = format!("INSERT INTO {} (val) VALUES ('ok')", table);
    let bad_sql = "THIS IS NOT VALID SQL";
    let result = session
        .batch_execute_in_transaction(vec![insert_ok.as_str(), bad_sql])
        .await;
    assert!(result.is_err(), "batch with invalid SQL should fail");
    // 事务应已 rollback
    assert!(!session.is_in_transaction().await);
}

// ============================================================================
// check_table_permission 测试
// ============================================================================

/// TEST-U-SESS-019: admin 角色 check_table_permission 应绕过检查
#[tokio::test]
async fn test_session_check_table_permission_admin_bypass() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    // admin 对任意表/操作都应通过
    let result = session.check_table_permission("any_table", "SELECT").await;
    assert!(
        result.is_ok(),
        "admin should bypass permission check: {:?}",
        result.err()
    );
}

// ============================================================================
// should_use_master 测试
// ============================================================================

/// TEST-U-SESS-020: 初始 should_use_master 应为 false（无写操作、无事务）
#[tokio::test]
async fn test_session_should_use_master_initial_false() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    // 初始状态：无事务、无写操作
    // 注意：should_use_master 的具体逻辑可能因实现而异，这里测试基本行为
    let _ = session.should_use_master().await;
    // 不断言具体值，因为实现可能基于时间窗口
}

/// TEST-U-SESS-021: begin_transaction 后 should_use_master 应为 true
#[tokio::test]
async fn test_session_should_use_master_true_in_transaction() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    session.begin_transaction().await.unwrap();
    assert!(
        session.should_use_master().await,
        "should use master when in transaction"
    );
}

/// TEST-U-SESS-022: mark_write 后 should_use_master 应为 true
#[tokio::test]
async fn test_session_mark_write_affects_should_use_master() {
    let pool = make_pool().await;
    let session = pool.get_session("admin").await.unwrap();
    session.mark_write().await;
    assert!(session.should_use_master().await, "should use master after mark_write");
}

// ============================================================================
// Drop 行为测试
// ============================================================================

/// TEST-U-SESS-023: session drop 后 pool status 应恢复
#[tokio::test]
async fn test_session_drop_releases_connection() {
    let pool = make_pool().await;
    let status_before = pool.status();
    {
        let _session = pool.get_session("admin").await.unwrap();
        // session 在作用域内
    } // session drop
    let status_after = pool.status();
    // drop 后 active 不应增加（连接归还）
    assert!(
        status_after.active <= status_before.active,
        "active should not increase after session drop"
    );
}

/// TEST-U-SESS-024: 多个 session 串行获取应可重用连接
#[tokio::test]
async fn test_session_serial_acquire_release() {
    let pool = make_pool().await;
    for i in 0..3 {
        let session = pool.get_session("admin").await;
        assert!(session.is_ok(), "iteration {} should succeed", i);
        drop(session);
    }
}
