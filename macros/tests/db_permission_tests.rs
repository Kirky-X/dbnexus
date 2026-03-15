// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! db_permission 宏测试套件
//!
//! 测试 `#[db_permission]` 属性宏的正确性

use trybuild::TestCases;

/// 测试 db_permission 宏所有编译通过的场景
///
/// 包含：
/// - 基本展开
/// - 方法签名验证
/// - DbEntity 组合
/// - 多角色多操作
/// - 有效角色名格式
#[test]
fn db_permission_pass_tests() {
    let t = TestCases::new();
    t.pass("tests/ui/db_permission_basic.rs");
    t.pass("tests/ui/db_permission_method_signatures.rs");
    t.pass("tests/ui/db_permission_with_db_entity.rs");
    t.pass("tests/ui/db_permission_multiple_roles_ops.rs");
    t.pass("tests/ui/db_permission_valid_role_names.rs");
}

/// 测试 db_permission 宏所有编译失败的场景
///
/// 包含：
/// - 无效角色名
/// - 角色名以数字开头
/// - 角色名包含连字符
/// - 空角色名
/// - 危险配置路径
/// - 路径遍历攻击
/// - 属性参数过长
#[test]
fn db_permission_compile_fail_tests() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/db_permission_invalid_role_name.rs");
    t.compile_fail("tests/ui/db_permission_role_starts_with_digit.rs");
    t.compile_fail("tests/ui/db_permission_role_with_hyphen.rs");
    t.compile_fail("tests/ui/db_permission_empty_role.rs");
    t.compile_fail("tests/ui/db_permission_dangerous_config_path.rs");
    t.compile_fail("tests/ui/db_permission_path_traversal.rs");
    t.compile_fail("tests/ui/db_permission_attribute_too_long.rs");
}
