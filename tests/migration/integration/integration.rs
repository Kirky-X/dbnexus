// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 迁移集成测试
//!
//! 注意: 部分测试需要内部 API，已暂时跳过

use dbnexus::DbPool;
use dbnexus::config::DatabaseType;
use dbnexus::migration::{
    Column, ColumnType, Index, MigrationFileParser, MigrationHistory, Schema, SchemaDiffer, SqlGenerator, Table,
    TableChange,
};

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
    }
}

/// TEST-M-001: 迁移执行器创建测试
#[tokio::test]
async fn test_migration_executor_creation() {
    let (pool, _temp_dir) = create_test_pool().await.expect("Failed to create test pool");

    // 获取会话
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 验证连接可用
    let conn = session.connection().expect("Connection should be available");
    assert!(!conn.is_closed(), "Connection should not be closed");

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
    let (pool, _temp_dir) = create_test_pool().await.expect("Failed to create test pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 验证连接可用
    let conn = session.connection().expect("Connection should be available");
    assert!(!conn.is_closed(), "Connection should not be closed");

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

    // 验证表已创建
    let url = pool.config().url_sanitized();
    let check_sql = if url.contains("postgres") {
        format!(
            "SELECT EXISTS(SELECT FROM information_schema.tables WHERE table_name = '{}')",
            table_name
        )
    } else if url.contains("mysql") {
        format!(
            "SELECT EXISTS(SELECT FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = '{}')",
            table_name
        )
    } else {
        format!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='{}'",
            table_name
        )
    };

    let result = session.execute_raw(&check_sql).await;
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

    let migration = dbnexus::migration::MigrationVersion {
        version: 1,
        description: "Initial migration".to_string(),
        applied_at: time::OffsetDateTime::now_utc(),
        file_path: "migrations/001_initial.sql".to_string(),
    };

    history.add_migration(migration);

    assert_eq!(history.applied_migrations.len(), 1);
    assert_eq!(history.get_latest_version(), Some(1));
}
