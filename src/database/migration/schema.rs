// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 数据库模式定义
//!
//! 定义数据库表、列、索引等结构

use super::types::*;
use crate::foundation::config::DatabaseType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// 列类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnType {
    /// 整数类型
    Integer,
    /// 长整数
    BigInteger,
    /// 字符串，可选长度
    String(Option<u32>),
    /// 文本
    Text,
    /// 布尔值
    Boolean,
    /// 浮点数
    Float,
    /// 双精度浮点
    Double,
    /// 日期
    Date,
    /// 时间
    Time,
    /// 日期时间
    DateTime,
    /// 时间戳
    Timestamp,
    /// JSON
    Json,
    /// 二进制
    Binary,
    /// 自定义类型
    Custom(String),
}

impl ColumnType {
    /// 获取对应数据库的类型名称
    pub fn to_sql(&self, db_type: DatabaseType) -> String {
        match self {
            ColumnType::Integer => "INTEGER".to_string(),
            ColumnType::BigInteger => match db_type {
                DatabaseType::MySql => "BIGINT".to_string(),
                _ => "BIGINT".to_string(),
            },
            ColumnType::String(None) => match db_type {
                DatabaseType::MySql => "VARCHAR(255)".to_string(),
                DatabaseType::Postgres => "VARCHAR(255)".to_string(),
                DatabaseType::Sqlite => "TEXT".to_string(),
            },
            ColumnType::String(Some(len)) => match db_type {
                DatabaseType::MySql => format!("VARCHAR({})", len),
                DatabaseType::Postgres => format!("VARCHAR({})", len),
                DatabaseType::Sqlite => "TEXT".to_string(),
            },
            ColumnType::Text => "TEXT".to_string(),
            ColumnType::Boolean => match db_type {
                DatabaseType::MySql => "BOOLEAN".to_string(),
                DatabaseType::Postgres => "BOOLEAN".to_string(),
                DatabaseType::Sqlite => "INTEGER".to_string(),
            },
            ColumnType::Float => "FLOAT".to_string(),
            ColumnType::Double => "DOUBLE PRECISION".to_string(),
            ColumnType::Date => "DATE".to_string(),
            ColumnType::Time => "TIME".to_string(),
            ColumnType::DateTime => match db_type {
                DatabaseType::MySql => "DATETIME".to_string(),
                DatabaseType::Postgres => "TIMESTAMP".to_string(),
                DatabaseType::Sqlite => "TEXT".to_string(),
            },
            ColumnType::Timestamp => "TIMESTAMP".to_string(),
            ColumnType::Json => match db_type {
                DatabaseType::MySql => "JSON".to_string(),
                DatabaseType::Postgres => "JSONB".to_string(),
                DatabaseType::Sqlite => "TEXT".to_string(),
            },
            ColumnType::Binary => "BLOB".to_string(),
            ColumnType::Custom(name) => name.to_string(),
        }
    }
}

/// 列定义
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// 列名
    pub name: String,
    /// 列类型
    pub column_type: ColumnType,
    /// 是否为主键
    pub is_primary_key: bool,
    /// 是否可为空
    pub is_nullable: bool,
    /// 是否有默认值
    pub has_default: bool,
    /// 默认值
    pub default_value: Option<String>,
    /// 是否自增
    pub is_auto_increment: bool,
    /// 注释
    pub comment: Option<String>,
}

/// 表定义
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// 表名
    pub name: String,
    /// 列定义
    pub columns: Vec<Column>,
    /// 主键列名列表
    pub primary_key_columns: Vec<String>,
    /// 索引列表
    pub indexes: Vec<Index>,
    /// 外键列表
    pub foreign_keys: Vec<ForeignKey>,
    /// 表注释
    pub comment: Option<String>,
}

/// 索引定义
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    /// 索引名
    pub name: String,
    /// 表名
    pub table_name: String,
    /// 索引列
    pub columns: Vec<String>,
    /// 是否唯一索引
    pub is_unique: bool,
    /// 是否是唯一约束
    pub is_constraint: bool,
}

/// 外键定义
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    /// 外键名
    pub name: String,
    /// 本地表名
    pub table_name: String,
    /// 本地表列
    pub column_name: String,
    /// 引用表名
    pub referenced_table_name: String,
    /// 引用表列
    pub referenced_column_name: String,
    /// 删除时的行为
    pub on_delete: Option<ForeignKeyAction>,
    /// 更新时的行为
    pub on_update: Option<ForeignKeyAction>,
}

/// 外键动作
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignKeyAction {
    /// 级联删除/更新
    Cascade,
    /// 设置为 NULL
    SetNull,
    /// 设置为默认值
    SetDefault,
    /// 限制操作
    Restrict,
    /// 不采取行动
    NoAction,
}

impl fmt::Display for ForeignKeyAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForeignKeyAction::Cascade => write!(f, "CASCADE"),
            ForeignKeyAction::SetNull => write!(f, "SET NULL"),
            ForeignKeyAction::SetDefault => write!(f, "SET DEFAULT"),
            ForeignKeyAction::Restrict => write!(f, "RESTRICT"),
            ForeignKeyAction::NoAction => write!(f, "NO ACTION"),
        }
    }
}

