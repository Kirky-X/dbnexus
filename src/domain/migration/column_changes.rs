// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 列变更操作
//!
//! 提供列变更操作的类型定义，用于跟踪表结构变更。
//!
//! # Example
//!
//! ```rust,no_run
//! use dbnexus::database::migration::column_changes::{ColumnChange, ColumnChangeType};
//!
//! let change = ColumnChange::new(
//!     ColumnChangeType::RenameColumn,
//!     "users".to_string(),
//!     "old_name".to_string(),
//!     "new_name".to_string()
//! );
//! assert_eq!(change.table_name, "users");
//! assert_eq!(change.column_name, "old_name");
//! assert_eq!(change.new_column_name, Some("new_name".to_string()));
//! ```

use serde::{Deserialize, Serialize};

/// 列变更类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnChangeType {
    /// 重命名列
    RenameColumn,
    /// 修改列类型
    ModifyColumn,
    /// 可空性变更
    NullabilityChanged,
    /// 默认值变更
    DefaultChanged,
}

/// 列变更
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnChange {
    /// 变更类型
    pub change_type: ColumnChangeType,
    /// 表名
    pub table_name: String,
    /// 列名
    pub column_name: String,
    /// 新列名（用于重命名）
    pub new_column_name: Option<String>,
    /// 新类型（用于修改列类型）
    pub new_type: Option<String>,
    /// 可空性
    pub nullable: Option<bool>,
    /// 默认值
    pub default_value: Option<String>,
}

impl ColumnChange {
    /// 创建新的列变更
    ///
    /// # Arguments
    ///
    /// * `change_type` - 变更类型
    /// * `table_name` - 表名
    /// * `column_name` - 列名
    /// * `value` - 新值（类型名、新列名等）
    pub fn new(change_type: ColumnChangeType, table_name: String, column_name: String, value: String) -> Self {
        let new_column_name = match change_type {
            ColumnChangeType::RenameColumn => Some(value.clone()),
            _ => None,
        };

        let new_type = match change_type {
            ColumnChangeType::ModifyColumn => Some(value.clone()),
            _ => None,
        };

        Self {
            change_type,
            table_name,
            column_name,
            new_column_name,
            new_type,
            nullable: None,
            default_value: None,
        }
    }

    /// 设置可空性
    ///
    /// # Arguments
    ///
    /// * `nullable` - 是否可为空
    pub fn with_nullable(mut self, nullable: bool) -> Self {
        self.nullable = Some(nullable);
        self
    }

    /// 设置默认值
    ///
    /// # Arguments
    ///
    /// * `default` - 默认值
    pub fn with_default(mut self, default: String) -> Self {
        self.default_value = Some(default);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{ColumnChange, ColumnChangeType};

    #[test]
    fn test_rename_column() {
        let change = ColumnChange::new(
            ColumnChangeType::RenameColumn,
            "users".to_string(),
            "old_name".to_string(),
            "new_name".to_string(),
        );
        assert_eq!(change.table_name, "users");
        assert_eq!(change.column_name, "old_name");
        assert_eq!(change.new_column_name, Some("new_name".to_string()));
    }

    #[test]
    fn test_modify_column() {
        let change = ColumnChange::new(
            ColumnChangeType::ModifyColumn,
            "users".to_string(),
            "age".to_string(),
            "INT".to_string(),
        );
        assert_eq!(change.change_type, ColumnChangeType::ModifyColumn);
        assert_eq!(change.table_name, "users");
        assert_eq!(change.column_name, "age");
        assert_eq!(change.new_type, Some("INT".to_string()));
    }

    #[test]
    fn test_with_nullable() {
        let change = ColumnChange::new(
            ColumnChangeType::ModifyColumn,
            "users".to_string(),
            "email".to_string(),
            "VARCHAR".to_string(),
        )
        .with_nullable(true);
        assert_eq!(change.nullable, Some(true));
    }

    #[test]
    fn test_with_default() {
        let change = ColumnChange::new(
            ColumnChangeType::ModifyColumn,
            "users".to_string(),
            "email".to_string(),
            "DEFAULT".to_string(),
        )
        .with_default("test@default.com".to_string());
        assert_eq!(change.default_value, Some("test@default.com".to_string()));
    }
}
