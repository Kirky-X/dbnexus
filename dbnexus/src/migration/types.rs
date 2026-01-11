// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 迁移类型定义
//!
//! 定义迁移相关的数据类型

/// 表变更类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableChange {
    /// 新增表
    CreateTable(Table),
    /// 删除表
    ///
    /// # Fields
    ///
    /// * `table_name` - 被删除的表名
    DropTable {
        /// 被删除的表名
        table_name: String,
    },
    /// 修改表
    ///
    /// 被修改的表名
    AlterTable {
        /// 表名
        table_name: String,
        /// 列变更列表
        column_changes: Vec<ColumnChange>,
        /// 新增的列
        added_columns: Vec<Column>,
        /// 删除的列名列表
        removed_columns: Vec<String>,
        /// 新增的索引
        added_indexes: Vec<Index>,
        /// 删除的索引名列表
        removed_indexes: Vec<String>,
        /// 新增的外键
        added_foreign_keys: Vec<ForeignKey>,
        /// 删除的外键名列表
        removed_foreign_keys: Vec<String>,
    },
}

/// 列变更类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnChange {
    /// 新增列
    AddColumn(Column),
    /// 删除列
    RemoveColumn {
        /// 列名
        column_name: String,
    },
    /// 修改列
    ModifyColumn {
        /// 列名
        column_name: String,
        /// 新列定义
        new_column: Column,
    },
    /// 重命名列
    RenameColumn {
        /// 旧列名
        old_name: String,
        /// 新列名
        new_name: String,
    },
    /// 类型变更
    TypeChanged {
        /// 列名
        column_name: String,
        /// 旧类型
        old_type: crate::migration::schema::ColumnType,
        /// 新类型
        new_type: crate::migration::schema::ColumnType,
    },
    /// 可空性变更
    NullabilityChanged {
        /// 列名
        column_name: String,
        /// 旧的可空性
        old_nullable: bool,
        /// 新的可空性
        new_nullable: bool,
    },
    /// 默认值变更
    DefaultChanged {
        /// 列名
        column_name: String,
        /// 旧默认值
        old_default: Option<String>,
        /// 新默认值
        new_default: Option<String>,
    },
}

use super::schema::{Column, ForeignKey, Index, Table};
