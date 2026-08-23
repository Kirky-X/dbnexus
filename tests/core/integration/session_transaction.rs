// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Session 和事务集成测试
//!
//! 配置解析通过 serde 直接反序列化

// 所有 import 仅在启用数据库驱动（sqlite/postgres/mysql）时被实际使用——
// 测试函数均带 `#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]`，
// 因此此处统一加 cfg gate，避免在 `--features "permission authentication sql-parser ladybug"`
// 等无数据库驱动的特性组合下触发 unused_imports / dead_code 警告。
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
use dbnexus::DbError;
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
use dbnexus::DbPool;
#[cfg(all(
    feature = "permission",
    any(feature = "sqlite", feature = "postgres", feature = "mysql")
))]
use dbnexus::access::{PermissionAction as Operation, PermissionConfig};
#[cfg(all(
    feature = "permission",
    any(feature = "sqlite", feature = "postgres", feature = "mysql")
))]
use dbnexus::foundation::ConfigError;
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
use tempfile::TempDir;

#[path = "../../common/mod.rs"]
mod common;

/// 使用 serde_json 直接解析 JSON 配置（测试用）
#[cfg(all(
    feature = "permission",
    any(feature = "sqlite", feature = "postgres", feature = "mysql")
))]
fn parse_json_config(json: &str) -> Result<PermissionConfig, ConfigError> {
    serde_json::from_str(json).map_err(|e| ConfigError::InvalidFormat(format!("JSON deserialize error: {}", e)))
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_session_role() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    assert_eq!(session.role(), "admin");
}

