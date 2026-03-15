// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DbEntity 宏测试套件
//!
//! 测试 `#[derive(DbEntity)]` 派生宏的正确性

use trybuild::TestCases;

/// 测试 DbEntity 宏基本展开
///
/// 验证宏能够正确展开并生成 table_name() 和 primary_key_column() 方法
#[test]
fn test_db_entity_basic() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_basic.rs");
}

/// 测试 DbEntity 宏使用 sea_orm 属性
///
/// 验证宏能够从 #[sea_orm(table_name = "...")] 中提取表名
#[test]
fn test_db_entity_sea_orm_attrs() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_sea_orm.rs");
}

/// 测试 DbEntity 宏缺少 table_name 属性时的错误
///
/// 验证宏在缺少 table_name 时给出清晰的编译错误
#[test]
fn test_db_entity_missing_table_name() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/db_entity_missing_table_name.rs");
}

/// 测试 DbEntity 宏缺少 primary_key 属性时的错误
///
/// 验证宏在缺少 primary_key 时给出清晰的编译错误
#[test]
fn test_db_entity_missing_primary_key() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/db_entity_missing_primary_key.rs");
}

/// 测试 DbEntity 宏与泛型结构体
///
/// 验证宏能够正确处理泛型结构体
#[test]
fn test_db_entity_generic() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_generic.rs");
}

/// 测试 DbEntity 宏生成的方法签名
///
/// 验证生成的 table_name() 和 primary_key_column() 方法具有正确的签名
#[test]
fn test_db_entity_method_signatures() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_method_signatures.rs");
}
