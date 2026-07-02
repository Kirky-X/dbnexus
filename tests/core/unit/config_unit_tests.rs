// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in project root for full license information.

//! 配置类型单元测试
//!
//! 覆盖：
//! - `DbConfig` 默认值、自定义字段、`database_type()` 解析、Duration 转换方法
//! - `foundation::config::PoolConfig` 默认值与 Duration 转换
//! - `CacheConfig` 默认值与 TTL Duration
//! - `foundation::config::DatabaseType` 的 `from_url`/`as_str`/`is_real_database`/`Display`
//! - `ConfigError` 各变体 Display

use dbnexus::foundation::config::{
    CacheConfig, ConfigError, DatabaseType as FoundationDatabaseType, DbConfig, PoolConfig as FoundationPoolConfig,
};
use std::time::Duration;

// ============================================================================
// DbConfig 测试
// ============================================================================

/// TEST-U-CONFIG-001: DbConfig 默认值应符合约定
#[test]
fn test_db_config_default_values() {
    let config = DbConfig::default();
    assert_eq!(config.url, "");
    assert_eq!(config.max_connections, 20);
    assert_eq!(config.min_connections, 5);
    assert_eq!(config.idle_timeout, 300);
    assert_eq!(config.acquire_timeout, 5000);
    assert_eq!(config.admin_role, "admin");
    assert_eq!(config.migration_timeout, 60);
    assert_eq!(config.warmup_timeout, 30);
    assert_eq!(config.warmup_retries, 3);
    assert!(!config.auto_migrate);
    assert!(config.permissions_path.is_none());
    assert!(config.migrations_dir.is_none());
}

/// TEST-U-CONFIG-002: DbConfig 自定义字段应正确赋值
#[test]
fn test_db_config_custom_values() {
    let config = DbConfig {
        url: "postgres://localhost/db".to_string(),
        max_connections: 50,
        min_connections: 10,
        idle_timeout: 600,
        acquire_timeout: 10000,
        admin_role: "root".to_string(),
        migration_timeout: 120,
        warmup_timeout: 60,
        warmup_retries: 5,
        auto_migrate: true,
        permissions_path: Some("/etc/permissions.yaml".to_string()),
        migrations_dir: Some(std::path::PathBuf::from("/etc/migrations")),
        cache_config: CacheConfig::default(),
    };
    assert_eq!(config.url, "postgres://localhost/db");
    assert_eq!(config.max_connections, 50);
    assert_eq!(config.min_connections, 10);
    assert_eq!(config.admin_role, "root");
    assert!(config.auto_migrate);
    assert_eq!(config.permissions_path, Some("/etc/permissions.yaml".to_string()));
}

/// TEST-U-CONFIG-003: database_type() 应正确解析 postgres URL
#[test]
fn test_db_config_database_type_postgres() {
    let config = DbConfig {
        url: "postgres://user:pass@localhost/db".to_string(),
        ..Default::default()
    };
    assert_eq!(config.database_type().unwrap(), FoundationDatabaseType::Postgres);
}

/// TEST-U-CONFIG-004: database_type() 应正确解析 postgresql:// 前缀
#[test]
fn test_db_config_database_type_postgresql_scheme() {
    let config = DbConfig {
        url: "postgresql://localhost/db".to_string(),
        ..Default::default()
    };
    assert_eq!(config.database_type().unwrap(), FoundationDatabaseType::Postgres);
}

/// TEST-U-CONFIG-005: database_type() 应正确解析 mysql URL
#[test]
fn test_db_config_database_type_mysql() {
    let config = DbConfig {
        url: "mysql://localhost/db".to_string(),
        ..Default::default()
    };
    assert_eq!(config.database_type().unwrap(), FoundationDatabaseType::MySql);
}

/// TEST-U-CONFIG-006: database_type() 应将 sqlite URL 解析为 Sqlite
#[test]
fn test_db_config_database_type_sqlite() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    };
    assert_eq!(config.database_type().unwrap(), FoundationDatabaseType::Sqlite);
}

/// TEST-U-CONFIG-007: database_type() 应对未知协议返回错误（0.3.0 行为变更：不再默认 Sqlite）
#[test]
fn test_db_config_database_type_unknown_returns_error() {
    let config = DbConfig {
        url: "unknown://localhost".to_string(),
        ..Default::default()
    };
    assert!(config.database_type().is_err());
}

