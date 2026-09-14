// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! error 与 DatabaseType 单元测试
//!
//! 覆盖：
//! - `DatabaseType` 枚举的默认值、Display、Clone/Copy、PartialEq、Debug、Serialize/Deserialize
//! - `DbNexusResult` 类型别名 Ok 路径

use dbnexus::DbNexusResult;
use dbnexus::foundation::DatabaseType;

// ============================================================================
// DatabaseType 枚举测试
// ============================================================================

/// 默认值应为 Sqlite
#[test]
fn test_database_type_default_is_sqlite() {
    let db_type = DatabaseType::default();
    assert!(matches!(db_type, DatabaseType::Sqlite));
}

/// Display 实现应输出小写名称
#[test]
fn test_database_type_display() {
    assert_eq!(DatabaseType::Sqlite.to_string(), "sqlite");
    assert_eq!(DatabaseType::Postgres.to_string(), "postgres");
    assert_eq!(DatabaseType::MySql.to_string(), "mysql");
}

/// Clone 应产生相等副本
#[test]
fn test_database_type_clone() {
    let original = DatabaseType::Postgres;
    let cloned = original;
    assert_eq!(original, cloned);
}

/// Copy 语义 — 赋值后原值仍可用且相等
#[test]
fn test_database_type_copy() {
    let original = DatabaseType::MySql;
    let copied = original; // Copy 发生
    assert_eq!(original, copied);
}

/// PartialEq 应正确判等
#[test]
fn test_database_type_partialeq() {
    assert_eq!(DatabaseType::Sqlite, DatabaseType::Sqlite);
    assert_ne!(DatabaseType::Sqlite, DatabaseType::Postgres);
    assert_ne!(DatabaseType::Postgres, DatabaseType::MySql);
}

/// Debug 输出应非空且包含变体名
#[test]
fn test_database_type_debug() {
    let debug_str = format!("{:?}", DatabaseType::Postgres);
    assert!(debug_str.contains("Postgres"));
    assert!(!debug_str.is_empty());
}

/// serde Serialize/Deserialize round-trip 应保持相等
#[test]
fn test_database_type_serde_round_trip() {
    let cases = [
        DatabaseType::Sqlite,
        DatabaseType::Postgres,
        DatabaseType::MySql,
    ];
    for original in cases {
        let json = serde_json::to_string(&original).expect("serialize should succeed");
        let restored: DatabaseType =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(original, restored, "round-trip failed for {:?}", original);
    }
}

/// 变体数量应为 4（0.3.0 新增 DuckDb，防止误删变体）
#[test]
fn test_database_type_variant_count() {
    let variants = [
        DatabaseType::Sqlite,
        DatabaseType::Postgres,
        DatabaseType::MySql,
        DatabaseType::DuckDb,
    ];
    assert_eq!(variants.len(), 4);
    // 确保四个变体互不相等
    for i in 0..variants.len() {
        for j in 0..variants.len() {
            if i == j {
                assert_eq!(variants[i], variants[j]);
            } else {
                assert_ne!(variants[i], variants[j]);
            }
        }
    }
}

// ============================================================================
// DbNexusResult 类型别名测试
// ============================================================================

/// DbNexusResult Ok 路径应解包为原值
#[test]
#[allow(clippy::unnecessary_literal_unwrap)]
fn test_dbnexus_result_ok() {
    let value = 42i32;
    let result: DbNexusResult<i32> = Ok(value);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), value);
}

// ============================================================================
// DatabaseType::from_url 边界用例测试
// ============================================================================

/// 带 query 参数的 PostgreSQL URI 应正确识别为 Postgres
#[test]
fn test_from_url_postgres_with_query_params() {
    let url = "postgres://user:pass@localhost:5432/mydb?sslmode=require&connect_timeout=10";
    let db_type = DatabaseType::from_url(url).expect("postgres URL with query should parse");
    assert_eq!(db_type, DatabaseType::Postgres);
}

/// 带 query 参数的 MySQL URI 应正确识别为 MySql
#[test]
fn test_from_url_mysql_with_query_params() {
    let url = "mysql://root:secret@127.0.0.1:3306/auth?charset=utf8mb4&parseTime=true";
    let db_type = DatabaseType::from_url(url).expect("mysql URL with query should parse");
    assert_eq!(db_type, DatabaseType::MySql);
}

/// 无 scheme 的字符串应返回错误（不回退到默认值）
#[test]
fn test_from_url_no_scheme_returns_error() {
    // 仅 host:port 形式，url::Url::parse 视为相对路径，无 scheme
    let result = DatabaseType::from_url("localhost:5432");
    assert!(
        result.is_err(),
        "URL without scheme should return error, got {:?}",
        result.ok()
    );
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Unsupported database scheme") || msg.contains("failed to parse URL"),
        "error message should mention unsupported scheme or parse failure, got: {msg}"
    );
}

/// 相对路径应返回错误（不回退到 Sqlite）
#[test]
fn test_from_url_relative_path_returns_error() {
    // 相对路径，url::Url::parse 视为相对 URL，无 scheme
    let result = DatabaseType::from_url("path/to/db.sqlite");
    assert!(
        result.is_err(),
        "relative path should return error, got {:?}",
        result.ok()
    );
}

/// 未知 scheme 应返回 UnsupportedDatabaseScheme 错误
#[test]
fn test_from_url_unknown_scheme_returns_error() {
    let result = DatabaseType::from_url("ftp://localhost/data");
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("ftp") || msg.contains("not a supported database scheme"),
        "error should mention the unknown scheme 'ftp', got: {msg}"
    );
}

/// DuckDb 短形式应识别为 DuckDb
#[test]
fn test_from_url_duckdb_short_form() {
    let result = DatabaseType::from_url("duckdb::memory:");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), DatabaseType::DuckDb);
}

/// postgresql scheme（别名）应识别为 Postgres
#[test]
fn test_from_url_postgresql_alias() {
    let result = DatabaseType::from_url("postgresql://user@host/db");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), DatabaseType::Postgres);
}
