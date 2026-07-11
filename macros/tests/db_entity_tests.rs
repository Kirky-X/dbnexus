// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! db_entity 宏测试套件
//!
//! 测试 `#[db_entity(...)]` 统一属性宏的正确性

use trybuild::TestCases;

/// 测试 db_entity 宏所有编译通过的场景
#[test]
fn db_entity_pass_tests() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_basic.rs");
    t.pass("tests/ui/db_entity_with_permissions.rs");
    t.pass("tests/ui/db_entity_with_cache.rs");
    t.pass("tests/ui/db_entity_with_audit.rs");
}

/// 测试 db_entity 宏所有编译失败的场景
#[test]
fn db_entity_compile_fail_tests() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/db_entity_missing_table_name.rs");
    t.compile_fail("tests/ui/db_entity_missing_primary_key.rs");
}
