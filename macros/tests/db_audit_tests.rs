// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! db_audit 宏测试套件
//!
//! 测试 `#[db_audit]` 属性宏的正确性

use trybuild::TestCases;

/// 测试 db_audit 宏所有编译通过的场景
///
/// 包含：
/// - 基本展开
/// - log_values = true
/// - log_values = false
/// - 常量生成
/// - DbEntity 组合
/// - 默认值验证
/// - db_crud 组合
/// - 完整配置组合
#[test]
fn db_audit_pass_tests() {
    let t = TestCases::new();
    t.pass("tests/ui/db_audit_basic.rs");
    t.pass("tests/ui/db_audit_log_values_true.rs");
    t.pass("tests/ui/db_audit_log_values_false.rs");
    t.pass("tests/ui/db_audit_constants.rs");
    t.pass("tests/ui/db_audit_with_db_entity.rs");
    t.pass("tests/ui/db_audit_default_values.rs");
    t.pass("tests/ui/db_audit_with_db_crud.rs");
    t.pass("tests/ui/db_audit_full_combination.rs");
}
