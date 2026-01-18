// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Session 和事务集成测试

use dbnexus::DbError;
use dbnexus::DbPool;
use dbnexus::config::DbConfigBuilder;
#[cfg(feature = "permission")]
use dbnexus::permission::{PermissionAction as Operation, PermissionConfig};
use tempfile::TempDir;

#[path = "../../common/mod.rs"]
mod common;

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_session_role() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    assert_eq!(session.role(), "admin");
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_session_permission_ctx() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let ctx = session.permission_ctx();
    assert_eq!(ctx.role(), "admin");
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_session_mark_write() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    session.mark_write();
    assert!(session.should_use_master());
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_transaction_begin() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    assert!(!session.is_in_transaction());
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction());
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_transaction_commit() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction());
    session.commit().await.expect("Failed to commit transaction");
    assert!(!session.is_in_transaction());
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_transaction_rollback() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction());
    session.rollback().await.expect("Failed to rollback transaction");
    assert!(!session.is_in_transaction());
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_transaction_double_begin_error() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    let result = session.begin_transaction().await;
    assert!(result.is_err());
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_transaction_commit_without_begin_error() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    let result = session.commit().await;
    assert!(result.is_err());
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_transaction_rollback_without_begin_error() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    let result = session.rollback().await;
    assert!(result.is_err());
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_should_use_master_in_transaction() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.should_use_master());
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_execute_raw_ddl_admin_only() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");

    let admin_session = pool.get_session("admin").await.expect("Failed to get session");
    let ok = admin_session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await;
    assert!(ok.is_ok());

    let system_session = pool.get_session("system").await.expect("Failed to get session");
    let denied = system_session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS user_only (id INTEGER PRIMARY KEY)")
        .await;
    assert!(matches!(denied, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_execute_raw_denies_ddl() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session
        .execute_raw("CREATE TABLE IF NOT EXISTS ddl_blocked (id INTEGER PRIMARY KEY)")
        .await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_execute_raw_denies_when_sql_parse_fails() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session.execute_raw("SELECT 1").await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_execute_insert_marks_write() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .expect("Failed to create table");

    session
        .execute("INSERT INTO users (id, name) VALUES (1, 'a')")
        .await
        .expect("Failed to insert");
    assert!(session.should_use_master());
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_batch_execute_in_transaction_rolls_back_on_error() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .expect("Failed to create table");

    let result = session
        .batch_execute_in_transaction(vec![
            "INSERT INTO users (id, name) VALUES (1, 'a')",
            "THIS IS NOT VALID SQL",
        ])
        .await;
    assert!(result.is_err());
    assert!(!session.is_in_transaction());
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission"))]
#[allow(clippy::unwrap_used)]
async fn test_check_permission_denied_returns_permission_error() {
    let temp_dir = TempDir::new().unwrap();
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "users"
        operations:
          - select
"#;
    std::fs::write(&perm_file, perm_content).unwrap();

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.unwrap();
    let session = pool.get_session("admin").await.unwrap();

    let perm_config = PermissionConfig::from_yaml(&std::fs::read_to_string(&perm_file).unwrap()).unwrap();
    session.permission_ctx().load_policy(&perm_config).await.unwrap();

    let result = session.check_permission("orders", &Operation::Select).await;
    match result {
        Ok(_) => panic!("expected permission denied"),
        Err(DbError::Permission(msg)) => assert!(msg.contains("Permission denied")),
        Err(e) => panic!("unexpected error: {e}"),
    }
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "sql-parser"))]
async fn test_execute_denies_ddl() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session
        .execute("CREATE TABLE IF NOT EXISTS ddl_blocked_2 (id INTEGER PRIMARY KEY)")
        .await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission"))]
async fn test_execute_with_operation_denied_by_permission() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "users"
        operations:
          - select
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session
        .execute_with_operation("SELECT 1 FROM orders", &Operation::Select)
        .await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission"))]
async fn test_execute_with_operation_allows_when_permitted() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "users"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY)")
        .await
        .expect("Failed to create table");

    let result = session
        .execute_with_operation("SELECT * FROM users", &Operation::Select)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission"))]
