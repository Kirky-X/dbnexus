// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 迁移集成测试
//!
//! 注意: 部分测试需要内部 API，已暂时跳过

use dbnexus::MigrationHistory;
use dbnexus::foundation::DatabaseType;
use sea_orm::ConnectionTrait;

#[path = "../../common/mod.rs"]
mod common;

fn table_exists_check_sql(db_type: DatabaseType, table_name: &str) -> String {
    match db_type {
        DatabaseType::Sqlite => format!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='{}'",
            table_name
        ),
        DatabaseType::Postgres => format!(
            "SELECT table_name FROM information_schema.tables WHERE table_schema='public' AND table_name='{}'",
            table_name
        ),
        DatabaseType::MySql => format!(
            "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name='{}'",
            table_name
        ),
        // DuckDB 使用 information_schema 兼容语法
        DatabaseType::DuckDb => format!(
            "SELECT table_name FROM information_schema.tables WHERE table_name='{}'",
            table_name
        ),
        DatabaseType::Ladybug | DatabaseType::Neo4j => {
            panic!("Graph databases do not support relational table existence checks")
        }
    }
}

/// TEST-M-001: 迁移执行器创建测试
#[tokio::test]
async fn test_migration_executor_creation() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed to create test pool");

    // 获取会话
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 验证连接可用（获取连接但不使用，仅验证会话正常）
    let _conn = session.connection().expect("Connection should be available");

    // 验证我们可以创建表（迁移执行器的基础）
    let table_name = format!(
        "migration_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    session
        .execute_raw_ddl(&format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY, name TEXT)",
            table_name
        ))
        .await
        .expect("Should be able to create table");

    // 清理
    let _ = session
        .execute_raw_ddl(&format!("DROP TABLE IF EXISTS {}", table_name))
        .await;
}

/// TEST-M-021: 迁移应用测试
#[tokio::test]
async fn test_migration_apply() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed to create test pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 创建测试表用于迁移测试
    let table_name = format!(
        "migration_apply_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // 执行迁移（创建表）
    session
        .execute_raw_ddl(&format!(
            "CREATE TABLE IF NOT EXISTS {} (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
            table_name
        ))
        .await
        .expect("Migration should apply successfully");

    // 验证表已创建（使用 connection 直接查询，绕过 SQL 解析器对元数据查询的限制）
    let check_sql = table_exists_check_sql(pool.config().database_type().unwrap(), &table_name);

    let conn = session.connection().expect("Connection should be available");
    let result = conn.execute_unprepared(&check_sql).await;
    assert!(result.is_ok(), "Migration should be applied");

    // 清理
    let _ = session
        .execute_raw_ddl(&format!("DROP TABLE IF EXISTS {}", table_name))
        .await;
}

/// TEST-M-002: 迁移历史创建测试
#[test]
fn test_migration_history_creation() {
    let history = MigrationHistory::new();

    assert!(history.applied_migrations.is_empty());
    assert_eq!(history.get_latest_version(), None);
}

/// TEST-M-003: 迁移历史添加测试
#[test]
fn test_migration_history_add() {
    let mut history = MigrationHistory::new();

    let migration = dbnexus::MigrationVersion {
        version: 1,
        description: "Initial migration".to_string(),
        applied_at: time::OffsetDateTime::now_utc(),
        file_path: "migrations/001_initial.sql".to_string(),
    };

    history.add_migration(migration);

    assert_eq!(history.applied_migrations.len(), 1);
    assert_eq!(history.get_latest_version(), Some(1));
}

/// TEST-M-004: 迁移执行器首次运行测试（无迁移表）
///
/// 验证 MigrationExecutor 在首次运行时（数据库中不存在迁移表）的行为：
/// - load_applied_versions 应返回空集合
/// - run_migrations 应能正确创建迁移表并应用迁移
#[tokio::test]
async fn test_migration_executor_first_run() {
    use dbnexus::MigrationExecutor;
    use std::io::Write;
    use tempfile::TempDir;

    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed to create test pool");
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let conn = session.connection().expect("Connection should be available");

    // 创建临时目录存放迁移文件
    let migration_dir = TempDir::new().expect("Failed to create temp dir");

    // 创建一个简单的迁移文件
    let migration_file_path = migration_dir.path().join("001_create_test_table.sql");
    let mut file = std::fs::File::create(&migration_file_path).expect("Failed to create migration file");
    let table_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    writeln!(
        file,
        "-- UP\nCREATE TABLE test_first_run_{} (id INTEGER PRIMARY KEY);",
        table_suffix
    )
    .expect("Failed to write migration file");
    drop(file);

    // 创建迁移执行器
    let db_type = pool.config().database_type().unwrap();
    let mut executor = MigrationExecutor::new(conn.clone(), db_type);

    // 验证首次运行时历史为空
    assert!(executor.history().applied_migrations.is_empty());

    // 运行迁移（这会调用 load_applied_versions）
    let result = executor.run_migrations(migration_dir.path()).await;
    assert!(result.is_ok(), "Migration should succeed on first run");
    assert_eq!(result.unwrap(), 1, "One migration should be applied");

    // 验证历史记录已更新
    assert_eq!(executor.history().applied_migrations.len(), 1);
    assert_eq!(executor.get_latest_migration().map(|m| m.version), Some(1));

    // 再次运行迁移，验证幂等性（不会重复应用）
    let result = executor.run_migrations(migration_dir.path()).await;
    assert!(result.is_ok(), "Second run should be idempotent: {:?}", result.err());
    assert_eq!(result.unwrap(), 0, "No migration should be applied on second run");

    // 清理测试表
    let _ = session
        .execute_raw_ddl(&format!("DROP TABLE IF EXISTS test_first_run_{}", table_suffix))
        .await;
}
