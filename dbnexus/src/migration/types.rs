// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 迁移类型定义
//!
//! 定义迁移相关的数据类型

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
}

use super::schema::{Column, Table, Index, ForeignKey};