#[tokio::test]
#[cfg(all(
    feature = "permission",
    any(feature = "sqlite", feature = "postgres", feature = "mysql")
))]
async fn test_session_permission_ctx() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let ctx = session.permission_ctx();
    assert_eq!(ctx.role(), "admin");
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_session_mark_write() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    session.mark_write().await;
    assert!(session.should_use_master().await);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_transaction_begin() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    assert!(!session.is_in_transaction().await);
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction().await);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_transaction_commit() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction().await);
    session.commit().await.expect("Failed to commit transaction");
    assert!(!session.is_in_transaction().await);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_transaction_rollback() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction().await);
    session.rollback().await.expect("Failed to rollback transaction");
    assert!(!session.is_in_transaction().await);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_transaction_double_begin_error() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    let result = session.begin_transaction().await;
    assert!(result.is_err());
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_transaction_commit_without_begin_error() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let result = session.commit().await;
    assert!(result.is_err());
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_transaction_rollback_without_begin_error() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let result = session.rollback().await;
    assert!(result.is_err());
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_should_use_master_in_transaction() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.should_use_master().await);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_execute_raw_ddl_admin_only() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");

    let table_name = common::generate_test_table_name("users");
    let admin_session = pool.get_session("admin").await.expect("Failed to get session");
    let ok = admin_session
        .execute_raw_ddl(&format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY, name TEXT)",
            table_name
        ))
        .await;
    assert!(ok.is_ok());

    let system_session = pool.get_session("system").await.expect("Failed to get session");
    let denied = system_session
        .execute_raw_ddl(&format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY)",
            common::generate_test_table_name("user_only")
        ))
        .await;
    assert!(matches!(denied, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_execute_raw_denies_ddl() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session
        .execute_raw("CREATE TABLE IF NOT EXISTS ddl_blocked (id INTEGER PRIMARY KEY)")
        .await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(all(
    any(feature = "sqlite", feature = "postgres", feature = "mysql"),
    not(feature = "sql-parser")
))]
async fn test_execute_raw_requires_sql_parser_feature() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session.execute_raw("SELECT 1").await;
    assert!(matches!(
        result,
        Err(DbError::Permission(msg)) if msg.contains("sql-parser")
    ));
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session.execute_raw("SELECT 1").await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 使用唯一表名避免测试间冲突
    let table_name = format!("test_users_{}", std::process::id());

    session
        .execute_raw_ddl(&format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY, name TEXT)",
            table_name
        ))
        .await
        .expect("Failed to create table");

    session
        .execute(&format!("INSERT INTO {} (id, name) VALUES (1, 'a')", table_name))
        .await
        .expect("Failed to insert");
    assert!(session.should_use_master().await);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let table_name = common::generate_test_table_name("batch_users");
    session
        .execute_raw_ddl(&format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY, name TEXT)",
            table_name
        ))
        .await
        .expect("Failed to create table");

    let insert_sql = format!("INSERT INTO {} (id, name) VALUES (1, 'a')", table_name);
    let result = session
        .batch_execute_in_transaction(vec![insert_sql.as_str(), "THIS IS NOT VALID SQL"])
        .await;
    assert!(result.is_err());
    assert!(!session.is_in_transaction().await);
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission"))]
#[allow(clippy::unwrap_used)]
async fn test_check_permission_denied_returns_permission_error() {
    let temp_dir = TempDir::new().unwrap();
    let perm_file = temp_dir.path().join("permissions.yaml");
    // 使用非 admin 角色以避免权限检查被绕过
    let perm_content = r#"{
  "roles": {
    "admin": {
      "tables": [
        {
          "name": "*",
          "operations": ["select", "insert", "update", "delete"]
        }
      ]
    },
    "test_user": {
      "tables": [
        {
          "name": "users",
          "operations": ["select"]
        }
      ]
    }
  }
}"#;
    std::fs::write(&perm_file, perm_content).unwrap();

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        admin_role: "admin".to_string(), // 明确设置 admin_role 为 "admin"
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.unwrap();
    // 使用非 admin 角色进行权限测试
    let session = pool.get_session("test_user").await.unwrap();

    let perm_config =
        parse_json_config(&std::fs::read_to_string(&perm_file).unwrap()).expect("Failed to parse permission JSON");
    session.permission_ctx().load_policy(&perm_config).await.unwrap();

    let result = session.check_permission("orders", &Operation::Select).await;
    match result {
        Ok(_) => panic!("expected permission denied"),
        // 消息经 i18n 本地化（zh-CN 下为中文措辞）+ Display 为大写，断言大小写不敏感参数
        Err(DbError::Permission(msg)) => {
            let lower = msg.to_lowercase();
            assert!(lower.contains("orders"), "should mention table: {msg}");
            assert!(lower.contains("select"), "should mention operation: {msg}");
        }
        Err(e) => panic!("unexpected error: {e}"),
    }
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "sql-parser"))]
async fn test_execute_denies_ddl() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

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
    let perm_content = r#"{
  "roles": {
    "admin": {
      "tables": [
        {
          "name": "users",
          "operations": ["select", "insert", "update", "delete"]
        }
      ]
    }
  }
}"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

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
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    session.mark_write().await;
    assert!(session.should_use_master().await);

    session.begin_transaction().await.expect("Failed to begin transaction");
    session.commit().await.expect("Failed to commit transaction");
    assert!(!session.should_use_master().await);
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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let result = session.execute("SELECT 1").await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_execute_denied_by_permission() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("permissions.yaml");
    // 使用非 admin 角色以避免权限检查被绕过
    let perm_content = r#"{
  "roles": {
    "admin": {
      "tables": [
        {
          "name": "*",
          "operations": ["select", "insert", "update", "delete"]
        }
      ]
    },
    "test_user": {
      "tables": [
        {
          "name": "users",
          "operations": ["select"]
        }
      ]
    }
  }
}"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        admin_role: "admin".to_string(), // 明确设置 admin_role 为 "admin"
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    // 使用非 admin 角色进行权限测试
    let session = pool.get_session("test_user").await.expect("Failed to get session");

    let result = session.execute("SELECT 1 FROM orders").await;
    assert!(matches!(result, Err(DbError::Permission(_))));
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission", feature = "sql-parser"))]
async fn test_execute_with_operation_denies_ddl() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

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
    let perm_content = r#"{
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
}"#;
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions file");

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .expect("Failed to create table");

    session
        .execute_with_operation("INSERT INTO users (id, name) VALUES (1, 'a')", &Operation::Insert)
        .await
        .expect("Failed to insert");
    assert!(session.should_use_master().await);
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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

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
    assert!(!session.is_in_transaction().await);
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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 使用唯一表名避免测试间冲突
    let table_name = format!("test_users_{}", std::process::id());

    session
        .execute_raw_ddl(&format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY, name TEXT)",
            table_name
        ))
        .await
        .expect("Failed to create table");

    session
        .execute(&format!("INSERT INTO {} (id, name) VALUES (1, 'a')", table_name))
        .await
        .expect("Failed to insert");
    assert!(session.should_use_master().await);
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "permission"))]
async fn test_session_trait_methods_work() {
    use dbnexus::DatabaseSession;

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

    let config = dbnexus::DbConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string()),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    let sess: &dyn DatabaseSession = &session;
    assert_eq!(sess.role(), "admin");
    assert!(!sess.is_in_transaction().await);

    sess.execute_raw_ddl("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .expect("Failed to create table");
    sess.execute("INSERT INTO users (id, name) VALUES (1, 'a')")
        .await
        .expect("Failed to insert");
    sess.execute_raw("SELECT * FROM users").await.expect("Failed to select");

    sess.begin_transaction().await.expect("Failed to begin transaction");
    assert!(sess.is_in_transaction().await);
    sess.rollback().await.expect("Failed to rollback");
    assert!(!sess.is_in_transaction().await);
}