/// TEST-U-CONFIG-008: database_type() 应大小写不敏感
#[test]
fn test_db_config_database_type_case_insensitive() {
    let config = DbConfig {
        url: "POSTGRES://localhost/db".to_string(),
        ..Default::default()
    };
    assert_eq!(config.database_type().unwrap(), FoundationDatabaseType::Postgres);
}

/// TEST-U-CONFIG-009: idle_timeout_duration() 应转换为 Duration
#[test]
fn test_db_config_idle_timeout_duration() {
    let config = DbConfig {
        idle_timeout: 120,
        ..Default::default()
    };
    assert_eq!(config.idle_timeout_duration(), Duration::from_secs(120));
}

/// TEST-U-CONFIG-010: acquire_timeout_duration() 应转换为 Duration（毫秒）
#[test]
fn test_db_config_acquire_timeout_duration() {
    let config = DbConfig {
        acquire_timeout: 3000,
        ..Default::default()
    };
    assert_eq!(config.acquire_timeout_duration(), Duration::from_millis(3000));
}

/// TEST-U-CONFIG-011: migration_timeout_duration() 应转换为 Duration
#[test]
fn test_db_config_migration_timeout_duration() {
    let config = DbConfig {
        migration_timeout: 90,
        ..Default::default()
    };
    assert_eq!(config.migration_timeout_duration(), Duration::from_secs(90));
}

/// TEST-U-CONFIG-012: cache_config() 应返回缓存配置引用
#[test]
fn test_db_config_cache_config_accessor() {
    let config = DbConfig::default();
    let cache = config.cache_config();
    assert_eq!(cache.policy_cache_capacity, 4096);
    assert_eq!(cache.default_ttl, 300);
}

/// TEST-U-CONFIG-013: DbConfig 应支持 Clone
#[test]
fn test_db_config_clone() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 10,
        ..Default::default()
    };
    let cloned = config.clone();
    assert_eq!(config.url, cloned.url);
    assert_eq!(config.max_connections, cloned.max_connections);
}

// ============================================================================
// foundation::config::PoolConfig 测试
// ============================================================================

/// TEST-U-CONFIG-014: foundation::config::PoolConfig 默认值
#[test]
fn test_foundation_pool_config_default() {
    let config = FoundationPoolConfig::default();
    assert_eq!(config.max_connections, 20);
    assert_eq!(config.min_connections, 5);
    assert_eq!(config.idle_timeout, 300);
    assert_eq!(config.acquire_timeout, 5000);
}

/// TEST-U-CONFIG-015: foundation::config::PoolConfig Duration 转换
#[test]
fn test_foundation_pool_config_duration_conversions() {
    let config = FoundationPoolConfig {
        max_connections: 30,
        min_connections: 10,
        idle_timeout: 600,
        acquire_timeout: 8000,
    };
    assert_eq!(config.idle_timeout_duration(), Duration::from_secs(600));
    assert_eq!(config.acquire_timeout_duration(), Duration::from_millis(8000));
}

// ============================================================================
// CacheConfig 测试
// ============================================================================

/// TEST-U-CONFIG-016: CacheConfig 默认值
#[test]
fn test_cache_config_default() {
    let cache = CacheConfig::default();
    assert_eq!(cache.policy_cache_capacity, 4096);
    assert_eq!(cache.sql_parse_cache_capacity, 1000);
    assert_eq!(cache.query_cache_capacity, 10000);
    assert_eq!(cache.default_ttl, 300);
}

/// TEST-U-CONFIG-017: CacheConfig default_ttl_duration() 应转换为 Duration
#[test]
fn test_cache_config_default_ttl_duration() {
    let cache = CacheConfig {
        default_ttl: 600,
        ..Default::default()
    };
    assert_eq!(cache.default_ttl_duration(), Duration::from_secs(600));
}

// ============================================================================
// foundation::config::DatabaseType 测试
// ============================================================================

