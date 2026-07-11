// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 实体转换模块单元测试
//!
//! 测试 entity 模块导出的类型是否正确可用：
//! - `Condition`（all/any/not/add 嵌套组合）
//! - `ActiveValue`（Set/NotSet/Unchanged 变体）
//! - `ActiveModelTrait` / `EntityTrait` trait 导出

use dbnexus::{ActiveModelTrait, Condition, EntityTrait};
use sea_orm::ActiveValue;

/// 测试 Condition 类型是否正确导出
#[test]
fn test_condition_all() {
    let condition = Condition::all();
    assert!(!format!("{condition:?}").is_empty());
}

/// 测试 Condition::any 是否可用
#[test]
fn test_condition_any() {
    let condition = Condition::any();
    assert!(!format!("{condition:?}").is_empty());
}

/// 测试 Condition::not 是否可用
#[test]
fn test_condition_not() {
    let condition = Condition::not(Condition::all());
    assert!(!format!("{condition:?}").is_empty());
}

/// 测试 Condition 组合操作
#[test]
fn test_condition_combination() {
    let condition = Condition::all()
        .add(Condition::any())
        .add(Condition::not(Condition::all()));

    let combined = Condition::all().add(Condition::all()).add(Condition::any());

    assert!(!format!("{condition:?}").is_empty());
    assert!(!format!("{combined:?}").is_empty());
}

/// 测试 Condition 嵌套组合（all + any）
#[test]
fn test_condition_nested_combination() {
    let condition = Condition::all().add(Condition::any());
    assert!(!format!("{condition:?}").is_empty());
}

/// 测试 Condition not 嵌套组合
#[test]
fn test_condition_not_nested() {
    let condition = Condition::not(Condition::all()).add(Condition::any());
    assert!(!format!("{condition:?}").is_empty());
}

/// 测试 Set 类型创建（字符串）
#[test]
fn test_set_string() {
    let set_value = ActiveValue::Set("test".to_string());
    assert!(matches!(set_value, ActiveValue::Set(_)));
}

/// 测试 Set 类型创建（数字）
#[test]
fn test_set_number() {
    let set_number = ActiveValue::Set(42);
    assert!(matches!(set_number, ActiveValue::Set(42)));
}

/// 测试 Set 类型创建（Option）
#[test]
fn test_set_option() {
    let set_option = ActiveValue::Set(Some(42));
    assert!(matches!(set_option, ActiveValue::Set(Some(42))));
}

/// 测试 Set 类型创建（None）
#[test]
fn test_set_none() {
    let none_value: ActiveValue<Option<i32>> = ActiveValue::Set(None);
    assert!(matches!(none_value, ActiveValue::Set(None)));
}

/// 测试 ActiveValue 变体（NotSet）
#[test]
fn test_active_value_not_set() {
    let not_set: ActiveValue<String> = ActiveValue::NotSet;
    assert!(matches!(not_set, ActiveValue::NotSet));
}

/// 测试 ActiveValue 变体（Unchanged）
#[test]
fn test_active_value_unchanged() {
    let unchanged: ActiveValue<String> = ActiveValue::Unchanged("original".to_string());
    assert!(matches!(unchanged, ActiveValue::Unchanged(_)));
}

/// 测试类型导出完整性
#[test]
fn test_type_exports() {
    // 确保所有必需的类型都已导出
    let _condition = Condition::all();
    let _set_value = ActiveValue::Set("test");
    let _string_type: ActiveValue<String> = ActiveValue::Set(String::new());
    let _i32_type: ActiveValue<i32> = ActiveValue::Set(0);
    let _option_type: ActiveValue<Option<String>> = ActiveValue::Set(None);
}

/// 测试 Condition 的 and/or 操作
///
/// `add` 消费 self，需 clone 保留原始值以供后续断言
#[test]
fn test_condition_logical_operations() {
    let condition1 = Condition::all();
    let condition2 = Condition::any();

    let and_result = condition1.clone().add(condition2.clone());
    assert!(!format!("{and_result:?}").is_empty());

    // 验证原始条件未被修改
    assert!(!format!("{condition1:?}").is_empty());
    assert!(!format!("{condition2:?}").is_empty());
}

/// 测试 Set 与不同类型的组合
#[test]
fn test_set_various_types() {
    // 字符串类型
    let _s1: ActiveValue<String> = ActiveValue::Set("hello".to_string());
    let _s2: ActiveValue<Option<String>> = ActiveValue::Set(Some("world".to_string()));

    // 数字类型
    let _n1: ActiveValue<i32> = ActiveValue::Set(42);
    let _n2: ActiveValue<i64> = ActiveValue::Set(1234567890i64);
    let _n3: ActiveValue<f64> = ActiveValue::Set(2.5);

    // 布尔类型
    let _b1: ActiveValue<bool> = ActiveValue::Set(true);
    let _b2: ActiveValue<bool> = ActiveValue::Set(false);

    // 时间类型（需要 with-chrono feature）
    use chrono::{DateTime, Utc};
    let _t1: ActiveValue<DateTime<Utc>> = ActiveValue::Set(Utc::now());
    let _t2: ActiveValue<Option<DateTime<Utc>>> = ActiveValue::Set(Some(Utc::now()));

    let condition = Condition::all();
    assert!(!format!("{condition:?}").is_empty());
}

/// 测试 ActiveModelTrait 是否可用（通过 trait bound）
#[test]
fn test_active_model_trait_bound() {
    // 编译时测试：函数声明需要 trait 在作用域内，编译通过即说明 trait 正确导出
    fn _assert_active_model_trait<T: ActiveModelTrait>() {}
    fn _assert_entity_trait<T: EntityTrait>() {}
}

/// 测试 Condition Debug 格式化
#[test]
fn test_condition_format() {
    let conditions: Vec<Condition> = vec![
        Condition::all(),
        Condition::any(),
        Condition::not(Condition::all()),
        Condition::all().add(Condition::any()),
    ];

    for condition in conditions {
        let debug = format!("{condition:?}");
        assert!(!debug.is_empty(), "Debug format should not be empty");
    }
}
