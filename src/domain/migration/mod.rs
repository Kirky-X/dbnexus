// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 数据库迁移模块
//!
//! # Modules
//!
//! - [`metadata`] - 元数据跟踪
//! - [`sql_reverser`] - SQL逆向生成器
//! - [`column_changes`] - 列变更操作
//! - [`differ`] - Schema差异检测
//! - [`types`] - 类型定义
//! - [`schema`] - Schema解析器
//! - [`executor`] - 迁移执行器

pub mod column_changes;
pub mod differ;
pub mod executor;
pub mod metadata;
pub mod schema;
/// SQL逆向生成器模块
pub mod sql_reverser;
pub mod types;

pub use column_changes::ColumnChangeType;
pub use differ::*;
pub use executor::*;
pub use metadata::*;
pub use schema::*;
pub use sql_reverser::*;
pub use types::*;
