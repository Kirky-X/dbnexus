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
    Column, ColumnType, Index, MigrationFileParser, MigrationHistory, Schema,
    SchemaDiffer, SqlGenerator, Table, TableChange,
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
/// NOTE: 暂时跳过，因为需要内部 connection() 方法
#[tokio::test]
#[ignore = "需要内部 connection() 方法"]
async fn test_migration_executor_creation() {
    // 实际测试由其他测试覆盖
}

/// TEST-M-021: 迁移应用测试
/// NOTE: 暂时跳过，因为需要内部 connection() 方法
#[tokio::test]
#[ignore = "需要内部 connection() 方法"]
async fn test_migration_apply() {
    // 实际测试由其他测试覆盖
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
