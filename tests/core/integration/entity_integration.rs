// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 实体转换模块集成测试
//!
//! 测试实体类型重导出和与 sea-orm 的集成

use dbnexus::Condition;
use dbnexus::DbPool;
use dbnexus::access::permission::{PermissionAction, PermissionConfig, RolePolicy, TablePermission};
use sea_orm::ActiveValue;

/// 根据数据库 URL 判断数据库类型
fn detect_db_type_from_url(url: &str) -> &'static str {
    if url.starts_with("postgres") || url.starts_with("postgresql") {
        "postgres"
    } else if url.starts_with("mysql") {
        "mysql"
    } else {
        "sqlite"
    }
}

fn get_database_url() -> Option<String> {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        return Some(url);
    }

    if cfg!(feature = "sqlite") {
        return Some("sqlite::memory:".to_string());
    }

    if cfg!(feature = "postgres") {
        let password = std::env::var("TEST_DB_PASSWORD").unwrap_or_else(|_| "dbnexus_password".to_string());
        return Some(format!("postgres://dbnexus:{}@localhost:15433/dbnexus_test", password));
    }

    if cfg!(feature = "mysql") {
        let password = std::env::var("TEST_DB_PASSWORD").unwrap_or_else(|_| "dbnexus_password".to_string());
        return Some(format!("mysql://dbnexus:{}@localhost:13308/dbnexus_test", password));
    }

    None
}

// 注意：entity.rs 主要是类型重导出模块，实际的实体定义需要使用 dbnexus-macros
// 这里测试类型是否正确导出以及与 Session 的集成

#[tokio::test]
async fn test_entity_types_are_accessible() {
    // 测试 entity 模块导出的类型是否可访问
    // 这些类型应该可以直接使用
    let condition = Condition::all();
    let set_value = ActiveValue::Set("test".to_string());
    let set_number = ActiveValue::Set(42);
    let set_option = ActiveValue::Set(Some(42));

    assert!(!format!("{condition:?}").is_empty());
    assert!(matches!(set_value, ActiveValue::Set(_)));
    assert!(matches!(set_number, ActiveValue::Set(42)));
    assert!(matches!(set_option, ActiveValue::Set(Some(42))));
}

#[tokio::test]
async fn test_condition_operations() {
    // 测试 Condition 构建器的各种操作
    let condition = Condition::all()
        .add(Condition::any())
        .add(Condition::not(Condition::all()));

    // 测试条件组合
    let combined = Condition::all().add(Condition::all()).add(Condition::any());

    assert!(!format!("{condition:?}").is_empty());
    assert!(!format!("{combined:?}").is_empty());
}

#[tokio::test]
async fn test_set_type() {
    // 测试 Set 类型
    let string = ActiveValue::Set("test".to_string());
    let number = ActiveValue::Set(42);
    let option = ActiveValue::Set(Some(42));
    let none_value: ActiveValue<Option<i32>> = ActiveValue::Set(None);

    // 验证 Set 类型可以正确创建
    assert!(matches!(string, ActiveValue::Set(_)));
    assert!(matches!(number, ActiveValue::Set(42)));
    assert!(matches!(option, ActiveValue::Set(Some(42))));
    assert!(matches!(none_value, ActiveValue::Set(None)));
}

#[tokio::test]
async fn test_entity_with_session() {
    // 测试实体操作与 Session 的集成
    // 注意：由于权限限制，这里只测试基本的 Session 功能

    let Some(url) = get_database_url() else {
        return;
    };

    let pool = DbPool::new(&url).await.unwrap();
    let session = pool.get_session("admin").await.unwrap();

    // 验证 session 可以正常工作
    assert_eq!(session.role(), "admin");
    assert!(!session.is_in_transaction().await);

    // 测试事务功能
    session.begin_transaction().await.unwrap();
    assert!(session.is_in_transaction().await);
    session.commit().await.unwrap();
    assert!(!session.is_in_transaction().await);
}

#[tokio::test]
async fn test_entity_types_compatibility() {
    // 测试 entity 模块导出的类型与 sea-orm 的兼容性

    // Condition 应该可以用于构建查询条件
    let condition = Condition::all();

    // Set 应该可以用于设置字段值
    let set_value = ActiveValue::Set("value".to_string());

    // 这些类型应该可以正常工作
    assert!(!format!("{condition:?}").is_empty());
    assert!(matches!(set_value, ActiveValue::Set(_)));
}

#[tokio::test]
async fn test_entity_module_reexports() {
    // 测试 entity 模块是否正确重新导出了 sea-orm 的类型

    // 以下类型应该从 dbnexus::entity 可访问：
    // - EntityTrait
    // - ActiveModelTrait
    // - Condition
    // - Set

    // 如果编译通过，说明重新导出正确
    let condition = Condition::all();
    assert!(!format!("{condition:?}").is_empty());
}