/// Schema 定义
#[derive(Debug, Clone)]
pub struct Schema {
    /// 数据库类型
    pub database_type: DatabaseType,
    /// 表定义（Vec 保留用于遍历）
    pub tables: Vec<Table>,
    /// 表索引（HashMap 用于 O(1) 查找）
    table_index: HashMap<String, usize>,
}

impl Default for Schema {
    fn default() -> Self {
        Self {
            database_type: DatabaseType::Sqlite,
            tables: Vec::new(),
            table_index: HashMap::new(),
        }
    }
}

impl Schema {
    /// 创建新的 Schema
    pub fn new(database_type: DatabaseType) -> Self {
        Self {
            database_type,
            tables: Vec::new(),
            table_index: HashMap::new(),
        }
    }

    /// 添加表
    pub fn add_table(&mut self, table: Table) {
        let index = self.tables.len();
        self.table_index.insert(table.name.clone(), index);
        self.tables.push(table);
    }

    /// 获取表
    pub fn get_table(&self, name: &str) -> Option<&Table> {
        if let Some(&index) = self.table_index.get(name) {
            self.tables.get(index)
        } else {
            None
        }
    }

    /// 获取表（可变）
    pub fn get_table_mut(&mut self, name: &str) -> Option<&mut Table> {
        if let Some(&index) = self.table_index.get(name) {
            self.tables.get_mut(index)
        } else {
            None
        }
    }

    /// 检查表是否存在
    pub fn has_table(&self, name: &str) -> bool {
        self.table_index.contains_key(name)
    }
}

/// 表变更类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    /// 版本号
    pub version: u32,
    /// 变更描述
    pub description: String,
    /// 表变更
    pub table_changes: Vec<TableChange>,
    /// 迁移 SQL（可选择生成）
    pub sql: Option<String>,
    /// 迁移时间戳
    pub timestamp: Option<time::OffsetDateTime>,
}

impl Migration {
    /// 创建新的 Migration
    pub fn new(version: u32, description: String) -> Self {
        Self {
            version,
            description,
            table_changes: Vec::new(),
            sql: None,
            timestamp: Some(time::OffsetDateTime::now_utc()),
        }
    }

    /// 添加表变更
    pub fn add_table_change(&mut self, change: TableChange) {
        self.table_changes.push(change);
    }
}

/// 迁移版本信息
#[derive(Debug, Clone)]
pub struct MigrationVersion {
    /// 版本号
    pub version: u32,
    /// 版本描述
    pub description: String,
    /// 应用时间
    pub applied_at: time::OffsetDateTime,
    /// 迁移文件路径
    pub file_path: String,
}

// 手动实现序列化和反序列化

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableMigrationVersion {
    /// 版本号
    pub version: u32,
    /// 版本描述
    pub description: String,
    /// 应用时间
    pub applied_at: String, // 作为字符串存储时间
    /// 迁移文件路径
    pub file_path: String,
}

impl From<MigrationVersion> for SerializableMigrationVersion {
    fn from(mv: MigrationVersion) -> Self {
        Self {
            version: mv.version,
            description: mv.description,
            applied_at: mv.applied_at.to_string(),
            file_path: mv.file_path,
        }
    }
}

impl From<SerializableMigrationVersion> for MigrationVersion {
    fn from(sm: SerializableMigrationVersion) -> Self {
        let applied_at =
            match time::OffsetDateTime::parse(&sm.applied_at, &time::format_description::well_known::Rfc3339) {
                Ok(dt) => dt,
                Err(e) => {
                    time::OffsetDateTime::now_utc()
                }
            };
        Self {
            version: sm.version,
            description: sm.description,
            applied_at,
            file_path: sm.file_path,
        }
    }
}

/// 迁移历史记录管理器
#[derive(Debug, Clone)]
pub struct MigrationHistory {
    /// 应用的迁移版本列表
    pub applied_migrations: Vec<MigrationVersion>,
}

impl Default for MigrationHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl MigrationHistory {
    /// 创建新的迁移历史记录
    pub fn new() -> Self {
        Self {
            applied_migrations: Vec::new(),
        }
    }

    /// 添加已应用的迁移
    pub fn add_migration(&mut self, migration: MigrationVersion) {
        self.applied_migrations.push(migration);
        // 按版本号排序（保持插入顺序的稳定性）
        self.applied_migrations.sort_by_key(|m| m.version);
    }

    /// 检查版本是否已应用
    pub fn is_version_applied(&self, version: u32) -> bool {
        self.applied_migrations.iter().any(|m| m.version == version)
    }

    /// 获取最高已应用版本号
    pub fn get_latest_version(&self) -> Option<u32> {
        self.applied_migrations.iter().map(|m| m.version).max()
    }

    /// 获取待应用的迁移版本
    pub fn get_pending_migrations<'a>(&self, all_migrations: &'a [Migration]) -> Vec<&'a Migration> {
        all_migrations
            .iter()
            .filter(|m| !self.is_version_applied(m.version))
            .collect()
    }
}
