// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 实体转换模块
//!
//! 提供实体转换工具和类型重导出
//!
//! # 重新导出的类型
//!
//! 这些类型由 `#[derive(DbEntity)]` 宏自动生成，用户需要使用它们来定义实体。
//!
//! - [`EntityTrait`] - 实体 trait，由宏自动实现
//! - [`ActiveModelTrait`] - ActiveModel trait，用于修改实体
//! - [`Condition`] - 查询条件构建器
//! - `Set<T>` - 值设置辅助类型，用于构建 [`ActiveModel`]

// 只暴露用户必需的类型
pub use sea_orm::entity::prelude::{ActiveModelTrait, EntityTrait};
pub use sea_orm::{Condition, Set};
