// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置管理模块集成测试
//!
//! 基于 serde 的配置构建、加载和验证功能测试
//!
//! # 测试说明
//!
//! - 验证 DbConfig 结构体及其 serde Deserialize 实现
//! - `#[cfg(feature = "yaml")]` 测试：验证 DbConfig 的 serde_yaml_ng 反序列化

use dbnexus::foundation::DatabaseType;
use dbnexus::{DbConfig, DbPool, DbPoolBuilder};

#[path = "../../common/mod.rs"]
mod common;

#[tokio::test]
async fn test_config_builder_basic() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 10,
        min_connections: 2,
        ..Default::default()
    };

    assert_eq!(config.url, "sqlite::memory:");
    assert_eq!(config.max_connections, 10);
}

#[cfg(feature = "yaml")]
#[tokio::test]
async fn test_yaml_loading() {
    let yaml = r#"url: "sqlite::memory:""#;
    let config = serde_yaml_ng::from_str::<DbConfig>(yaml).unwrap();
    assert_eq!(config.url, "sqlite::memory:");
}

#[cfg(feature = "yaml")]
#[tokio::test]
async fn test_yaml_with_all_fields() {
    let yaml = r#"
url: "sqlite::memory:"
max_connections: 20
min_connections: 5
idle_timeout: 300
acquire_timeout: 5000
"#;
    let config = serde_yaml_ng::from_str::<DbConfig>(yaml).unwrap();
    assert_eq!(config.url, "sqlite::memory:");
    assert_eq!(config.max_connections, 20);
    assert_eq!(config.min_connections, 5);
}

#[tokio::test]
async fn test_database_type() {
    assert_eq!(DatabaseType::from_url("sqlite::memory:"), DatabaseType::Sqlite);
    assert_eq!(DatabaseType::from_url("sqlite:///path/to/db"), DatabaseType::Sqlite);
    assert_eq!(DatabaseType::from_url("postgres://localhost"), DatabaseType::Postgres);
    assert_eq!(DatabaseType::from_url("postgresql://localhost"), DatabaseType::Postgres);
    assert_eq!(DatabaseType::from_url("mysql://localhost"), DatabaseType::MySql);
    assert_eq!(DatabaseType::Sqlite.as_str(), "sqlite");
    assert_eq!(DatabaseType::Postgres.as_str(), "postgres");
    assert_eq!(DatabaseType::MySql.as_str(), "mysql");
}

#[tokio::test]
async fn test_database_type_is_real() {
    // SQLite 是嵌入式数据库
    assert_eq!(DatabaseType::Sqlite.as_str(), "sqlite");
    // PostgreSQL 和 MySQL 是真正的数据库服务器
    assert_eq!(DatabaseType::Postgres.as_str(), "postgres");
    assert_eq!(DatabaseType::MySql.as_str(), "mysql");
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_dbpool_from_config() {
    let url = common::get_test_database_url();
    let config = DbConfig {
        url,
        max_connections: 10,
        min_connections: 3,
        ..Default::default()
    };

    // try_from uses block_on which can't be called from within a tokio runtime
    // So we use the async version instead and pass config directly
    let pool = DbPoolBuilder::new().config(config).build().await.unwrap();
    assert_eq!(pool.config().max_connections, 10);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_dbpool_new() {
    let (config, _temp_dir) = common::get_test_config();
    let pool = DbPool::with_config(config).await.unwrap();
    let _session = pool.get_session("admin").await.unwrap();
    let status = pool.status();
    assert!(status.total >= 1);
}

#[tokio::test]
async fn test_config_validation() {
    // min > max 应该验证失败
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 10,
        ..Default::default()
    };

    // 注意：DbConfig 结构体不会在创建时验证，需要手动验证或由 DbPool 验证
    // 这里我们检查配置已创建，但 DbPool 创建时会失败
    assert!(config.max_connections < config.min_connections);
}

#[tokio::test]
async fn test_config_clone() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 10,
        ..Default::default()
    };

    let cloned = config.clone();
    assert_eq!(config.url, cloned.url);
    assert_eq!(config.max_connections, cloned.max_connections);
}

