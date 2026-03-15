// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DbEntity 宏测试套件
//!
//! 测试 `#[derive(DbEntity)]` 派生宏的正确性

use trybuild::TestCases;

/// 测试 DbEntity 宏所有编译通过的场景
///
/// 包含：
/// - 基本展开
/// - sea_orm 属性
/// - 泛型结构体
/// - 方法签名验证
#[test]
fn db_entity_pass_tests() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_basic.rs");
    t.pass("tests/ui/db_entity_sea_orm.rs");
    t.pass("tests/ui/db_entity_generic.rs");
    t.pass("tests/ui/db_entity_method_signatures.rs");
}

/// 测试 DbEntity 宏所有编译失败的场景
///
/// 包含：
/// - 缺少 table_name 属性
/// - 缺少 primary_key 属性
#[test]
fn db_entity_compile_fail_tests() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/db_entity_missing_table_name.rs");
    t.compile_fail("tests/ui/db_entity_missing_primary_key.rs");
}