#[tokio::test]
async fn test_condition_building() {
    // 测试 Condition 构建器的各种构建方式

    // 简单条件
    let simple = Condition::all();

    // 嵌套条件
    let nested = Condition::all().add(Condition::any().add(Condition::all()));

    // 否定条件
    let not_condition = Condition::not(Condition::all());

    // 组合条件
    let combined = Condition::all()
        .add(Condition::any())
        .add(Condition::not(Condition::all()));

    assert!(!format!("{simple:?}").is_empty());
    assert!(!format!("{nested:?}").is_empty());
    assert!(!format!("{not_condition:?}").is_empty());
    assert!(!format!("{combined:?}").is_empty());
}

#[tokio::test]
async fn test_set_with_various_types() {
    // 测试 Set 类型与各种类型的兼容性

    let string = ActiveValue::Set("string".to_string());
    let i32_value = ActiveValue::Set(42);
    let i64_value = ActiveValue::Set(100_i64);
    let f64_value = ActiveValue::Set(1.5);
    let bool_value = ActiveValue::Set(true);
    let option_value: ActiveValue<Option<i32>> = ActiveValue::Set(None);

    assert!(matches!(string, ActiveValue::Set(_)));
    assert!(matches!(i32_value, ActiveValue::Set(42)));
    assert!(matches!(i64_value, ActiveValue::Set(100)));
    assert!(matches!(f64_value, ActiveValue::Set(_)));
    assert!(matches!(bool_value, ActiveValue::Set(true)));
    assert!(matches!(option_value, ActiveValue::Set(None)));
}

#[tokio::test]
async fn test_entity_integration_with_sql_operations() {
    // 测试实体操作与 SQL 操作的集成
    // 注意：由于权限限制，这里只测试基本的 Session 功能

    let Some(url) = get_database_url() else {
        return;
    };

    let pool = DbPool::new(&url).await.unwrap();
    let session = pool.get_session("admin").await.unwrap();

    // 验证 session 可以正常工作
    assert_eq!(session.role(), "admin");
    assert!(!session.is_in_transaction().await);

    // 测试事务功能
    session.begin_transaction().await.unwrap();
    assert!(session.is_in_transaction().await);
    session.rollback().await.unwrap();
    assert!(!session.is_in_transaction().await);
}

// ============================================================================
// 完整 CRUD 操作集成测试（使用权限配置）
// ============================================================================

const CRUD_TEST_TABLE: &str = "crud_entity_test";

/// 获取包含完整权限的测试配置
///
/// 返回 (DbConfig, TempDir)：TempDir 必须在测试期间保持存活，否则权限文件和数据库会被自动清理。
/// 使用独立临时目录避免并行测试间的文件竞争。
///
/// 注意：使用文件型 SQLite 而非 `sqlite::memory:`，因为 sea-orm 的 `Database::connect()`
/// 内部维护自己的连接池，每个内部连接到 `:memory:` 会创建独立的内存数据库，导致
/// `CREATE TABLE` 在连接 A 上执行后，`INSERT` 在连接 B 上执行时找不到表。
fn get_test_config_with_all_permissions() -> (dbnexus::DbConfig, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let db_path = dir.path().join("test.db");
        format!("sqlite://{}?mode=rwc", db_path.display())
    });

    let perm_content = r#"
roles:
  admin:
    tables:
      - name: "*"
        operations:
          - SELECT
          - INSERT
          - UPDATE
          - DELETE
"#;

    let perm_file = dir.path().join("perms.yaml");
    std::fs::write(&perm_file, perm_content).expect("Failed to write permissions");

    let config = dbnexus::DbConfig {
        url,
        max_connections: 5,
        admin_role: "admin".to_string(),
        permissions_path: Some(perm_file.to_string_lossy().to_string()),
        ..Default::default()
    };

    (config, dir)
}

