// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! sea-orm 类型特性桥接测试
//!
//! 验证 `with-chrono` 和 `with-uuid` 特性是否正确桥接 sea-orm 的类型支持：
//! - `with-chrono`: 启用后可在 sea-orm 实体中使用 `chrono::DateTime<Utc>` 等类型
//! - `with-uuid`: 启用后可在 sea-orm 实体中使用 `uuid::Uuid` 类型
//!
//! # 运行方式
//!
//! ```bash
//! cargo test --test sea_orm_type_tests --features "sqlite,macros,with-chrono,with-uuid"
//! ```

#![cfg(all(feature = "with-chrono", feature = "with-uuid", feature = "macros"))]

use sea_orm::ActiveValue;

// ============================================================================
// chrono 类型实体测试（with-chrono 特性）
// ============================================================================

mod chrono_entity {
    use sea_orm::entity::prelude::*;

    /// 测试实体：使用 chrono::DateTime<Utc> 作为字段类型
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "events_with_chrono")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub event_name: String,
        /// chrono::DateTime<Utc> 字段（需要 with-chrono 特性桥接 sea-orm/with-chrono）
        pub created_at: chrono::DateTime<chrono::Utc>,
        pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 测试 chrono 类型实体能编译并实例化
#[test]
fn test_chrono_entity_compiles() {
    let now = chrono::Utc::now();
    let model = chrono_entity::Model {
        id: 1,
        event_name: "test_event".to_string(),
        created_at: now,
        updated_at: None,
    };
    assert_eq!(model.id, 1);
    assert_eq!(model.event_name, "test_event");
    assert!(model.updated_at.is_none());
    assert_eq!(model.created_at, now);
}

/// 测试 chrono 类型的 ActiveModel 能正确设置字段
#[test]
fn test_chrono_active_model_set() {
    let now = chrono::Utc::now();
    let active = chrono_entity::ActiveModel {
        id: ActiveValue::NotSet,
        event_name: ActiveValue::Set("test".to_string()),
        created_at: ActiveValue::Set(now),
        updated_at: ActiveValue::Set(Some(now)),
    };
    // 验证 ActiveValue 已设置（is_set 返回 true）
    assert!(active.event_name.is_set());
    assert!(active.created_at.is_set());
    assert!(active.updated_at.is_set());
}

/// 测试 chrono::DateTime 格式化（验证 sea-orm ValueType trait 实现）
#[test]
fn test_chrono_datetime_serialization() {
    let now = chrono::Utc::now();
    let model = chrono_entity::Model {
        id: 1,
        event_name: "serialize_test".to_string(),
        created_at: now,
        updated_at: Some(now),
    };
    // 验证 chrono 类型能被格式化（sea-orm ValueType 要求 Display/Debug）
    assert!(!format!("{:?}", model.created_at).is_empty());
    assert!(!format!("{}", model.created_at).is_empty());
}

// ============================================================================
// uuid 类型实体测试（with-uuid 特性）
// ============================================================================

mod uuid_entity {
    use sea_orm::entity::prelude::*;

    /// 测试实体：使用 uuid::Uuid 作为字段类型
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "resources_with_uuid")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: uuid::Uuid,
        pub resource_name: String,
        pub parent_id: Option<uuid::Uuid>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 测试 uuid 类型实体能编译并实例化
#[test]
fn test_uuid_entity_compiles() {
    let id = uuid::Uuid::new_v4();
    let model = uuid_entity::Model {
        id,
        resource_name: "test_resource".to_string(),
        parent_id: None,
    };
    assert_eq!(model.id, id);
    assert_eq!(model.resource_name, "test_resource");
    assert!(model.parent_id.is_none());
}

/// 测试 uuid 类型的 ActiveModel 能正确设置字段
#[test]
fn test_uuid_active_model_set() {
    let id = uuid::Uuid::new_v4();
    let active = uuid_entity::ActiveModel {
        id: ActiveValue::Set(id),
        resource_name: ActiveValue::Set("test".to_string()),
        parent_id: ActiveValue::Set(None),
    };
    // 验证 ActiveValue 已设置（is_set 返回 true）
    assert!(active.id.is_set());
    assert!(active.resource_name.is_set());
    assert!(active.parent_id.is_set());
}

/// 测试 uuid::Uuid 格式化（验证 sea-orm ValueType trait 实现）
#[test]
fn test_uuid_serialization() {
    let id = uuid::Uuid::new_v4();
    let model = uuid_entity::Model {
        id,
        resource_name: "serialize_test".to_string(),
        parent_id: Some(id),
    };
    // 验证 uuid 类型能被格式化（sea-orm ValueType 要求 Display/Debug）
    assert!(!format!("{:?}", model.id).is_empty());
    assert!(!format!("{}", model.id).is_empty());
}

// ============================================================================
// 混合类型实体测试（with-chrono + with-uuid 同时使用）
// ============================================================================

mod mixed_entity {
    use sea_orm::entity::prelude::*;

    /// 测试实体：同时使用 chrono 和 uuid 类型字段
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "mixed_types_entity")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: uuid::Uuid,
        pub name: String,
        pub created_at: chrono::DateTime<chrono::Utc>,
        pub modified_at: Option<chrono::DateTime<chrono::Utc>>,
        pub related_id: Option<uuid::Uuid>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// 测试 chrono + uuid 混合类型实体能编译并实例化
#[test]
fn test_mixed_types_entity_compiles() {
    let id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();
    let model = mixed_entity::Model {
        id,
        name: "mixed_test".to_string(),
        created_at: now,
        modified_at: Some(now),
        related_id: None,
    };
    assert_eq!(model.id, id);
    assert_eq!(model.created_at, now);
    assert!(model.related_id.is_none());
}

/// 测试混合类型的 ActiveModel 能正确设置所有字段
#[test]
fn test_mixed_types_active_model() {
    let id = uuid::Uuid::new_v4();
    let related = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();
    let active = mixed_entity::ActiveModel {
        id: ActiveValue::Set(id),
        name: ActiveValue::Set("mixed".to_string()),
        created_at: ActiveValue::Set(now),
        modified_at: ActiveValue::Set(Some(now)),
        related_id: ActiveValue::Set(Some(related)),
    };
    // 验证所有字段都已设置
    assert!(active.id.is_set());
    assert!(active.name.is_set());
    assert!(active.created_at.is_set());
    assert!(active.modified_at.is_set());
    assert!(active.related_id.is_set());
}
