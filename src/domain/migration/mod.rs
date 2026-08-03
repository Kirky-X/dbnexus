// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 数据库迁移模块
//!
//! # Modules
//!
//! - [`metadata`] - 元数据跟踪
//! - [`sql_reverser`] - SQL逆向生成器（需要 `migration` feature）
//! - [`column_changes`] - 列变更操作
//! - [`differ`] - Schema差异检测（需要 `migration` feature）
//! - [`types`] - 类型定义
//! - [`schema`] - Schema解析器
//! - [`executor`] - 迁移执行器（需要 `migration` feature）
//! - [`converter`] - Sea-ORM TableCreateStatement 转换器
//!
//! # Feature Gating
//!
//! - 基础模块（`schema`/`converter`/`types`/`metadata`/`column_changes`）始终可用，
//!   供 `#[db_entity]` 宏生成的 `schema()` 方法使用。
//! - 执行模块（`executor`/`differ`/`sql_reverser`）需要 `migration` feature，
//!   因为它们依赖 `sqlparser` 和 `regex`。

pub mod column_changes;
pub mod converter;
#[cfg(feature = "migration")]
pub mod differ;
#[cfg(feature = "migration")]
pub mod executor;
pub mod metadata;
pub mod schema;
/// SQL逆向生成器模块
#[cfg(feature = "migration")]
pub mod sql_reverser;
pub mod types;

/// 分片迁移编排器（shard-migration feature）
#[cfg(feature = "shard-migration")]
pub mod shard_orchestrator;

pub use column_changes::ColumnChangeType;
pub use converter::*;
#[cfg(feature = "migration")]
pub use differ::*;
#[cfg(feature = "migration")]
pub use executor::*;
pub use metadata::*;
pub use schema::*;
#[cfg(feature = "migration")]
pub use sql_reverser::*;
pub use types::*;

#[cfg(feature = "shard-migration")]
pub use shard_orchestrator::{OrchestratedMigrationResult, ShardMigrationOrchestrator, ShardMigrationResult};