/// 创建测试表
async fn setup_crud_test_table(pool: &DbPool) {
    let session = pool.get_session("admin").await.unwrap();

    // 先删除表（如果存在），以确保使用正确的结构
    let _ = session
        .execute_raw_ddl(&format!("DROP TABLE IF EXISTS {}", CRUD_TEST_TABLE))
        .await;

    // 使用运行时检测而非编译时 cfg! 来决定 SQL 语法
    let db_type = detect_db_type_from_url(pool.config().url.as_str());
    let create_sql = match db_type {
        "mysql" => {
            eprintln!("Using MySQL CREATE TABLE syntax");
            format!(
                "CREATE TABLE IF NOT EXISTS {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255), email VARCHAR(255), age INT)",
                CRUD_TEST_TABLE
            )
        }
        "postgres" => {
            eprintln!("Using PostgreSQL CREATE TABLE syntax");
            format!(
                "CREATE TABLE IF NOT EXISTS {} (id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY, name TEXT, email TEXT, age INTEGER)",
                CRUD_TEST_TABLE
            )
        }
        _ => {
            eprintln!("Using SQLite CREATE TABLE syntax");
            format!(
                "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY, name TEXT, email TEXT, age INTEGER)",
                CRUD_TEST_TABLE
            )
        }
    };

    eprintln!("CREATE SQL: {}", create_sql);
    session.execute_raw_ddl(&create_sql).await.unwrap();
}

/// 清理测试表
async fn cleanup_crud_test_table(pool: &DbPool) {
    let session = pool.get_session("admin").await.unwrap();
    let _ = session
        .execute_raw_ddl(&format!("DROP TABLE IF EXISTS {}", CRUD_TEST_TABLE))
        .await;
}

#[tokio::test]
async fn test_entity_crud_full_operations() {
    let (config, _perm_dir) = get_test_config_with_all_permissions();
    let pool = DbPool::with_config(config).await.unwrap();
    setup_crud_test_table(&pool).await;

    // Get session and load permissions BEFORE using it
    let session = pool.get_session("admin").await.unwrap();

    // Load permissions into this session's context
    let perm_config = PermissionConfig {
        roles: [(
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
        )]
        .into_iter()
        .collect(),
    };

    session
        .permission_ctx()
        .load_policy(&perm_config)
        .await
        .expect("Failed to load permissions");

    // CRITICAL: Use the SAME session that has permissions loaded
    // Do NOT get a new session after loading permissions!

    // CREATE
    let create = session
        .execute_raw(&format!(
            "INSERT INTO {} (name, email, age) VALUES ('CRUD Test', 'crud@test.com', 25)",
            CRUD_TEST_TABLE
        ))
        .await;
    assert!(create.is_ok(), "INSERT failed: {:?}", create.err());

    // READ
    let read = session
        .execute_raw(&format!("SELECT * FROM {} WHERE name = 'CRUD Test'", CRUD_TEST_TABLE))
        .await;
    assert!(read.is_ok(), "SELECT failed: {:?}", read.err());

    // UPDATE
    let update = session
        .execute_raw(&format!(
            "UPDATE {} SET age = 30 WHERE name = 'CRUD Test'",
            CRUD_TEST_TABLE
        ))
        .await;
    assert!(update.is_ok(), "UPDATE failed: {:?}", update.err());

    // DELETE
    let delete = session
        .execute_raw(&format!("DELETE FROM {} WHERE name = 'CRUD Test'", CRUD_TEST_TABLE))
        .await;
    assert!(delete.is_ok(), "DELETE failed: {:?}", delete.err());

    cleanup_crud_test_table(&pool).await;
}

#[tokio::test]
async fn test_entity_transaction_with_permissions() {
    let (config, _perm_dir) = get_test_config_with_all_permissions();
    let pool = DbPool::with_config(config).await.unwrap();
    setup_crud_test_table(&pool).await;

    // Get session and load permissions BEFORE using it
    let session = pool.get_session("admin").await.unwrap();

    // Load permissions into this session's context
    let perm_config = PermissionConfig {
        roles: [(
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
        )]
        .into_iter()
        .collect(),
    };

    session
        .permission_ctx()
        .load_policy(&perm_config)
        .await
        .expect("Failed to load permissions");

    // CRITICAL: Use the SAME session that has permissions loaded

    // Commit test
    session.begin_transaction().await.unwrap();
    session
        .execute_raw(&format!(
            "INSERT INTO {} (name, email) VALUES ('TX Commit', 'tx@test.com')",
            CRUD_TEST_TABLE
        ))
        .await
        .expect("INSERT in transaction failed");
    session.commit().await.unwrap();

    let after_commit = session
        .execute_raw(&format!("SELECT COUNT(*) FROM {}", CRUD_TEST_TABLE))
        .await;
    assert!(after_commit.is_ok(), "SELECT COUNT failed: {:?}", after_commit.err());

    // Rollback test
    session.begin_transaction().await.unwrap();
    session
        .execute_raw(&format!(
            "INSERT INTO {} (name, email) VALUES ('TX Rollback', 'rollback@test.com')",
            CRUD_TEST_TABLE
        ))
        .await
        .expect("INSERT before rollback failed");
    session.rollback().await.unwrap();

    cleanup_crud_test_table(&pool).await;
}
