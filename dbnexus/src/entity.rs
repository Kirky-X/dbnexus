// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 实体转换模块
//!
//! 提供实体转换工具和类型重导出

pub use sea_orm::entity::prelude::{
    ActiveModelBehavior, ActiveModelTrait, DeriveActiveModel, DeriveIntoActiveModel, EntityTrait, Iden, RelationTrait,
};

pub use sea_orm::{Condition, Set};
