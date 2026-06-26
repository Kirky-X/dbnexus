// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in project root for full license information.

//! common::error 与 DatabaseType 单元测试
//!
//! 覆盖：
//! - `DatabaseType` 枚举的默认值、Display、Clone/Copy、PartialEq、Debug、Serialize/Deserialize
//! - `DbNexusResult` 类型别名 Ok 路径

use dbnexus::common::error::DbNexusResult;
use dbnexus::foundation::config::DatabaseType;

// ============================================================================
// DatabaseType 枚举测试
// ============================================================================

/// TEST-U-COMMON-001: 默认值应为 Sqlite
#[test]
fn test_database_type_default_is_sqlite() {
    let db_type = DatabaseType::default();
    assert!(matches!(db_type, DatabaseType::Sqlite));
}

/// TEST-U-COMMON-002: Display 实现应输出小写名称
#[test]
fn test_database_type_display() {
    assert_eq!(DatabaseType::Sqlite.to_string(), "sqlite");
    assert_eq!(DatabaseType::Postgres.to_string(), "postgres");
    assert_eq!(DatabaseType::MySql.to_string(), "mysql");
}

/// TEST-U-COMMON-003: Clone 应产生相等副本
#[test]
fn test_database_type_clone() {
    let original = DatabaseType::Postgres;
    let cloned = original;
    assert_eq!(original, cloned);
}

/// TEST-U-COMMON-004: Copy 语义 — 赋值后原值仍可用且相等
#[test]
fn test_database_type_copy() {
    let original = DatabaseType::MySql;
    let copied = original; // Copy 发生
    assert_eq!(original, copied);
}

/// TEST-U-COMMON-005: PartialEq 应正确判等
#[test]
fn test_database_type_partialeq() {
    assert_eq!(DatabaseType::Sqlite, DatabaseType::Sqlite);
    assert_ne!(DatabaseType::Sqlite, DatabaseType::Postgres);
    assert_ne!(DatabaseType::Postgres, DatabaseType::MySql);
}

/// TEST-U-COMMON-006: Debug 输出应非空且包含变体名
#[test]
fn test_database_type_debug() {
    let debug_str = format!("{:?}", DatabaseType::Postgres);
    assert!(debug_str.contains("Postgres"));
    assert!(!debug_str.is_empty());
}

/// TEST-U-COMMON-007: serde Serialize/Deserialize round-trip 应保持相等
#[test]
fn test_database_type_serde_round_trip() {
    let cases = [DatabaseType::Sqlite, DatabaseType::Postgres, DatabaseType::MySql];
    for original in cases {
        let json = serde_json::to_string(&original).expect("serialize should succeed");
        let restored: DatabaseType =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(original, restored, "round-trip failed for {:?}", original);
    }
}

/// TEST-U-COMMON-008: 变体数量应为 3（防止误删变体）
#[test]
fn test_database_type_variant_count() {
    let variants = [DatabaseType::Sqlite, DatabaseType::Postgres, DatabaseType::MySql];
    assert_eq!(variants.len(), 3);
    // 确保三个变体互不相等
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

/// TEST-U-COMMON-009: DbNexusResult Ok 路径应解包为原值
#[test]
#[allow(clippy::unnecessary_literal_unwrap)]
fn test_dbnexus_result_ok() {
    let value = 42i32;
    let result: DbNexusResult<i32> = Ok(value);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), value);
}