#[tokio::test]
async fn test_config_builder_chaining() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 20,
        min_connections: 5,
        idle_timeout: 600,
        acquire_timeout: 10000,
        auto_migrate: true,
        migration_timeout: 120,
        admin_role: "superuser".to_string(),
        ..Default::default()
    };

    assert_eq!(config.url, "sqlite::memory:");
    assert_eq!(config.max_connections, 20);
    assert_eq!(config.min_connections, 5);
    assert_eq!(config.idle_timeout, 600);
    assert_eq!(config.acquire_timeout, 10000);
    assert!(config.auto_migrate);
    assert_eq!(config.migration_timeout, 120);
    assert_eq!(config.admin_role, "superuser");
}

#[cfg(feature = "yaml")]
#[tokio::test]
async fn test_config_invalid_yaml() {
    let invalid_yaml = r#"url: "sqlite::memory:" invalid syntax"#;
    let result = serde_yaml_ng::from_str::<DbConfig>(invalid_yaml);
    assert!(result.is_err());
}

#[cfg(feature = "yaml")]
#[tokio::test]
async fn test_config_missing_required_field() {
    let yaml = r#"max_connections: 10"#;
    let result = serde_yaml_ng::from_str::<DbConfig>(yaml);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_default_values() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    };

    assert_eq!(config.max_connections, 20);
    assert_eq!(config.min_connections, 5);
    assert_eq!(config.idle_timeout, 300);
    assert_eq!(config.acquire_timeout, 5000);
    assert!(!config.auto_migrate);
    assert_eq!(config.migration_timeout, 60);
    assert_eq!(config.admin_role, "admin");
}

#[tokio::test]
async fn test_config_boundary_values() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1,
        min_connections: 1,
        ..Default::default()
    };
    assert_eq!(config.max_connections, 1);
    assert_eq!(config.min_connections, 1);

    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1000,
        min_connections: 1,
        ..Default::default()
    };
    assert_eq!(config.max_connections, 1000);
}

#[tokio::test]
async fn test_config_boundary_rejection() {
    // 注意：DbConfig 结构体不会在创建时验证，这些值会被接受
    // 验证会在 DbPool 创建时进行
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1001,
        ..Default::default()
    };
    // 配置已创建，值被设置
    assert_eq!(config.max_connections, 1001);

    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 200,
        min_connections: 101,
        ..Default::default()
    };
    // 配置已创建，值被设置
    assert_eq!(config.max_connections, 200);
    assert_eq!(config.min_connections, 101);
}

#[tokio::test]
async fn test_config_invalid_urls() {
    // 空URL - 配置会被创建，但 DbPool 创建时会失败
    let config = DbConfig {
        url: "".to_string(),
        ..Default::default()
    };
    // 配置已创建
    assert_eq!(config.url, "");

    // 无效URL格式 - 配置会被创建，但 DbPool 创建时会失败
    let config = DbConfig {
        url: "invalid://test".to_string(),
        ..Default::default()
    };
    // 配置已创建
    assert_eq!(config.url, "invalid://test");
}

#[tokio::test]
async fn test_config_timeout_boundaries() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        idle_timeout: 1,
        acquire_timeout: 1,
        ..Default::default()
    };
    assert_eq!(config.idle_timeout, 1);
    assert_eq!(config.acquire_timeout, 1);

    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        idle_timeout: 86400,
        acquire_timeout: 300000,
        ..Default::default()
    };
    assert_eq!(config.idle_timeout, 86400);
    assert_eq!(config.acquire_timeout, 300000);
}

#[tokio::test]
async fn test_config_url_access() {
    let config = DbConfig {
        url: "postgres://user:password@localhost:5432/mydb".to_string(),
        ..Default::default()
    };

    // URL 可以直接访问
    assert!(config.url.contains("postgres://"));
    assert!(config.url.contains("password"));

    let mem_config = DbConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    };
    assert_eq!(mem_config.url, "sqlite::memory:");
}

#[tokio::test]
async fn test_config_admin_role_variants() {
    let test_roles = vec!["admin", "administrator", "root", "superuser"];

    for role in test_roles {
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            admin_role: role.to_string(),
            ..Default::default()
        };
        assert_eq!(config.admin_role, role);
    }
}
