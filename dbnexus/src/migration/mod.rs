// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 数据库迁移模块
//!
//! 提供数据库迁移功能，包括迁移文件解析、执行、版本管理等

mod differ;
mod executor;
mod schema;
mod types;

// 精确导出公共 API，避免过度暴露内部实现
pub use differ::{MigrationCommand, MigrationDirection, MigrationPlan, SchemaDiffer, SqlGenerator};
pub use executor::{MigrationExecutor, MigrationFile, MigrationFileParser};
pub use schema::{
    Column, ColumnType, ForeignKey, ForeignKeyAction, Index, Migration, MigrationHistory, MigrationVersion, Schema,
    Table,
};
pub use types::{ColumnChange, TableChange};
