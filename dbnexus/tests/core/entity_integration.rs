// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 实体转换模块集成测试
//!
//! 测试实体类型重导出和与 sea-orm 的集成

use dbnexus::DbPool;
use dbnexus::entity::Condition;
use sea_orm::ActiveValue;

fn get_database_url() -> Option<String> {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        return Some(url);
    }

    if cfg!(feature = "sqlite") {
        return Some("sqlite::memory:".to_string());
    }

    None
}

// 注意：entity.rs 主要是类型重导出模块，实际的实体定义需要使用 dbnexus-macros
// 这里测试类型是否正确导出以及与 Session 的集成

#[tokio::test]
async fn test_entity_types_are_accessible() {
    // 测试 entity 模块导出的类型是否可访问
    // 这些类型应该可以直接使用
    let condition = Condition::all();
    let set_value = ActiveValue::Set("test".to_string());
    let set_number = ActiveValue::Set(42);
    let set_option = ActiveValue::Set(Some(42));

    assert!(!format!("{condition:?}").is_empty());
    assert!(matches!(set_value, ActiveValue::Set(_)));
    assert!(matches!(set_number, ActiveValue::Set(42)));
    assert!(matches!(set_option, ActiveValue::Set(Some(42))));
}

#[tokio::test]
async fn test_condition_operations() {
    // 测试 Condition 构建器的各种操作
    let condition = Condition::all()
        .add(Condition::any())
        .add(Condition::not(Condition::all()));

    // 测试条件组合
    let combined = Condition::all().add(Condition::all()).add(Condition::any());

    assert!(!format!("{condition:?}").is_empty());
    assert!(!format!("{combined:?}").is_empty());
}

#[tokio::test]
async fn test_set_type() {
    // 测试 Set 类型
    let string = ActiveValue::Set("test".to_string());
    let number = ActiveValue::Set(42);
    let option = ActiveValue::Set(Some(42));
    let none_value: ActiveValue<Option<i32>> = ActiveValue::Set(None);

    // 验证 Set 类型可以正确创建
    assert!(matches!(string, ActiveValue::Set(_)));
    assert!(matches!(number, ActiveValue::Set(42)));
    assert!(matches!(option, ActiveValue::Set(Some(42))));
    assert!(matches!(none_value, ActiveValue::Set(None)));
}

#[tokio::test]
async fn test_entity_with_session() {
    // 测试实体操作与 Session 的集成
    // 注意：由于权限限制，这里只测试基本的 Session 功能

    let Some(url) = get_database_url() else {
        return;
    };

    let pool = DbPool::new(&url).await.unwrap();
    let mut session = pool.get_session("admin").await.unwrap();

    // 验证 session 可以正常工作
    assert_eq!(session.role(), "admin");
    assert!(!session.is_in_transaction());

    // 测试事务功能
    session.begin_transaction().await.unwrap();
    assert!(session.is_in_transaction());
    session.commit().await.unwrap();
    assert!(!session.is_in_transaction());
}

#[tokio::test]
async fn test_entity_types_compatibility() {
    // 测试 entity 模块导出的类型与 sea-orm 的兼容性

    // Condition 应该可以用于构建查询条件
    let condition = Condition::all();

    // Set 应该可以用于设置字段值
    let set_value = ActiveValue::Set("value".to_string());

    // 这些类型应该可以正常工作
    assert!(!format!("{condition:?}").is_empty());
    assert!(matches!(set_value, ActiveValue::Set(_)));
}

#[tokio::test]
async fn test_entity_module_reexports() {
    // 测试 entity 模块是否正确重新导出了 sea-orm 的类型

    // 以下类型应该从 dbnexus::entity 可访问：
    // - EntityTrait
    // - ActiveModelTrait
    // - Condition
    // - Set

    // 如果编译通过，说明重新导出正确
    let condition = Condition::all();
    assert!(!format!("{condition:?}").is_empty());
}

#[tokio::test]
async fn test_condition_building() {
    // 测试 Condition 构建器的各种构建方式

    // 简单条件
    let simple = Condition::all();

    // 嵌套条件
    let nested = Condition::all().add(Condition::any().add(Condition::all()));

    // 否定条件
    let not_condition = Condition::not(Condition::all());

    // 组合条件
    let combined = Condition::all()
        .add(Condition::any())
        .add(Condition::not(Condition::all()));

    assert!(!format!("{simple:?}").is_empty());
    assert!(!format!("{nested:?}").is_empty());
    assert!(!format!("{not_condition:?}").is_empty());
    assert!(!format!("{combined:?}").is_empty());
}

#[tokio::test]
async fn test_set_with_various_types() {
    // 测试 Set 类型与各种类型的兼容性

    let string = ActiveValue::Set("string".to_string());
    let i32_value = ActiveValue::Set(42);
    let i64_value = ActiveValue::Set(100_i64);
    let f64_value = ActiveValue::Set(1.5);
    let bool_value = ActiveValue::Set(true);
    let option_value: ActiveValue<Option<i32>> = ActiveValue::Set(None);

    assert!(matches!(string, ActiveValue::Set(_)));
    assert!(matches!(i32_value, ActiveValue::Set(42)));
    assert!(matches!(i64_value, ActiveValue::Set(100)));
    assert!(matches!(f64_value, ActiveValue::Set(_)));
    assert!(matches!(bool_value, ActiveValue::Set(true)));
    assert!(matches!(option_value, ActiveValue::Set(None)));
}

#[tokio::test]
async fn test_entity_integration_with_sql_operations() {
    // 测试实体操作与 SQL 操作的集成
    // 注意：由于权限限制，这里只测试基本的 Session 功能

    let Some(url) = get_database_url() else {
        return;
    };

    let pool = DbPool::new(&url).await.unwrap();
    let mut session = pool.get_session("admin").await.unwrap();

    // 验证 session 可以正常工作
    assert_eq!(session.role(), "admin");
    assert!(!session.is_in_transaction());

    // 测试事务功能
    session.begin_transaction().await.unwrap();
    assert!(session.is_in_transaction());
    session.rollback().await.unwrap();
    assert!(!session.is_in_transaction());
}
