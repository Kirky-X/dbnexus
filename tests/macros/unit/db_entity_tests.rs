// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! db_entity 宏测试套件
//!
//! 测试 `#[db_entity(...)]` 统一属性宏的正确性

use trybuild::TestCases;

/// 测试 db_entity 宏基本展开
///
/// 验证宏能够正确展开并生成 table_name() 和 primary_key_column() 方法
#[test]
fn test_db_entity_basic() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_basic.rs");
}

/// 测试 db_entity 宏 permissions 子参数
///
/// 验证宏能够正确生成 ALLOWED_ROLES、ALLOWED_OPERATIONS 常量
#[test]
fn test_db_entity_with_permissions() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_with_permissions.rs");
}

/// 测试 db_entity 宏 cache 子参数
///
/// 验证宏能够正确生成 CACHE_TTL、CACHE_STRATEGY 等常量和 cache_key() 方法
#[test]
fn test_db_entity_with_cache() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_with_cache.rs");
}

/// 测试 db_entity 宏 audit 子参数
///
/// 验证宏能够正确生成 AUDIT_TABLE_NAME、AUDIT_ENABLED 等常量
#[test]
fn test_db_entity_with_audit() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_with_audit.rs");
}

/// 测试 db_entity 宏缺少 table_name 参数时的错误
///
/// 验证宏在缺少 table_name 时给出清晰的编译错误
#[test]
fn test_db_entity_missing_table_name() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/db_entity_missing_table_name.rs");
}

/// 测试 db_entity 宏缺少 primary_key 参数时的错误
///
/// 验证宏在缺少 primary_key 时给出清晰的编译错误
#[test]
fn test_db_entity_missing_primary_key() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/db_entity_missing_primary_key.rs");
}
