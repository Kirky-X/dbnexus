// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 迁移元数据跟踪模块
//!
//! 负责跟踪迁移的元数据，包括：
//! - 表结构快照
//! - 列定义
//! - 约束定义
//!
//! # Example
//!
//! ```rust,no_run
//! use dbnexus::domain::{MigrationMetadata, TableSnapshot, ColumnDefinition};
//!
//! let mut metadata = MigrationMetadata::new(
//!     1,
//!     "test_migration".to_string(),
//!     "CREATE TABLE test (id INT);".to_string(),
//! );
//! let snapshot = TableSnapshot::new("test");
//! metadata.add_table_snapshot(snapshot);
//! assert_eq!(metadata.table_snapshots.len(), 1);
//! assert!(metadata.table_snapshots.get("test").is_some());
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 列定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDefinition {
    /// 列名
    pub name: String,
    /// 数据类型
    pub data_type: String,
    /// 是否可为空
    pub nullable: bool,
    /// 默认值
    pub default_value: Option<String>,
}

/// 约束定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintDefinition {
    /// 约束名称
    pub name: Option<String>,
    /// 约束类型
    pub constraint_type: String,
    /// 约束涉及的列
    pub columns: Vec<String>,
}

/// 表结构快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSnapshot {
    /// 表名
    pub table_name: String,
    /// 列定义列表
    pub columns: Vec<ColumnDefinition>,
    /// 约束定义列表
    pub constraints: Vec<ConstraintDefinition>,
}

impl TableSnapshot {
    /// 创建新的表快照
    pub fn new(table_name: &str) -> Self {
        Self {
            table_name: table_name.to_string(),
            columns: Vec::new(),
            constraints: Vec::new(),
        }
    }
}

/// 迁移元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationMetadata {
    /// 迁移版本号
    pub version: u32,
    /// 迁移名称
    pub name: String,
    /// 迁移的 UP SQL
    pub up_sql: String,
    /// 可选的回滚 SQL
    pub down_sql: Option<String>,
    /// 表结构快照映射
    pub table_snapshots: HashMap<String, TableSnapshot>,
}

impl MigrationMetadata {
    /// 创建新的迁移元数据
    ///
    /// # Arguments
    ///
    /// * `version` - 迁移版本号
    /// * `name` - 迁移名称
    /// * `up_sql` - 迁移的 UP SQL
    /// * `down_sql` - 可选的回滚 SQL
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::domain::MigrationMetadata;
    ///
    /// let metadata = MigrationMetadata::new(
    ///     1,
    ///     "test_migration".to_string(),
    ///     "CREATE TABLE users (id INT);".to_string()
    /// );
    /// ```
    pub fn new(version: u32, name: String, up_sql: String) -> Self {
        Self {
            version,
            name,
            up_sql,
            down_sql: None,
            table_snapshots: HashMap::new(),
        }
    }

    /// 添加表结构快照
    ///
    /// # Arguments
    ///
    /// * `snapshot` - 表快照结构
    ///
    /// # Example
    ///
    /// ```rust
    /// use dbnexus::domain::migration::{MigrationMetadata, TableSnapshot};
    ///
    /// let mut metadata = MigrationMetadata::new(
    ///     1,
    ///     "test".to_string(),
    ///     "CREATE TABLE test (id INT);".to_string()
    /// );
    /// let snapshot = TableSnapshot::new("test_table");
    /// metadata.add_table_snapshot(snapshot);
    /// ```
    pub fn add_table_snapshot(&mut self, snapshot: TableSnapshot) {
        self.table_snapshots.insert(snapshot.table_name.clone(), snapshot);
    }

    /// 获取表快照
    ///
    /// # Arguments
    ///
    /// * `table_name` - 表名
    ///
    /// # Returns
    ///
    /// - `Some(TableSnapshot)` - 表快照
    /// - `None` - 表不存在
    pub fn get_table_snapshot(&self, table_name: &str) -> Option<&TableSnapshot> {
        self.table_snapshots.get(table_name)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{MigrationMetadata, TableSnapshot};

    #[test]
    fn test_create_metadata() {
        let metadata = MigrationMetadata::new(
            1,
            "test_migration".to_string(),
            "CREATE TABLE test (id INT);".to_string(),
        );
        assert_eq!(metadata.version, 1);
        assert_eq!(metadata.name, "test_migration");
        assert!(metadata.table_snapshots.is_empty());
    }

    #[test]
    fn test_add_table_snapshot() {
        let mut metadata = MigrationMetadata::new(1, "test".to_string(), "CREATE TABLE test (id INT);".to_string());
        let snapshot = TableSnapshot::new("test");
        metadata.add_table_snapshot(snapshot);
        assert_eq!(metadata.table_snapshots.len(), 1);
        assert!(metadata.table_snapshots.contains_key("test"));
    }

    #[test]
    fn test_get_nonexistent_table() {
        let metadata = MigrationMetadata::new(1, "test".to_string(), "CREATE TABLE test (id INT);".to_string());
        assert!(metadata.get_table_snapshot("nonexistent").is_none());
    }
}
