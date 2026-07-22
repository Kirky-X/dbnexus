// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `#[db_entity]` 宏主键类型泛型化回归测试（0.4.2 新增）
//!
//! 验证 `find_by_id` / `find_by_ids` / `delete` / `force_delete` 在不同主键类型下的编译期与运行时正确性：
//! - `i64` 主键：默认整数主键，向后兼容
//! - `uuid::Uuid` 主键：修复 0.4.1 中 `the trait bound 'uuid::Uuid: From<i64>' is not satisfied` 的关键场景
//! - `String` 主键：覆盖业务字符串主键
//!
//! # 运行方式
//!
//! ```bash
//! cargo test --test db_entity_pk_types_test --features "sqlite,macros,permission,with-uuid,with-time"
//! ```

#![cfg(all(feature = "macros", feature = "with-uuid", feature = "with-time"))]

use dbnexus::db_entity;
use dbnexus::sea_orm::ActiveValue;
use dbnexus::sea_orm::entity::prelude::*;

// ============================================================================
// i64 主键实体（向后兼容基线）
// ============================================================================

mod i64_entity {
    use super::*;

    /// 整型主键实体：验证宏泛型化后旧调用代码无需任何修改
    ///
    /// 注意：`#[db_entity]` 宏自动生成 `impl ActiveModelBehavior for ActiveModel {}`，
    /// 无需手写（与 examples/src/common/entities.rs 写法一致）。
    #[db_entity(table_name = "users_i64", primary_key = "id")]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users_i64")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}

// ============================================================================
// Uuid 主键实体（0.4.1 回归 bug 关键场景）
// ============================================================================

mod uuid_entity {
    use super::*;

    /// Uuid 主键实体：0.4.1 中编译失败的回归用例
    ///
    /// 在 0.4.1 中宏生成的 `find_by_id(pk: i64)` 与 `Uuid` 主键不兼容，
    /// 报错：`the trait bound 'uuid::Uuid: From<i64>' is not satisfied`。
    #[db_entity(table_name = "users_uuid", primary_key = "id")]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users_uuid")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: uuid::Uuid,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}

// ============================================================================
// String 主键实体（覆盖字符串主键场景）
// ============================================================================

mod string_entity {
    use super::*;

    /// 字符串主键实体：覆盖业务场景（如订单号、slug 等）
    #[db_entity(table_name = "users_string", primary_key = "code")]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users_string")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub code: String,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}

// ============================================================================
// 测试用例
// ============================================================================

/// i64 主键实体：编译期验证 + 元数据方法验证
#[test]
fn test_i64_pk_entity_compiles() {
    let model = i64_entity::Model {
        id: 1,
        name: "alice".to_string(),
    };
    assert_eq!(model.id, 1);
    assert_eq!(model.name, "alice");
    // 宏生成的元数据方法
    assert_eq!(i64_entity::Model::table_name(), "users_i64");
    assert_eq!(i64_entity::Model::primary_key_column(), "id");
}

/// i64 主键 ActiveModel 行为验证
#[test]
fn test_i64_pk_active_model() {
    let active = i64_entity::ActiveModel {
        id: ActiveValue::Set(42),
        name: ActiveValue::Set("bob".to_string()),
    };
    assert!(active.id.is_set());
    assert!(active.name.is_set());
}

/// Uuid 主键实体：0.4.1 回归关键用例
///
/// 此用例在 0.4.1 中编译失败，0.4.2 修复后应通过。
#[test]
fn test_uuid_pk_entity_compiles() {
    let id = uuid::Uuid::new_v4();
    let model = uuid_entity::Model {
        id,
        name: "carol".to_string(),
    };
    assert_eq!(model.id, id);
    assert_eq!(model.name, "carol");
    assert_eq!(uuid_entity::Model::table_name(), "users_uuid");
    assert_eq!(uuid_entity::Model::primary_key_column(), "id");
}

/// Uuid 主键 ActiveModel 行为验证
#[test]
fn test_uuid_pk_active_model() {
    let id = uuid::Uuid::new_v4();
    let active = uuid_entity::ActiveModel {
        id: ActiveValue::Set(id),
        name: ActiveValue::Set("dave".to_string()),
    };
    assert!(active.id.is_set());
    assert!(active.name.is_set());
}

/// String 主键实体：覆盖字符串主键场景
#[test]
fn test_string_pk_entity_compiles() {
    let model = string_entity::Model {
        code: "USER001".to_string(),
        name: "eve".to_string(),
    };
    assert_eq!(model.code, "USER001");
    assert_eq!(model.name, "eve");
    assert_eq!(string_entity::Model::table_name(), "users_string");
    assert_eq!(string_entity::Model::primary_key_column(), "code");
}

/// String 主键 ActiveModel 行为验证
#[test]
fn test_string_pk_active_model() {
    let active = string_entity::ActiveModel {
        code: ActiveValue::Set("USER002".to_string()),
        name: ActiveValue::Set("frank".to_string()),
    };
    assert!(active.code.is_set());
    assert!(active.name.is_set());
}

/// 验证 `find_by_id` 函数签名存在且参数类型为泛型
///
/// 这个测试通过引用函数本身来验证签名泛型化成功——如果宏仍然写死 `pk: i64`，
/// 引用 `uuid_entity::Model::find_by_id` 时类型签名会与 Uuid 主键冲突，编译失败。
#[test]
fn test_find_by_id_signature_is_generic() {
    // 仅取函数指针，不调用——验证函数本身存在且签名泛型化
    let _ = uuid_entity::Model::find_by_id::<uuid::Uuid>;
    let _ = i64_entity::Model::find_by_id::<i64>;
    let _ = string_entity::Model::find_by_id::<String>;
}

/// 验证 `find_by_ids` 函数签名存在且参数类型为泛型
#[test]
fn test_find_by_ids_signature_is_generic() {
    let _ = uuid_entity::Model::find_by_ids::<uuid::Uuid>;
    let _ = i64_entity::Model::find_by_ids::<i64>;
    let _ = string_entity::Model::find_by_ids::<String>;
}

/// 验证 `delete` 函数签名存在且参数类型为泛型
#[test]
fn test_delete_signature_is_generic() {
    let _ = uuid_entity::Model::delete::<uuid::Uuid>;
    let _ = i64_entity::Model::delete::<i64>;
    let _ = string_entity::Model::delete::<String>;
}

// ============================================================================
// soft_delete=true Uuid 主键实体（覆盖 MEDIUM-2：delete/force_delete 签名测试）
// ============================================================================

mod soft_delete_uuid_entity {
    use super::*;

