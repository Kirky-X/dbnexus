// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! 共享示例实体
//!
//! 定义跨示例复用的实体（`user::Model`、`article::Model`），避免每个示例都重复声明
//! `#[derive(DbEntity, DeriveEntityModel)]` + `#[db_crud]` 样板。
//!
//! 实体使用顶层 prelude 风格 API（`dbnexus::DbEntity` / `dbnexus::db_crud`），
//! 与根级示例保持一致。
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

use dbnexus::{DbEntity, db_crud};
use sea_orm::entity::prelude::*;

// ============================================
// User 实体
// ============================================

/// 用户实体模块
pub mod user {
    use super::*;

    /// 用户实体模型
    ///
    /// 使用 `#[derive(DbEntity, DeriveEntityModel)]` 同时获得：
    /// - sea-orm 的 EntityModel 实现（Entity/ActiveModel/Column 等）
    /// - dbnexus 的 `table_name()` / `primary_key_column()` 辅助方法
    ///
    /// `#[db_crud(table_name = "users")]` 属性宏自动生成带权限检查的 CRUD 方法。
    #[derive(Clone, Debug, PartialEq, DbEntity, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    #[db_crud(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub name: String,
        pub email: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================
// Article 实体
// ============================================

/// 文章实体模块
pub mod article {
    use super::*;

    /// 文章实体模型
    ///
    /// 用于演示 `db_cache` 宏等需要文章表场景的示例。
    #[derive(Clone, Debug, PartialEq, DbEntity, DeriveEntityModel)]
    #[sea_orm(table_name = "articles")]
    #[db_crud(table_name = "articles")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub title: String,
        pub content: String,
        pub author: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