async fn test_commit_clears_last_write() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    session.mark_write();
    assert!(session.should_use_master());

    session.begin_transaction().await.expect("Failed to begin transaction");
    session.commit().await.expect("Failed to commit transaction");
    assert!(!session.should_use_master());
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_execute_denies_when_no_table_in_statement() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session.execute("SELECT 1").await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_execute_denied_by_permission() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "users"
        operations:
          - select
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session.execute("SELECT 1 FROM orders").await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_execute_with_operation_denies_ddl() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session
        .execute_with_operation(
            "CREATE TABLE IF NOT EXISTS ddl_blocked_3 (id INTEGER PRIMARY KEY)",
            &Operation::Select,
        )
        .await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_execute_with_operation_insert_marks_write() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .expect("Failed to create table");

    session
        .execute_with_operation("INSERT INTO users (id, name) VALUES (1, 'a')", &Operation::Insert)
        .await
        .expect("Failed to insert");
    assert!(session.should_use_master());
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_batch_execute_collects_results() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .expect("Failed to create table");

    let results = session
        .batch_execute(vec![
            "INSERT INTO users (id, name) VALUES (1, 'a')",
            "INSERT INTO users (id, name) VALUES (2, 'b')",
        ])
        .await
        .expect("Failed to batch execute");
    assert_eq!(results.len(), 2);
    assert!(results.into_iter().all(|r| r.is_ok()));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_batch_execute_in_transaction_commits_on_success() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .expect("Failed to create table");

    let results = session
        .batch_execute_in_transaction(vec![
            "INSERT INTO users (id, name) VALUES (1, 'a')",
            "INSERT INTO users (id, name) VALUES (2, 'b')",
        ])
        .await
        .expect("Expected batch transaction to succeed");
    assert_eq!(results.len(), 2);
    assert!(!session.is_in_transaction());
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_execute_raw_in_transaction_rolls_back_writes() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .expect("Failed to create table");

    session.begin_transaction().await.expect("Failed to begin");
    session
        .execute_raw("INSERT INTO users (id, name) VALUES (1, 'a')")
        .await
        .expect("Failed to insert in tx");
    session.rollback().await.expect("Failed to rollback");

    session
        .execute("INSERT INTO users (id, name) VALUES (1, 'a')")
        .await
        .expect("Expected insert to succeed after rollback");
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_execute_raw_rejects_effectively_empty_table_name() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session.execute_raw("SELECT 1 FROM \"\"").await;
    assert!(matches!(result, Err(DbError::Permission(msg)) if msg.contains("Failed to extract table name")));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_execute_rejects_effectively_empty_table_name() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session.execute("SELECT 1 FROM \"\"").await;
    assert!(matches!(result, Err(DbError::Permission(msg)) if msg.contains("Failed to extract table name")));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission"))]
async fn test_execute_with_operation_denied_by_permission_uses_extracted_table_name() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "users"
        operations:
          - select
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session
        .execute_with_operation("SELECT 1 FROM orders ", &Operation::Select)
        .await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission"))]
async fn test_execute_with_operation_update_marks_write() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .expect("Failed to create table");

    // INSERT 是 DML 操作，应使用 execute_raw 而非 execute_raw_ddl
    session
        .execute_raw("INSERT INTO users (id, name) VALUES (1, 'a')")
        .await
        .expect("Failed to insert");

    session
        .execute_with_operation("UPDATE users SET name = 'b' WHERE id = 1", &Operation::Update)
        .await
        .expect("Failed to update");
    assert!(session.should_use_master());
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission"))]
async fn test_session_trait_methods_work() {
    use dbnexus::pool::DatabaseSession;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .expect("Failed to build config");
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let mut session = pool.get_session("admin").await.expect("Failed to get session");

    let sess: &mut dyn DatabaseSession = &mut session;
    assert_eq!(sess.role(), "admin");
    assert!(!sess.is_in_transaction());

    sess.execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .expect("Failed to create table");
    sess.execute("INSERT INTO users (id, name) VALUES (1, 'a')")
        .await
        .expect("Failed to insert");
    sess.execute_raw("SELECT * FROM users").await.expect("Failed to select");

    sess.begin_transaction().await.expect("Failed to begin transaction");
    assert!(sess.is_in_transaction());
    sess.rollback().await.expect("Failed to rollback");
    assert!(!sess.is_in_transaction());
}
