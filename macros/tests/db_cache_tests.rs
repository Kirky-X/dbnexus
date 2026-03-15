// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! db_cache 宏测试套件
//!
//! 测试 `#[db_cache]` 属性宏的正确性

use trybuild::TestCases;

/// 测试 db_cache 宏所有编译通过的场景
///
/// 包含：
/// - 基本展开
/// - 自定义 TTL
/// - 自定义策略
/// - 自定义容量
/// - 方法签名验证
/// - 所有参数组合
/// - DbEntity 组合
/// - 默认值验证
/// - 常量生成
#[test]
fn db_cache_pass_tests() {
    let t = TestCases::new();
    t.pass("tests/ui/db_cache_basic.rs");
    t.pass("tests/ui/db_cache_custom_ttl.rs");
    t.pass("tests/ui/db_cache_custom_strategy.rs");
    t.pass("tests/ui/db_cache_custom_capacity.rs");
    t.pass("tests/ui/db_cache_method_signatures.rs");
    t.pass("tests/ui/db_cache_all_params.rs");
    t.pass("tests/ui/db_cache_with_db_entity.rs");
    t.pass("tests/ui/db_cache_default_values.rs");
    t.pass("tests/ui/db_cache_constants.rs");
}
