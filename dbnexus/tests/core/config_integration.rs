// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置管理模块集成测试
//!
//! 测试配置构建、加载和验证功能

use dbnexus::{
    config::{DbConfig, DbConfigBuilder, DatabaseType},
    DbPool,
};

#[tokio::test]
async fn test_config_builder_basic() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(10)
        .min_connections(2)
        .build()
        .unwrap();

    assert_eq!(config.url, "sqlite::memory:");
    assert_eq!(config.max_connections, 10);
}

#[tokio::test]
async fn test_config_direct() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 15,
        min_connections: 3,
        idle_timeout: 600,
        acquire_timeout: 10000,
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 120,
        admin_role: "admin".to_string(),
    };

    assert_eq!(config.max_connections, 15);
}

#[tokio::test]
async fn test_yaml_loading() {
    let yaml = r#"url: "sqlite::memory:""#;
    let config = DbConfig::from_yaml_str(yaml).unwrap();
    assert_eq!(config.url, "sqlite::memory:");
}

#[tokio::test]
async fn test_yaml_with_all_fields() {
    let yaml = r#"
url: "sqlite::memory:"
max_connections: 20
min_connections: 5
idle_timeout: 300
acquire_timeout: 5000
permissions_path: "/path/to/permissions.yaml"
migrations_dir: "/path/to/migrations"
auto_migrate: true
migration_timeout: 60
admin_role: "administrator"
"#;
    let config = DbConfig::from_yaml_str(yaml).unwrap();
    assert_eq!(config.url, "sqlite::memory:");
    assert_eq!(config.max_connections, 20);
    assert_eq!(config.min_connections, 5);
    assert_eq!(config.idle_timeout, 300);
    assert_eq!(config.acquire_timeout, 5000);
    assert_eq!(config.permissions_path, Some("/path/to/permissions.yaml".to_string()));
    assert_eq!(config.migrations_dir, Some(std::path::PathBuf::from("/path/to/migrations")));
    assert!(config.auto_migrate);
    assert_eq!(config.migration_timeout, 60);
    assert_eq!(config.admin_role, "administrator");
}

#[tokio::test]
async fn test_database_type() {
    assert_eq!(
        DatabaseType::parse_database_type("sqlite::memory:"),
        DatabaseType::Sqlite
    );
    assert_eq!(
        DatabaseType::parse_database_type("sqlite:///path/to/db"),
        DatabaseType::Sqlite
    );
    assert_eq!(
        DatabaseType::parse_database_type("postgres"),
        DatabaseType::Postgres
    );
    assert_eq!(
        DatabaseType::parse_database_type("postgresql"),
        DatabaseType::Postgres
    );
    assert_eq!(
        DatabaseType::parse_database_type("mysql"),
        DatabaseType::MySql
    );
    assert_eq!(DatabaseType::Sqlite.as_str(), "sqlite");
    assert_eq!(DatabaseType::Postgres.as_str(), "postgres");
    assert_eq!(DatabaseType::MySql.as_str(), "mysql");
}

#[tokio::test]
async fn test_database_type_is_real() {
    assert!(!DatabaseType::Sqlite.is_real_database());
    assert!(DatabaseType::Postgres.is_real_database());
    assert!(DatabaseType::MySql.is_real_database());
}

#[tokio::test]
async fn test_dbpool_from_config() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(10)
        .min_connections(3)
        .build()
        .unwrap();

    let pool = DbPool::try_from(&config).unwrap();
    assert_eq!(pool.config().max_connections, 10);
}

#[tokio::test]
async fn test_dbpool_new() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let status = pool.status();
    assert!(status.total >= 1);
}

#[tokio::test]
async fn test_config_validation() {
    // 测试最小连接数不能超过最大连接数
    let result = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .min_connections(10)  // 最小连接数大于最大连接数
        .build();

    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_with_permissions() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let perm_file = temp_dir.path().join("test_permissions.yaml");

    let yaml_content = r#"
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

    std::fs::write(&perm_file, yaml_content).expect("Failed to write permissions file");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .permissions_path(perm_file.to_string_lossy().as_ref())
        .build()
        .unwrap();

    assert_eq!(config.permissions_path, Some(perm_file.to_string_lossy().to_string()));
}

#[tokio::test]
async fn test_config_with_migrations() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let migrations_dir = temp_dir.path().join("migrations");

    std::fs::create_dir(&migrations_dir).expect("Failed to create migrations directory");

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .migrations_dir(migrations_dir.to_string_lossy().as_ref())
        .auto_migrate(true)
        .build()
        .unwrap();

    assert_eq!(config.migrations_dir, Some(migrations_dir));
    assert!(config.auto_migrate);
}

#[tokio::test]
async fn test_config_clone() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(10)
        .build()
        .unwrap();

    let cloned = config.clone();
    assert_eq!(config.url, cloned.url);
    assert_eq!(config.max_connections, cloned.max_connections);
}

#[tokio::test]
async fn test_config_builder_chaining() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(20)
        .min_connections(5)
        .idle_timeout(600)
        .acquire_timeout(10000)
        .auto_migrate(true)
        .migration_timeout(120)
        .admin_role("superuser")
        .build()
        .unwrap();

    assert_eq!(config.url, "sqlite::memory:");
    assert_eq!(config.max_connections, 20);
    assert_eq!(config.min_connections, 5);
    assert_eq!(config.idle_timeout, 600);
    assert_eq!(config.acquire_timeout, 10000);
    assert!(config.auto_migrate);
    assert_eq!(config.migration_timeout, 120);
    assert_eq!(config.admin_role, "superuser");
}

#[tokio::test]
async fn test_config_invalid_yaml() {
    let invalid_yaml = r#"url: "sqlite::memory:" invalid syntax"#;
    let result = DbConfig::from_yaml_str(invalid_yaml);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_missing_required_field() {
    let yaml = r#"max_connections: 10"#;  // 缺少 url 字段
    let result = DbConfig::from_yaml_str(yaml);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_default_values() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .build()
        .unwrap();

    // 验证默认值
    assert_eq!(config.max_connections, 20);  // 默认值
    assert_eq!(config.min_connections, 5);   // 默认值
    assert_eq!(config.idle_timeout, 300);    // 默认值
    assert_eq!(config.acquire_timeout, 3000); // 默认值
    assert!(!config.auto_migrate);           // 默认值
    assert_eq!(config.migration_timeout, 60); // 默认值
    assert_eq!(config.admin_role, "admin");  // 默认值
}