/// TEST-U-CONFIG-018: DatabaseType::from_url() 各协议解析
#[test]
fn test_foundation_database_type_from_url() {
    assert_eq!(
        FoundationDatabaseType::from_url("postgres://localhost").unwrap(),
        FoundationDatabaseType::Postgres
    );
    assert_eq!(
        FoundationDatabaseType::from_url("postgresql://localhost").unwrap(),
        FoundationDatabaseType::Postgres
    );
    assert_eq!(
        FoundationDatabaseType::from_url("mysql://localhost").unwrap(),
        FoundationDatabaseType::MySql
    );
    assert_eq!(
        FoundationDatabaseType::from_url("sqlite::memory:").unwrap(),
        FoundationDatabaseType::Sqlite
    );
    assert!(FoundationDatabaseType::from_url("unknown://localhost").is_err());
}

/// TEST-U-CONFIG-019: DatabaseType::parse_database_type() 应为 from_url() 别名
#[test]
fn test_foundation_database_type_parse_alias() {
    assert_eq!(
        FoundationDatabaseType::parse_database_type("postgres://x").unwrap(),
        FoundationDatabaseType::from_url("postgres://x").unwrap()
    );
    assert_eq!(
        FoundationDatabaseType::parse_database_type("mysql://x").unwrap(),
        FoundationDatabaseType::from_url("mysql://x").unwrap()
    );
}

/// TEST-U-CONFIG-020: DatabaseType::as_str() 应返回小写名称
#[test]
fn test_foundation_database_type_as_str() {
    assert_eq!(FoundationDatabaseType::Postgres.as_str(), "postgres");
    assert_eq!(FoundationDatabaseType::MySql.as_str(), "mysql");
    assert_eq!(FoundationDatabaseType::Sqlite.as_str(), "sqlite");
}

/// TEST-U-CONFIG-021: DatabaseType::is_real_database() 应区分真实数据库与 SQLite
#[test]
fn test_foundation_database_type_is_real_database() {
    assert!(FoundationDatabaseType::Postgres.is_real_database());
    assert!(FoundationDatabaseType::MySql.is_real_database());
    assert!(!FoundationDatabaseType::Sqlite.is_real_database());
}

/// TEST-U-CONFIG-022: DatabaseType Display 应与 as_str 一致
#[test]
fn test_foundation_database_type_display() {
    assert_eq!(FoundationDatabaseType::Postgres.to_string(), "postgres");
    assert_eq!(FoundationDatabaseType::MySql.to_string(), "mysql");
    assert_eq!(FoundationDatabaseType::Sqlite.to_string(), "sqlite");
}

/// TEST-U-CONFIG-023: DatabaseType PartialEq/Copy/Clone
#[test]
fn test_foundation_database_type_traits() {
    let a = FoundationDatabaseType::Postgres;
    let b = a; // Copy
    assert_eq!(a, b);
    assert_ne!(a, FoundationDatabaseType::Sqlite);
}

// ============================================================================
// ConfigError Display 测试
// ============================================================================

/// TEST-U-CONFIG-024: ConfigError::MissingField Display
#[test]
fn test_config_error_missing_field_display() {
    let err = ConfigError::MissingField("url".to_string());
    let msg = err.to_string();
    assert!(msg.contains("url"), "msg = {}", msg);
    assert!(msg.contains("Missing"), "msg = {}", msg);
}

/// TEST-U-CONFIG-025: ConfigError::MissingUrl Display
#[test]
fn test_config_error_missing_url_display() {
    let err = ConfigError::MissingUrl;
    assert!(err.to_string().contains("url"));
}

/// TEST-U-CONFIG-026: ConfigError::InvalidValue Display 应包含 key 和 message
#[test]
fn test_config_error_invalid_value_display() {
    let err = ConfigError::InvalidValue {
        key: "max_connections".to_string(),
        message: "must be positive".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("max_connections"), "msg = {}", msg);
    assert!(msg.contains("must be positive"), "msg = {}", msg);
}

/// TEST-U-CONFIG-027: ConfigError::InvalidUrl Display
#[test]
fn test_config_error_invalid_url_display() {
    let err = ConfigError::InvalidUrl("bad scheme".to_string());
    assert!(err.to_string().contains("bad scheme"));
}

/// TEST-U-CONFIG-028: ConfigError::UnsupportedProtocol Display
#[test]
fn test_config_error_unsupported_protocol_display() {
    let err = ConfigError::UnsupportedProtocol("foo://".to_string());
    let msg = err.to_string();
    assert!(msg.contains("foo://") || msg.contains("protocol"), "msg = {}", msg);
}
