// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 迁移类型定义
//!
//! 定义迁移相关的数据类型

use super::schema::{Column, ForeignKey, Index, Table};

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
        old_type: super::schema::ColumnType,
        /// 新类型
        new_type: super::schema::ColumnType,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::migration::schema::{Column, ColumnType, Table};

    fn sample_column() -> Column {
        Column {
            name: "id".into(), column_type: ColumnType::Integer,
            is_primary_key: true, is_nullable: false,
            has_default: false, default_value: None,
            is_auto_increment: true, comment: None,
        }
    }

    fn sample_table() -> Table {
        Table {
            name: "users".into(),
            columns: vec![sample_column()],
            primary_key_columns: vec!["id".into()],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        }
    }

    #[test]
    fn table_change_create_table() {
        let c = TableChange::CreateTable(sample_table());
        assert!(matches!(c, TableChange::CreateTable(_)));
    }

    #[test]
    fn table_change_drop_table() {
        let c = TableChange::DropTable { table_name: "old".into() };
        assert_eq!(
            match c { TableChange::DropTable { ref table_name } => table_name, _ => "" },
            "old"
        );
    }

    #[test]
    fn table_change_alter_table() {
        let c = TableChange::AlterTable {
            table_name: "users".into(),
            column_changes: vec![ColumnChange::AddColumn(sample_column())],
            added_columns: vec![],
            removed_columns: vec![],
            added_indexes: vec![],
            removed_indexes: vec![],
            added_foreign_keys: vec![],
            removed_foreign_keys: vec![],
        };
        match c {
            TableChange::AlterTable { ref table_name, ref column_changes, .. } => {
                assert_eq!(table_name, "users");
                assert_eq!(column_changes.len(), 1);
            }
            _ => panic!("expected AlterTable"),
        }
    }

    #[test]
    fn column_change_add_column() {
        let c = ColumnChange::AddColumn(sample_column());
        assert!(matches!(c, ColumnChange::AddColumn(_)));
    }

    #[test]
    fn column_change_remove_column() {
        let c = ColumnChange::RemoveColumn { column_name: "age".into() };
        assert_eq!(
            match c { ColumnChange::RemoveColumn { ref column_name } => column_name, _ => "" },
            "age"
        );
    }

    #[test]
    fn column_change_modify_column() {
        let c = ColumnChange::ModifyColumn {
            column_name: "name".into(),
            new_column: sample_column(),
        };
        match c {
            ColumnChange::ModifyColumn { ref column_name, .. } => assert_eq!(column_name, "name"),
            _ => panic!("expected ModifyColumn"),
        }
    }

    #[test]
    fn column_change_rename_column() {
        let c = ColumnChange::RenameColumn {
            old_name: "old".into(), new_name: "new".into(),
        };
        match c {
            ColumnChange::RenameColumn { ref old_name, ref new_name } => {
                assert_eq!(old_name, "old");
                assert_eq!(new_name, "new");
            }
            _ => panic!("expected RenameColumn"),
        }
    }

    #[test]
    fn column_change_type_changed() {
        let c = ColumnChange::TypeChanged {
            column_name: "col".into(),
            old_type: ColumnType::Integer,
            new_type: ColumnType::Text,
        };
        match c {
            ColumnChange::TypeChanged { ref column_name, ref old_type, ref new_type } => {
                assert_eq!(column_name, "col");
                assert_eq!(*old_type, ColumnType::Integer);
                assert_eq!(*new_type, ColumnType::Text);
            }
            _ => panic!("expected TypeChanged"),
        }
    }

    #[test]
    fn column_change_nullability_changed() {
        let c = ColumnChange::NullabilityChanged {
            column_name: "col".into(), old_nullable: true, new_nullable: false,
        };
        match c {
            ColumnChange::NullabilityChanged { ref column_name, old_nullable, new_nullable } => {
                assert_eq!(column_name, "col");
                assert!(old_nullable);
                assert!(!new_nullable);
            }
            _ => panic!("expected NullabilityChanged"),
        }
    }

    #[test]
    fn column_change_default_changed() {
        let c = ColumnChange::DefaultChanged {
            column_name: "col".into(),
            old_default: Some("0".into()),
            new_default: None,
        };
        match c {
            ColumnChange::DefaultChanged { ref column_name, ref old_default, ref new_default } => {
                assert_eq!(column_name, "col");
                assert_eq!(old_default.as_deref(), Some("0"));
                assert!(new_default.is_none());
            }
            _ => panic!("expected DefaultChanged"),
        }
    }
}
