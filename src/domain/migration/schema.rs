// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
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
                // SQLite 的 INTEGER 类型本身就是 64 位（1/2/3/4/6/8 字节自动选择），
                // BIGINT 在 SQLite 中只是 INTEGER 的类型别名（亲和性为 INTEGER）。
                // 但 SQLite 的 AUTOINCREMENT 只允许声明为 INTEGER 的主键，
                // 因此 SQLite 下 BigInteger 映射为 INTEGER 是语义正确的。
                DatabaseType::Sqlite => "INTEGER".to_string(),
                _ => "BIGINT".to_string(),
            },
            ColumnType::String(None) => match db_type {
                DatabaseType::MySql => "VARCHAR(255)".to_string(),
                DatabaseType::Postgres => "VARCHAR(255)".to_string(),
                DatabaseType::Sqlite => "TEXT".to_string(),
                DatabaseType::DuckDb => "VARCHAR(255)".to_string(),
            },
            ColumnType::String(Some(len)) => match db_type {
                DatabaseType::MySql => format!("VARCHAR({})", len),
                DatabaseType::Postgres => format!("VARCHAR({})", len),
                DatabaseType::Sqlite => "TEXT".to_string(),
                DatabaseType::DuckDb => format!("VARCHAR({})", len),
            },
            ColumnType::Text => "TEXT".to_string(),
            ColumnType::Boolean => match db_type {
                DatabaseType::MySql => "BOOLEAN".to_string(),
                DatabaseType::Postgres => "BOOLEAN".to_string(),
                DatabaseType::Sqlite => "INTEGER".to_string(),
                DatabaseType::DuckDb => "BOOLEAN".to_string(),
            },
            ColumnType::Float => "FLOAT".to_string(),
            ColumnType::Double => "DOUBLE PRECISION".to_string(),
            ColumnType::Date => "DATE".to_string(),
            ColumnType::Time => "TIME".to_string(),
            ColumnType::DateTime => match db_type {
                DatabaseType::MySql => "DATETIME".to_string(),
                DatabaseType::Postgres => "TIMESTAMP".to_string(),
                DatabaseType::Sqlite => "TEXT".to_string(),
                DatabaseType::DuckDb => "TIMESTAMP".to_string(),
            },
            ColumnType::Timestamp => "TIMESTAMP".to_string(),
            ColumnType::Json => match db_type {
                DatabaseType::MySql => "JSON".to_string(),
                DatabaseType::Postgres => "JSONB".to_string(),
                DatabaseType::Sqlite => "TEXT".to_string(),
                DatabaseType::DuckDb => "JSON".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::config::DatabaseType;

    #[test]
    fn column_type_to_sql_integer() {
        assert_eq!(ColumnType::Integer.to_sql(DatabaseType::Sqlite), "INTEGER");
        assert_eq!(ColumnType::Integer.to_sql(DatabaseType::Postgres), "INTEGER");
        assert_eq!(ColumnType::Integer.to_sql(DatabaseType::MySql), "INTEGER");
    }

    #[test]
    fn column_type_to_sql_string() {
        assert_eq!(ColumnType::String(None).to_sql(DatabaseType::Sqlite), "TEXT");
        assert_eq!(ColumnType::String(None).to_sql(DatabaseType::Postgres), "VARCHAR(255)");
        assert_eq!(ColumnType::String(Some(64)).to_sql(DatabaseType::MySql), "VARCHAR(64)");
    }

    #[test]
    fn column_type_to_sql_boolean() {
        assert_eq!(ColumnType::Boolean.to_sql(DatabaseType::Sqlite), "INTEGER");
        assert_eq!(ColumnType::Boolean.to_sql(DatabaseType::Postgres), "BOOLEAN");
    }

    #[test]
    fn column_type_to_sql_json() {
        assert_eq!(ColumnType::Json.to_sql(DatabaseType::Sqlite), "TEXT");
        assert_eq!(ColumnType::Json.to_sql(DatabaseType::Postgres), "JSONB");
        assert_eq!(ColumnType::Json.to_sql(DatabaseType::MySql), "JSON");
    }

    #[test]
    fn column_type_to_sql_custom() {
        assert_eq!(ColumnType::Custom("UUID".into()).to_sql(DatabaseType::Sqlite), "UUID");
    }

    #[test]
    fn schema_new_empty() {
        let s = Schema::new(DatabaseType::Sqlite);
        assert_eq!(s.database_type, DatabaseType::Sqlite);
        assert!(s.tables.is_empty());
    }

    #[test]
    fn schema_add_and_get_table() {
        let mut s = Schema::new(DatabaseType::Sqlite);
        let table = Table {
            name: "users".into(),
            columns: vec![Column {
                name: "id".into(),
                column_type: ColumnType::Integer,
                is_primary_key: true,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: true,
                comment: None,
            }],
            primary_key_columns: vec!["id".into()],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };
        s.add_table(table);
        assert!(s.has_table("users"));
        assert!(s.get_table("users").is_some());
        assert!(s.get_table("nonexistent").is_none());
    }

    #[test]
    fn migration_new_and_add_change() {
        let mut m = Migration::new(1, "initial".into());
        assert_eq!(m.version, 1);
        assert_eq!(m.description, "initial");
        m.add_table_change(TableChange::CreateTable(Table {
            name: "t".into(),
            columns: vec![],
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        }));
        assert_eq!(m.table_changes.len(), 1);
    }

    #[test]
    fn migration_history_ordering() {
        let mut h = MigrationHistory::new();
        assert!(h.applied_migrations.is_empty());

        let v1 = MigrationVersion {
            version: 2,
            description: "v2".into(),
            applied_at: time::OffsetDateTime::now_utc(),
            file_path: "m2.sql".into(),
        };
        let v2 = MigrationVersion {
            version: 1,
            description: "v1".into(),
            applied_at: time::OffsetDateTime::now_utc(),
            file_path: "m1.sql".into(),
        };
        h.add_migration(v1);
        h.add_migration(v2);
        assert_eq!(h.applied_migrations.len(), 2);
        assert_eq!(h.get_latest_version(), Some(2));
    }

    #[test]
    fn migration_history_pending() {
        let mut h = MigrationHistory::new();
        h.add_migration(MigrationVersion {
            version: 1,
            description: "v1".into(),
            applied_at: time::OffsetDateTime::now_utc(),
            file_path: "m1.sql".into(),
        });
        let all = [
            Migration::new(1, "v1".into()),
            Migration::new(2, "v2".into()),
            Migration::new(3, "v3".into()),
        ];
        let pending = h.get_pending_migrations(&all);
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].version, 2);
        assert_eq!(pending[1].version, 3);
    }

    #[test]
    fn foreign_key_action_display() {
        assert_eq!(ForeignKeyAction::Cascade.to_string(), "CASCADE");
        assert_eq!(ForeignKeyAction::SetNull.to_string(), "SET NULL");
        assert_eq!(ForeignKeyAction::SetDefault.to_string(), "SET DEFAULT");
        assert_eq!(ForeignKeyAction::Restrict.to_string(), "RESTRICT");
        assert_eq!(ForeignKeyAction::NoAction.to_string(), "NO ACTION");
    }

    // ===== 补充测试：覆盖未覆盖的分支 =====

    #[test]
    fn test_column_type_to_sql_big_integer() {
        // SQLite 的 BigInteger 映射为 INTEGER（SQLite 的 INTEGER 是 64 位，
        // 且 AUTOINCREMENT 只允许 INTEGER）
        assert_eq!(ColumnType::BigInteger.to_sql(DatabaseType::Sqlite), "INTEGER");
        assert_eq!(ColumnType::BigInteger.to_sql(DatabaseType::Postgres), "BIGINT");
        assert_eq!(ColumnType::BigInteger.to_sql(DatabaseType::MySql), "BIGINT");
    }

    #[test]
    fn test_column_type_to_sql_float_double() {
        assert_eq!(ColumnType::Float.to_sql(DatabaseType::Postgres), "FLOAT");
        assert_eq!(ColumnType::Double.to_sql(DatabaseType::Postgres), "DOUBLE PRECISION");
        assert_eq!(ColumnType::Double.to_sql(DatabaseType::MySql), "DOUBLE PRECISION");
    }

    #[test]
    fn test_column_type_to_sql_date_time_types() {
        assert_eq!(ColumnType::Date.to_sql(DatabaseType::Postgres), "DATE");
        assert_eq!(ColumnType::Time.to_sql(DatabaseType::Postgres), "TIME");
        assert_eq!(ColumnType::Timestamp.to_sql(DatabaseType::Postgres), "TIMESTAMP");

        // DateTime 有数据库特定行为
        assert_eq!(ColumnType::DateTime.to_sql(DatabaseType::MySql), "DATETIME");
        assert_eq!(ColumnType::DateTime.to_sql(DatabaseType::Postgres), "TIMESTAMP");
        assert_eq!(ColumnType::DateTime.to_sql(DatabaseType::Sqlite), "TEXT");
    }

    #[test]
    fn test_column_type_to_sql_binary() {
        assert_eq!(ColumnType::Binary.to_sql(DatabaseType::Postgres), "BLOB");
        assert_eq!(ColumnType::Binary.to_sql(DatabaseType::Sqlite), "BLOB");
    }

    #[test]
    fn test_column_type_to_sql_text() {
        assert_eq!(ColumnType::Text.to_sql(DatabaseType::Postgres), "TEXT");
        assert_eq!(ColumnType::Text.to_sql(DatabaseType::MySql), "TEXT");
        assert_eq!(ColumnType::Text.to_sql(DatabaseType::Sqlite), "TEXT");
    }

    #[test]
    fn test_schema_default() {
        let s = Schema::default();
        assert_eq!(s.database_type, DatabaseType::Sqlite);
        assert!(s.tables.is_empty());
    }

    #[test]
    fn test_schema_get_table_mut() {
        let mut s = Schema::new(DatabaseType::Sqlite);
        let table = Table {
            name: "users".into(),
            columns: vec![Column {
                name: "id".into(),
                column_type: ColumnType::Integer,
                is_primary_key: true,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: true,
                comment: None,
            }],
            primary_key_columns: vec!["id".into()],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };
        s.add_table(table);

        // 通过 get_table_mut 修改表
        {
            let users = s.get_table_mut("users").expect("table should exist");
            users.comment = Some("updated comment".to_string());
        }

        // 验证修改生效
        let users = s.get_table("users").expect("table should exist");
        assert_eq!(users.comment.as_deref(), Some("updated comment"));

        // 不存在的表应返回 None
        assert!(s.get_table_mut("nonexistent").is_none());
    }

    #[test]
    fn test_migration_history_default() {
        let h = MigrationHistory::default();
        assert!(h.applied_migrations.is_empty());
    }

    #[test]
    fn test_migration_history_is_version_applied() {
        let mut h = MigrationHistory::new();
        assert!(!h.is_version_applied(1));

        h.add_migration(MigrationVersion {
            version: 1,
            description: "v1".into(),
            applied_at: time::OffsetDateTime::now_utc(),
            file_path: "m1.sql".into(),
        });

        assert!(h.is_version_applied(1));
        assert!(!h.is_version_applied(2));
    }

    #[test]
    fn test_migration_history_get_latest_version_empty() {
        let h = MigrationHistory::new();
        assert_eq!(h.get_latest_version(), None);
    }

    #[test]
    fn test_migration_history_get_pending_migrations_empty() {
        let h = MigrationHistory::new();
        let all: Vec<Migration> = vec![];
        let pending = h.get_pending_migrations(&all);
        assert!(pending.is_empty());
    }

    #[test]
    fn test_migration_history_get_pending_migrations_all_applied() {
        let mut h = MigrationHistory::new();
        h.add_migration(MigrationVersion {
            version: 1,
            description: "v1".into(),
            applied_at: time::OffsetDateTime::now_utc(),
            file_path: "m1.sql".into(),
        });
        h.add_migration(MigrationVersion {
            version: 2,
            description: "v2".into(),
            applied_at: time::OffsetDateTime::now_utc(),
            file_path: "m2.sql".into(),
        });

        let all = [Migration::new(1, "v1".into()), Migration::new(2, "v2".into())];
        let pending = h.get_pending_migrations(&all);
        assert!(pending.is_empty());
    }

    #[test]
    fn test_serializable_migration_version_round_trip() {
        let original = MigrationVersion {
            version: 42,
            description: "test migration".to_string(),
            applied_at: time::OffsetDateTime::now_utc(),
            file_path: "migrations/042_test.sql".to_string(),
        };

        // MigrationVersion -> SerializableMigrationVersion
        let serializable: SerializableMigrationVersion = original.clone().into();
        assert_eq!(serializable.version, 42);
        assert_eq!(serializable.description, "test migration");
        assert_eq!(serializable.file_path, "migrations/042_test.sql");
        assert!(!serializable.applied_at.is_empty());

        // SerializableMigrationVersion -> MigrationVersion (valid timestamp)
        let restored: MigrationVersion = serializable.into();
        assert_eq!(restored.version, 42);
        assert_eq!(restored.description, "test migration");
        assert_eq!(restored.file_path, "migrations/042_test.sql");
    }

    #[test]
    fn test_serializable_migration_version_invalid_timestamp_fallback() {
        let serializable = SerializableMigrationVersion {
            version: 1,
            description: "bad timestamp".to_string(),
            applied_at: "not-a-valid-timestamp".to_string(),
            file_path: "m1.sql".to_string(),
        };

        // 无效时间戳应该回退到当前时间
        let restored: MigrationVersion = serializable.into();
        assert_eq!(restored.version, 1);
        assert_eq!(restored.description, "bad timestamp");
        // applied_at 应该是当前时间（不为空，且能解析为有效时间）
        let now = time::OffsetDateTime::now_utc();
        let diff = restored.applied_at - now;
        assert!(diff.whole_seconds().abs() < 5, "timestamp should be close to now");
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
                Err(_) => {
                    // 解析失败，使用当前时间
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
