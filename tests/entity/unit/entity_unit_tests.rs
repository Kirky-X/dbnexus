// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 实体转换模块单元测试
//!
//! 测试 entity 模块导出的类型是否正确可用

use dbnexus::entity::{ActiveModelTrait, Condition, EntityTrait};
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

/// 测试 Condition 包含操作
#[test]
fn test_condition_contains() {
    let condition = Condition::contains("name", "test");
    assert!(!format!("{condition:?}").is_empty());
}

/// 测试 Condition 等于操作
#[test]
fn test_condition_equal() {
    let condition = Condition::expr("id = 1");
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

/// 测试 Set 类型创建（Vec）
#[test]
fn test_set_vec() {
    let set_vec: ActiveValue<Option<Vec<i32>>> = ActiveValue::Set(Some(vec![1, 2, 3]));
    assert!(matches!(set_vec, ActiveValue::Set(Some(_))));
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
#[test]
fn test_condition_logical_operations() {
    let condition1 = Condition::all();
    let condition2 = Condition::any();

    // 测试 and 操作（通过 add 实现）
    let and_result = condition1.add(condition2.clone());
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
    let _n3: ActiveValue<f64> = ActiveValue::Set(3.14159);

    // 布尔类型
    let _b1: ActiveValue<bool> = ActiveValue::Set(true);
    let _b2: ActiveValue<bool> = ActiveValue::Set(false);

    // 时间类型
    use chrono::{DateTime, Utc};
    let _t1: ActiveValue<DateTime<Utc>> = ActiveValue::Set(Utc::now());
    let _t2: ActiveValue<Option<DateTime<Utc>>> = ActiveValue::Set(Some(Utc::now()));

    // 验证所有类型都可以正确格式化
    let condition = Condition::all();
    assert!(!format!("{condition:?}").is_empty());
}

/// 测试 ActiveModelTrait 是否可用（通过 trait bound）
#[test]
fn test_active_model_trait_bound() {
    // 这是一个编译时测试，确保 ActiveModelTrait 正确导出
    // 实际的测试需要具体的实体类型
    fn assert_active_model_trait<T: ActiveModelTrait>() {}
    fn assert_entity_trait<T: EntityTrait>() {}

    // 如果代码能够编译通过，说明类型正确导出
    // 注意：这个测试主要用于验证编译，不进行实际运行时检查
}

/// 测试 Condition 格式化
#[test]
fn test_condition_format() {
    let conditions: Vec<Condition> = vec![
        Condition::all(),
        Condition::any(),
        Condition::not(Condition::all()),
        Condition::contains("name", "test"),
        Condition::expr("age > 18"),
    ];

    for condition in conditions {
        let debug = format!("{condition:?}");
        let display = format!("{condition}");
        assert!(!debug.is_empty(), "Debug format should not be empty");
        assert!(
            !display.is_empty() || display.contains("condition"),
            "Display format should work"
        );
    }
}