/// v0.3.0 性能优化验证：短锁模式下事务功能正常工作
///
/// 此测试覆盖 begin_transaction → execute_raw → commit 的完整流程，
/// 验证使用 `Arc<DatabaseTransaction>` 和短锁模式后事务仍能正确执行。
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_transaction_short_lock_pattern_works() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 建表
    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS short_lock_test (id INTEGER PRIMARY KEY, val TEXT)")
        .await
        .expect("Failed to create table");
    // 幂等清理：持久库重跑时清空固定主键残留，避免 duplicate key
    session
        .execute_raw("DELETE FROM short_lock_test")
        .await
        .expect("Failed to clean table");

    // 完整事务流程：begin → execute → commit
    session.begin_transaction().await.expect("begin failed");
    assert!(session.is_in_transaction().await);

    session
        .execute_raw("INSERT INTO short_lock_test (id, val) VALUES (1, 'hello')")
        .await
        .expect("insert in transaction failed");

    session.commit().await.expect("commit failed");
    assert!(!session.is_in_transaction().await);

    // 验证数据已提交
    let result = session.execute_raw("SELECT * FROM short_lock_test").await;
    assert!(result.is_ok(), "select after commit failed");
}

/// v0.3.0 性能优化验证：事务中 execute_raw 后能继续执行（验证 Arc clone 不会阻塞后续操作）
#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_transaction_multiple_executes_in_same_transaction() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");

    session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS multi_exec_test (id INTEGER PRIMARY KEY, val TEXT)")
        .await
        .expect("Failed to create table");

    // 幂等清理：持久库重跑时清空固定主键残留，避免 duplicate key
    session
        .execute_raw("DELETE FROM multi_exec_test")
        .await
        .expect("Failed to clean table");

    session.begin_transaction().await.expect("begin failed");

    // 在同一事务中执行多次 SQL（验证 Arc clone 后锁外执行不阻塞后续）
    for i in 1..=5 {
        session
            .execute_raw(&format!(
                "INSERT INTO multi_exec_test (id, val) VALUES ({}, 'val_{}')",
                i, i
            ))
            .await
            .expect("insert in transaction failed");
    }

    session.commit().await.expect("commit failed");

    // 验证所有数据已提交
    let result = session.execute_raw("SELECT * FROM multi_exec_test").await;
    assert!(result.is_ok());
}
