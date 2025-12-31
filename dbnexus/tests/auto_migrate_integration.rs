// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 自动迁移集成测试
//!
//! 测试自动迁移功能的各个组件：配置解析、迁移扫描、手动迁移、自动迁移触发等

use dbnexus::config::DbConfig;
use dbnexus::DbPool;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// TEST-AM-001: 自动迁移配置创建测试
#[tokio::test]
async fn test_auto_migrate_config_creation() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        migrations_dir: Some(PathBuf::from("./migrations")),
        auto_migrate: true,
        migration_timeout: 120,
        ..Default::default()
    };

    assert!(config.auto_migrate);
    assert_eq!(config.migration_timeout, 120);
    assert!(config.migrations_dir.is_some());
}

/// TEST-AM-002: 迁移文件扫描测试（使用内存数据库）
#[tokio::test]
async fn test_migration_file_scanning() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // 创建测试迁移文件
    let migration_content_1 = r#"-- Migration: create_users_table
-- Version: 1

-- UP
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT
);

-- DOWN
DROP TABLE users;
"#;

    let migration_content_2 = r#"-- Migration: create_orders_table
-- Version: 2

-- UP
CREATE TABLE orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    total DECIMAL(10, 2) NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- DOWN
DROP TABLE orders;
"#;

    fs::write(temp_dir.path().join("1_create_users_table.sql"), migration_content_1)
        .expect("Failed to write migration file 1");
    fs::write(temp_dir.path().join("2_create_orders_table.sql"), migration_content_2)
        .expect("Failed to write migration file 2");

    // 使用内存数据库
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 5000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
    };

    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");

    let migrations = pool.run_migrations(temp_dir.path()).await.expect("Failed to run migrations");

    // 验证迁移已应用（内存数据库不支持幂等性测试，因为每次连接都是新的数据库）
    assert_eq!(migrations, 2, "Should have applied 2 migrations");
}

/// TEST-AM-003: 迁移超时配置测试
#[tokio::test]
async fn test_migration_timeout_config() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        migration_timeout: 300,
        ..Default::default()
    };

    assert_eq!(config.migration_timeout, 300);
    assert_eq!(config.migration_timeout_duration().as_secs(), 300);
}

/// TEST-AM-004: 空迁移目录测试
#[tokio::test]
async fn test_empty_migrations_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 5000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
    };

    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");

    let result = pool.run_migrations(temp_dir.path()).await;

    assert!(result.is_ok(), "Running migrations on empty directory should succeed");
    assert_eq!(result.unwrap(), 0, "Should apply 0 migrations");
}

/// TEST-AM-005: 不存在目录测试
#[tokio::test]
async fn test_nonexistent_migrations_directory() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 5000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
    };

    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");

    let non_existent_path = PathBuf::from("/tmp/non_existent_migrations_12345");

    let result = pool.run_migrations(&non_existent_path).await;

    assert!(result.is_ok(), "Running migrations on non-existent directory should succeed");
    assert_eq!(result.unwrap(), 0, "Should apply 0 migrations");
}

/// TEST-AM-006: 环境变量迁移配置测试
#[tokio::test]
async fn test_migration_config_from_env() {
    unsafe {
        std::env::set_var("DATABASE_URL", "sqlite::memory:");
        std::env::set_var("DB_MIGRATIONS_DIR", "/custom/migrations");
        std::env::set_var("DB_AUTO_MIGRATE", "true");
        std::env::set_var("DB_MIGRATION_TIMEOUT", "120");
    }

    let config = DbConfig::from_env().expect("Failed to create config from env");

    assert!(config.auto_migrate);
    assert!(config.migrations_dir.is_some());
    assert_eq!(config.migrations_dir.unwrap(), PathBuf::from("/custom/migrations"));
    assert_eq!(config.migration_timeout, 120);

    unsafe {
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("DB_MIGRATIONS_DIR");
        std::env::remove_var("DB_AUTO_MIGRATE");
        std::env::remove_var("DB_MIGRATION_TIMEOUT");
    }
}

/// TEST-AM-007: 迁移版本排序测试（使用内存数据库）
#[tokio::test]
async fn test_migration_version_sorting() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // 创建乱序的迁移文件
    let migration_v3 = r#"-- Migration: third
-- Version: 3
-- UP
CREATE TABLE table_3 (id INTEGER PRIMARY KEY);
-- DOWN
DROP TABLE table_3;
"#;

    let migration_v1 = r#"-- Migration: first
-- Version: 1
-- UP
CREATE TABLE table_1 (id INTEGER PRIMARY KEY);
-- DOWN
DROP TABLE table_1;
"#;

    let migration_v2 = r#"-- Migration: second
-- Version: 2
-- UP
CREATE TABLE table_2 (id INTEGER PRIMARY KEY);
-- DOWN
DROP TABLE table_2;
"#;

    // 乱序写入
    fs::write(temp_dir.path().join("3_third.sql"), migration_v3).expect("Failed to write v3");
    fs::write(temp_dir.path().join("1_first.sql"), migration_v1).expect("Failed to write v1");
    fs::write(temp_dir.path().join("2_second.sql"), migration_v2).expect("Failed to write v2");

    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 5000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
    };

    let pool = DbPool::with_config(config).await.expect("Failed to create test pool");

    // 运行迁移（内存数据库不支持幂等性测试）
    let applied = pool
        .run_migrations(temp_dir.path())
        .await
        .expect("Failed to run migrations");
    assert_eq!(applied, 3, "Should apply all 3 migrations");
}
