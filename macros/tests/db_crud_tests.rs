// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! db_crud 宏测试套件
//!
//! 测试 `#[db_crud]` 属性宏的正确性

use trybuild::TestCases;

/// 测试 db_crud 宏所有编译通过的场景
///
/// 包含：
/// - 基本展开
/// - table_name 参数
/// - 方法签名验证
/// - sea_orm 组合
/// - 所有 CRUD 方法
/// - 泛型结构体
#[test]
fn db_crud_pass_tests() {
    let t = TestCases::new();
    t.pass("tests/ui/db_crud_basic.rs");
    t.pass("tests/ui/db_crud_table_name_arg.rs");
    t.pass("tests/ui/db_crud_method_signatures.rs");
    t.pass("tests/ui/db_crud_with_sea_orm.rs");
    t.pass("tests/ui/db_crud_all_methods.rs");
    t.pass("tests/ui/db_crud_generic.rs");
}

/// 测试 db_crud 宏编译失败的场景
///
/// 包含：
/// - 缺少 table_name 参数
#[test]
fn db_crud_compile_fail_tests() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/db_crud_missing_table_name.rs");
}
