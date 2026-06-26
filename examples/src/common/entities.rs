// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! 共享示例实体
//!
//! 定义跨示例复用的实体（`user::Model`、`article::Model`），避免每个示例都重复声明
//! `#[db_entity(...)]` + `#[derive(DeriveEntityModel)]` 样板。
//!
//! 实体使用顶层 prelude 风格 API（`dbnexus::db_entity`），与根级示例保持一致。
//!
//! # 访问方式
//!
//! ```ignore
//! use common::entities::user::Model as User;
//! use common::entities::article::Model as Article;
//! ```
//!
//! 每个 entity 必须放在独立子模块中，因为 `DeriveEntityModel` 宏会生成
//! 固定名称的 `ActiveModel` / `Relation` / `Entity` / `Column` 等关联项。

use dbnexus::db_entity;
use sea_orm::entity::prelude::*;

// ============================================
// User 实体
// ============================================

/// 用户实体模块
pub mod user {
    use super::*;

    /// 用户实体模型
    ///
    /// 使用 `#[db_entity(table_name = "users", primary_key = "id")]` 统一属性宏获得：
    /// - sea-orm 的 EntityModel 实现（Entity/ActiveModel/Column 等，由 DeriveEntityModel 生成）
    /// - dbnexus 的 `table_name()` / `primary_key_column()` 辅助方法
    /// - 8 个带权限检查的 CRUD 方法（insert/find_by_id/update/delete/find_all/...）
    /// - `impl ActiveModelBehavior for ActiveModel`（宏自动生成，用户无需手写）
    #[db_entity(table_name = "users", primary_key = "id")]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub name: String,
        pub email: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}

// ============================================
// Article 实体
// ============================================

/// 文章实体模块
pub mod article {
    use super::*;

    /// 文章实体模型
    ///
    /// 用于演示 `db_entity` 宏的 cache 子参数等场景。
    #[db_entity(table_name = "articles", primary_key = "id")]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "articles")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub title: String,
        pub content: String,
        pub author: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}
