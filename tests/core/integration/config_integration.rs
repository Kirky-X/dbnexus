// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置管理模块集成测试
//!
//! 测试配置构建、加载和验证功能

use dbnexus::{
    DbPool,
    config::{DatabaseType, DbConfigBuilder},
};

#[cfg(feature = "config-yaml")]
use dbnexus::config::DbConfig;

#[tokio::test]
async fn test_config_builder_basic() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(10)
        .min_connections(2)
        .build()
        .unwrap();

    assert_eq!(config.url_sanitized(), "sqlite::memory:");
    assert_eq!(config.max_connections(), 10);
}

#[cfg(feature = "config-yaml")]
#[tokio::test]
async fn test_yaml_loading() {
    let yaml = r#"url: "sqlite::memory:""#;
    let config = DbConfig::from_yaml_str(yaml).unwrap();
    assert_eq!(config.url_sanitized(), "sqlite::memory:");
}

#[cfg(feature = "config-yaml")]
#[tokio::test]
async fn test_yaml_with_all_fields() {
    let yaml = r#"
url: "sqlite::memory:"
max_connections: 20
min_connections: 5
idle_timeout: 300
acquire_timeout: 5000
"#;
    let config = DbConfig::from_yaml_str(yaml).unwrap();
    assert_eq!(config.url_sanitized(), "sqlite::memory:");
    assert_eq!(config.max_connections(), 20);
    assert_eq!(config.min_connections(), 5);
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
    assert_eq!(DatabaseType::parse_database_type("postgres"), DatabaseType::Postgres);
    assert_eq!(DatabaseType::parse_database_type("postgresql"), DatabaseType::Postgres);
    assert_eq!(DatabaseType::parse_database_type("mysql"), DatabaseType::MySql);
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
    assert_eq!(pool.config().max_connections(), 10);
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_dbpool_new() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let _session = pool.get_session("admin").await.unwrap();
    let status = pool.status();
    assert!(status.total >= 1);
}

#[tokio::test]
async fn test_config_validation() {
    let result = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(5)
        .min_connections(10)
        .build();

    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_clone() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(10)
        .build()
        .unwrap();

    let cloned = config.clone();
    assert_eq!(config.url_sanitized(), cloned.url_sanitized());
    assert_eq!(config.max_connections(), cloned.max_connections());
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

    assert_eq!(config.url_sanitized(), "sqlite::memory:");
    assert_eq!(config.max_connections(), 20);
    assert_eq!(config.min_connections(), 5);
    assert_eq!(config.idle_timeout(), 600);
    assert_eq!(config.acquire_timeout(), 10000);
    assert!(config.auto_migrate());
    assert_eq!(config.migration_timeout(), 120);
    assert_eq!(config.admin_role(), "superuser");
}

#[cfg(feature = "config-yaml")]
#[tokio::test]
async fn test_config_invalid_yaml() {
    let invalid_yaml = r#"url: "sqlite::memory:" invalid syntax"#;
    let result = DbConfig::from_yaml_str(invalid_yaml);
    assert!(result.is_err());
}

#[cfg(feature = "config-yaml")]
#[tokio::test]
async fn test_config_missing_required_field() {
    let yaml = r#"max_connections: 10"#;
    let result = DbConfig::from_yaml_str(yaml);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_default_values() {
    let config = DbConfigBuilder::new().url("sqlite::memory:").build().unwrap();

    assert_eq!(config.max_connections(), 20);
    assert_eq!(config.min_connections(), 5);
    assert_eq!(config.idle_timeout(), 300);
    assert_eq!(config.acquire_timeout(), 5000);
    assert!(!config.auto_migrate());
    assert_eq!(config.migration_timeout(), 60);
    assert_eq!(config.admin_role(), "admin");
}

#[tokio::test]
async fn test_config_boundary_values() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(1)
        .min_connections(1)
        .build()
        .unwrap();
    assert_eq!(config.max_connections(), 1);
    assert_eq!(config.min_connections(), 1);

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(1000)
        .min_connections(1)
        .build()
        .unwrap();
    assert_eq!(config.max_connections(), 1000);
}

#[tokio::test]
async fn test_config_boundary_rejection() {
    let result = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(1001)
        .build();
    assert!(result.is_err());

    let result = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(200)
        .min_connections(101)
        .build();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_invalid_urls() {
    let result = DbConfigBuilder::new().url("").build();
    assert!(result.is_err());

    let result = DbConfigBuilder::new().url("invalid://test").build();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_timeout_boundaries() {
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .idle_timeout(1)
        .acquire_timeout(1)
        .build()
        .unwrap();
    assert_eq!(config.idle_timeout(), 1);
    assert_eq!(config.acquire_timeout(), 1);

    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .idle_timeout(86400)
        .acquire_timeout(300000)
        .build()
        .unwrap();
    assert_eq!(config.idle_timeout(), 86400);
    assert_eq!(config.acquire_timeout(), 300000);
}

#[tokio::test]
async fn test_config_url_sanitization() {
    let config = DbConfigBuilder::new()
        .url("postgres://user:password@localhost:5432/mydb")
        .build()
        .unwrap();

    let sanitized = config.url_sanitized();
    assert!(sanitized.contains("postgres://"));
    assert!(!sanitized.contains("password"));

    let mem_config = DbConfigBuilder::new().url("sqlite::memory:").build().unwrap();
    assert_eq!(mem_config.url_sanitized(), "sqlite::memory:");
}

#[tokio::test]
async fn test_config_admin_role_variants() {
    let test_roles = vec!["admin", "administrator", "root", "superuser"];

    for role in test_roles {
        let config = DbConfigBuilder::new()
            .url("sqlite::memory:")
            .admin_role(role)
            .build()
            .unwrap();
        assert_eq!(config.admin_role(), role);
    }
}