    /// soft_delete=true 的 Uuid 主键实体：验证 soft_delete=true 分支下
    /// `delete` 和 `force_delete` 的泛型化签名（约束为 `Into<sea_orm::Value>`）。
    ///
    /// `#[db_entity]` 宏在 soft_delete=true 时自动注入 `deleted_at: Option<OffsetDateTime>` 字段
    /// （Task 6.5），并生成 `force_delete` 方法。
    #[db_entity(table_name = "soft_delete_uuid", primary_key = "id", soft_delete = true)]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "soft_delete_uuid")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: uuid::Uuid,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}

/// 验证 soft_delete=true 分支下 `delete` 函数签名泛型化（约束为 `Into<sea_orm::Value>`）
///
/// 覆盖 MEDIUM-2：soft_delete=true 分支的 delete 签名此前未被测试覆盖。
#[test]
fn test_soft_delete_delete_signature_is_generic() {
    let _ = soft_delete_uuid_entity::Model::delete::<uuid::Uuid>;
    let _ = soft_delete_uuid_entity::Model::delete::<i64>;
    let _ = soft_delete_uuid_entity::Model::delete::<String>;
}

/// 验证 `force_delete` 函数签名存在且参数类型为泛型（仅 soft_delete=true 实体生成）
///
/// 覆盖 MEDIUM-2：force_delete 签名此前未被测试覆盖。
#[test]
fn test_force_delete_signature_is_generic() {
    let _ = soft_delete_uuid_entity::Model::force_delete::<uuid::Uuid>;
    let _ = soft_delete_uuid_entity::Model::force_delete::<i64>;
    let _ = soft_delete_uuid_entity::Model::force_delete::<String>;
}

// ============================================================================
// cache_key 泛型化测试（0.4.3：cache_key 从 i64 改为 Display 泛型）
// ============================================================================

mod cache_uuid_entity {
    use super::*;

    /// 启用 cache 的 Uuid 主键实体：验证 cache_key 泛型化后支持 Uuid 主键。
    ///
    /// 0.4.2 中 cache_key(id: i64) 硬编码 i64，Uuid 主键实体调用 cache_key 编译失败。
    /// 0.4.3 修复为 `cache_key<PK: Display>(id: PK)`。
    #[db_entity(
        table_name = "cache_uuid",
        primary_key = "id",
        cache(ttl = 60, strategy = "lru", max_capacity = 100)
    )]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "cache_uuid")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: uuid::Uuid,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}

/// 验证 cache_key 泛型化后支持 Uuid 主键（0.4.3 修复）
#[test]
fn test_cache_key_accepts_uuid() {
    let id = uuid::Uuid::nil();
    let key = cache_uuid_entity::Model::cache_key(id);
    assert_eq!(key, format!("cache_uuid:{}", id));
}

/// 验证 cache_key 泛型化后仍向后兼容 i64（0.4.2 基线）
#[test]
fn test_cache_key_backward_compatible_i64() {
    let key = cache_uuid_entity::Model::cache_key(42i64);
    assert_eq!(key, "cache_uuid:42");
}

/// 验证 cache_key 泛型化后支持 String 主键
#[test]
fn test_cache_key_accepts_string() {
    let key = cache_uuid_entity::Model::cache_key("USER001".to_string());
    assert_eq!(key, "cache_uuid:USER001");
}
