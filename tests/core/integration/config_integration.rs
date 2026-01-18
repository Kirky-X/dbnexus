// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置管理模块集成测试
//!
//! 测试配置构建、加载和验证功能

use dbnexus::{
    DbPool,
    config::{DatabaseType, DbConfig, DbConfigBuilder},
};

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

#[tokio::test]
async fn test_yaml_loading() {
    let yaml = r#"url: "sqlite::memory:""#;
    let config = DbConfig::from_yaml_str(yaml).unwrap();
    assert_eq!(config.url_sanitized(), "sqlite::memory:");
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
    assert_eq!(config.url_sanitized(), "sqlite::memory:");
    assert_eq!(config.max_connections(), 20);
    assert_eq!(config.min_connections(), 5);
    assert_eq!(config.idle_timeout(), 300);
    assert_eq!(config.acquire_timeout(), 5000);
    assert_eq!(config.permissions_path(), Some("/path/to/permissions.yaml".as_ref()));
    assert_eq!(
        config.migrations_dir(),
        Some(std::path::PathBuf::from("/path/to/migrations").as_path())
    );
    assert!(config.auto_migrate());
    assert_eq!(config.migration_timeout(), 60);
    assert_eq!(config.admin_role(), "administrator");
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
    // 获取 session 以确保连接已建立
    let _session = pool.get_session("admin").await.unwrap();
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

    assert_eq!(config.permissions_path(), Some(perm_file.to_string_lossy().as_ref()));
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

    assert_eq!(config.migrations_dir(), Some(migrations_dir.as_path()));
    assert!(config.auto_migrate());
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

#[tokio::test]
async fn test_config_invalid_yaml() {
    let invalid_yaml = r#"url: "sqlite::memory:" invalid syntax"#;
    let result = DbConfig::from_yaml_str(invalid_yaml);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_missing_required_field() {
    let yaml = r#"max_connections: 10"#; // 缺少 url 字段
    let result = DbConfig::from_yaml_str(yaml);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_default_values() {
    let config = DbConfigBuilder::new().url("sqlite::memory:").build().unwrap();

    // 验证默认值
    assert_eq!(config.max_connections(), 20); // 默认值
    assert_eq!(config.min_connections(), 5); // 默认值
    assert_eq!(config.idle_timeout(), 300); // 默认值
    assert_eq!(config.acquire_timeout(), 5000); // 默认值（恢复为保守值）
    assert!(!config.auto_migrate()); // 默认值
    assert_eq!(config.migration_timeout(), 60); // 默认值
    assert_eq!(config.admin_role(), "admin"); // 默认值
}

// ============ 边界场景测试 ============

#[tokio::test]
async fn test_config_boundary_values() {
    // 测试边界值：最小连接数为 1
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(1)
        .min_connections(1)
        .build()
        .unwrap();
    assert_eq!(config.max_connections(), 1);
    assert_eq!(config.min_connections(), 1);

    // 测试边界值：最大允许连接数 1000
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
    // 测试超过最大连接数限制
    let result = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(1001) // 超过 1000 限制
        .build();
    assert!(result.is_err());

    // 测试 min_connections 超过 100
    let result = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(200)
        .min_connections(101) // 超过 100 限制
        .build();
    assert!(result.is_err());

    // 测试 min_connections 为 0
    let result = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .max_connections(10)
        .min_connections(0)
        .build();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_invalid_urls() {
    // 测试空 URL
    let result = DbConfigBuilder::new().url("").build();
    assert!(result.is_err());

    // 测试无效协议
    let result = DbConfigBuilder::new().url("invalid://test").build();
    assert!(result.is_err());

    // 测试包含非法字符的协议
    let result = DbConfigBuilder::new().url("test@protocol://host").build();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_valid_url_variants() {
    // 测试各种有效的 URL 变体
    let test_cases = vec![
        ("sqlite::memory:", true),
        ("sqlite3::memory:", true),
        ("sqlite:///test.db", true),
        ("sqlite3:///test.db", true),
        ("postgres://localhost/test", true),
        ("postgresql://localhost/test", true),
        ("mysql://localhost/test", true),
    ];

    for (url, _should_pass) in test_cases {
        let result = DbConfigBuilder::new().url(url).build();
        // 这些 URL 应该都能通过基本验证
        // 注意：有些可能需要实际的数据库连接
        assert!(
            result.is_ok() || matches!(result, Err(dbnexus::config::ConfigError::UnsupportedProtocol)),
            "URL {} should be valid or unsupported protocol",
            url
        );
    }
}

#[tokio::test]
async fn test_config_timeout_boundaries() {
    // 测试超时边界值
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .idle_timeout(1) // 最小空闲超时
        .acquire_timeout(1) // 最小获取超时
        .build()
        .unwrap();
    assert_eq!(config.idle_timeout(), 1);
    assert_eq!(config.acquire_timeout(), 1);

    // 测试大超时值
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .idle_timeout(86400) // 24小时
        .acquire_timeout(300000) // 5分钟
        .build()
        .unwrap();
    assert_eq!(config.idle_timeout(), 86400);
    assert_eq!(config.acquire_timeout(), 300000);
}

#[tokio::test]
async fn test_config_url_sanitization() {
    // 测试 URL 脱敏功能
    let config = DbConfigBuilder::new()
        .url("postgres://user:password@localhost:5432/mydb")
        .build()
        .unwrap();

    let sanitized = config.url_sanitized();
    assert!(sanitized.contains("postgres://"));
    assert!(!sanitized.contains("password"));
    assert!(sanitized.contains("@"));

    // 内存数据库不应该被脱敏
    let mem_config = DbConfigBuilder::new().url("sqlite::memory:").build().unwrap();
    assert_eq!(mem_config.url_sanitized(), "sqlite::memory:");
}

#[tokio::test]
async fn test_config_admin_role_variants() {
    // 测试管理员角色名称变体
    let test_roles = vec!["admin", "administrator", "root", "superuser", "ADMIN", "Admin"];

    for role in test_roles {
        let config = DbConfigBuilder::new()
            .url("sqlite::memory:")
            .admin_role(role)
            .build()
            .unwrap();
        assert_eq!(config.admin_role(), role);
    }
}

#[tokio::test]
async fn test_config_warmup_boundaries() {
    // 测试预热参数边界值
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .warmup_timeout(1) // 最小超时
        .warmup_retries(0) // 最小重试次数
        .build()
        .unwrap();
    assert_eq!(config.warmup_timeout(), 1);
    assert_eq!(config.warmup_retries(), 0);

    // 测试大值
    let config = DbConfigBuilder::new()
        .url("sqlite::memory:")
        .warmup_timeout(300) // 5分钟
        .warmup_retries(10)
        .build()
        .unwrap();
    assert_eq!(config.warmup_timeout(), 300);
    assert_eq!(config.warmup_retries(), 10);
}
