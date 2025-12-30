//! Migration 模块
//!
//! 提供数据库迁移功能，包括 Schema 抽象、Schema 差异检测和 SQL 生成

use serde::{Deserialize, Serialize};
use std::fmt;

/// 数据库类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseType {
    /// PostgreSQL
    Postgres,
    /// MySQL
    MySQL,
    /// SQLite
    SQLite,
}

impl fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseType::Postgres => write!(f, "postgresql"),
            DatabaseType::MySQL => write!(f, "mysql"),
            DatabaseType::SQLite => write!(f, "sqlite"),
        }
    }
}

/// 列数据类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
                DatabaseType::MySQL => "BIGINT".to_string(),
                _ => "BIGINT".to_string(),
            },
            ColumnType::String(None) => match db_type {
                DatabaseType::MySQL => "VARCHAR(255)".to_string(),
                DatabaseType::Postgres => "VARCHAR(255)".to_string(),
                DatabaseType::SQLite => "TEXT".to_string(),
            },
            ColumnType::String(Some(len)) => match db_type {
                DatabaseType::MySQL => format!("VARCHAR({})", len),
                DatabaseType::Postgres => format!("VARCHAR({})", len),
                DatabaseType::SQLite => "TEXT".to_string(),
            },
            ColumnType::Text => "TEXT".to_string(),
            ColumnType::Boolean => match db_type {
                DatabaseType::MySQL => "BOOLEAN".to_string(),
                DatabaseType::Postgres => "BOOLEAN".to_string(),
                DatabaseType::SQLite => "INTEGER".to_string(),
            },
            ColumnType::Float => "FLOAT".to_string(),
            ColumnType::Double => "DOUBLE PRECISION".to_string(),
            ColumnType::Date => "DATE".to_string(),
            ColumnType::Time => "TIME".to_string(),
            ColumnType::DateTime => match db_type {
                DatabaseType::MySQL => "DATETIME".to_string(),
                DatabaseType::Postgres => "TIMESTAMP".to_string(),
                DatabaseType::SQLite => "TEXT".to_string(),
            },
            ColumnType::Timestamp => "TIMESTAMP".to_string(),
            ColumnType::Json => match db_type {
                DatabaseType::MySQL => "JSON".to_string(),
                DatabaseType::Postgres => "JSONB".to_string(),
                DatabaseType::SQLite => "TEXT".to_string(),
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
    /// 表定义
    pub tables: Vec<Table>,
}

impl Schema {
    /// 创建新的 Schema
    pub fn new(database_type: DatabaseType) -> Self {
        Self {
            database_type,
            tables: Vec::new(),
        }
    }

    /// 添加表
    pub fn add_table(&mut self, table: Table) {
        self.tables.push(table);
    }

    /// 获取表
    pub fn get_table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
    }

    /// 获取表（可变）
    pub fn get_table_mut(&mut self, name: &str) -> Option<&mut Table> {
        self.tables.iter_mut().find(|t| t.name == name)
    }

    /// 检查表是否存在
    pub fn has_table(&self, name: &str) -> bool {
        self.tables.iter().any(|t| t.name == name)
    }
}

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
    /// 列类型变更
    ///
    /// 变更的列名
    TypeChanged {
        /// 列名
        column_name: String,
        /// 旧的类型
        old_type: ColumnType,
        /// 新的类型
        new_type: ColumnType,
    },
    /// 列可空性变更
    ///
    /// 变更的列名和新的可空性
    NullabilityChanged {
        /// 列名
        column_name: String,
        /// 旧的可空性
        old_nullable: bool,
        /// 新的可空性
        new_nullable: bool,
    },
    /// 添加默认值
    ///
    /// 变更的列名和新的默认值
    DefaultChanged {
        /// 列名
        column_name: String,
        /// 旧的默认值
        old_default: Option<String>,
        /// 新的默认值
        new_default: Option<String>,
    },
}

/// Migration 变更
#[derive(Debug, Clone)]
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
        Self {
            version: sm.version,
            description: sm.description,
            applied_at: time::OffsetDateTime::parse(&sm.applied_at, &time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| time::OffsetDateTime::now_utc()),
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
        // 按版本号排序
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

/// 迁移执行器
#[derive(Debug, Clone)]
pub struct MigrationExecutor {
    /// 数据库连接
    pub connection: crate::orm::DatabaseConnection,
    /// SQL 生成器
    pub sql_generator: SqlGenerator,
    /// 迁移历史记录
    pub history: MigrationHistory,
}

impl MigrationExecutor {
    /// 创建新的迁移执行器
    pub fn new(connection: crate::orm::DatabaseConnection, db_type: DatabaseType) -> Self {
        Self {
            connection,
            sql_generator: SqlGenerator::new(db_type),
            history: MigrationHistory::new(),
        }
    }

    /// 读取数据库中的迁移历史
    pub async fn load_history(&mut self) -> Result<(), crate::config::DbError> {
        // 确保迁移历史表存在
        self.ensure_migration_table_exists().await?;

        // 从数据库读取历史记录
        // 注意：这里简化处理，实际需要查询数据库中的迁移记录
        Ok(())
    }

    /// 确保迁移历史表存在
    async fn ensure_migration_table_exists(&self) -> Result<(), crate::config::DbError> {
        use crate::orm::ConnectionTrait;

        // 这里需要执行创建迁移历史表的 SQL
        let create_table_sql = match self.sql_generator.db_type {
            DatabaseType::Postgres => {
                "CREATE TABLE IF NOT EXISTS dbnexus_migrations (
                    version INTEGER PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    file_path TEXT
                );"
            }
            DatabaseType::MySQL => {
                "CREATE TABLE IF NOT EXISTS dbnexus_migrations (
                    version INT PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    file_path TEXT
                );"
            }
            DatabaseType::SQLite => {
                "CREATE TABLE IF NOT EXISTS dbnexus_migrations (
                    version INTEGER PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT (datetime('now')),
                    file_path TEXT
                );"
            }
        };

        self.connection
            .execute_unprepared(create_table_sql)
            .await
            .map_err(crate::config::DbError::Connection)?;
        Ok(())
    }

    /// 应用单个迁移
    pub async fn apply_migration(&mut self, migration: &Migration) -> Result<(), crate::config::DbError> {
        use crate::orm::{ConnectionTrait, TransactionTrait};

        // 生成迁移 SQL
        let sql = self.sql_generator.generate_migration_sql(migration);

        // 开始事务
        let txn = self
            .connection
            .begin()
            .await
            .map_err(crate::config::DbError::Connection)?;

        // 执行迁移 SQL
        if !sql.is_empty() {
            txn.execute_unprepared(&sql)
                .await
                .map_err(crate::config::DbError::Connection)?;
        }

        // 记录迁移历史
        let version_record = MigrationVersion {
            version: migration.version,
            description: migration.description.clone(),
            applied_at: migration.timestamp.unwrap_or_else(time::OffsetDateTime::now_utc),
            file_path: format!("migration_v{}.sql", migration.version),
        };

        // 插入到迁移历史表
        let insert_sql = match self.sql_generator.db_type {
            DatabaseType::Postgres | DatabaseType::MySQL => {
                format!(
                    "INSERT INTO dbnexus_migrations (version, description, applied_at, file_path) VALUES ({}, '{}', '{}', '{}');",
                    migration.version,
                    migration.description.replace('\'', "''"), // 转义单引号
                    version_record.applied_at.to_string().replace('\'', "''"),
                    version_record.file_path.replace('\'', "''")
                )
            }
            DatabaseType::SQLite => {
                format!(
                    "INSERT INTO dbnexus_migrations (version, description, applied_at, file_path) VALUES ({}, '{}', '{}', '{}');",
                    migration.version,
                    migration.description.replace('\'', "''"), // 转义单引号
                    version_record.applied_at.to_string().replace('\'', "''"),
                    version_record.file_path.replace('\'', "''")
                )
            }
        };

        txn.execute_unprepared(&insert_sql)
            .await
            .map_err(crate::config::DbError::Connection)?;
        // 提交事务
        txn.commit().await.map_err(crate::config::DbError::Connection)?;

        Ok(())
    }

    /// 获取待应用的迁移
    pub async fn get_pending_migrations<'a>(&'a mut self, all_migrations: &'a [Migration]) -> Vec<&'a Migration> {
        // 重新加载历史记录以获取最新状态
        if self.load_history().await.is_ok() {
            self.history.get_pending_migrations(all_migrations)
        } else {
            // 如果加载失败，返回所有迁移（保守处理）
            all_migrations.iter().collect()
        }
    }

    /// 获取所有迁移的版本号
    pub fn get_all_versions(&self) -> Vec<u32> {
        self.history.applied_migrations.iter().map(|m| m.version).collect()
    }

    /// 获取最新应用的迁移
    pub fn get_latest_migration(&self) -> Option<&MigrationVersion> {
        self.history.applied_migrations.last()
    }

    /// 检查是否所有迁移都已应用
    pub fn is_fully_migrated(&self, total_migrations: usize) -> bool {
        self.history.applied_migrations.len() == total_migrations
    }
}

/// 迁移文件解析器
pub struct MigrationFileParser;

impl MigrationFileParser {
    /// 解析迁移文件内容
    pub fn parse_migration_file(content: &str) -> Result<(String, String), String> {
        // 提取迁移描述
        let description = Self::extract_description(content);

        // 验证SQL语法（简单验证）
        Self::validate_sql_syntax(content)?;

        Ok((description, content.to_string()))
    }

    /// 从迁移文件中提取描述
    fn extract_description(content: &str) -> String {
        // 尝试从注释中提取描述
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("-- Migration:") {
                return trimmed[12..].trim().to_string();
            } else if trimmed.starts_with("/*") || trimmed.starts_with("--") {
                continue; // 跳过其他注释行
            } else {
                break; // 遇到非注释行则停止
            }
        }
        "Migration".to_string()
    }

    /// 验证SQL语法（基本验证）
    fn validate_sql_syntax(content: &str) -> Result<(), String> {
        // 检查是否包含基本的SQL语句
        let has_up = content.contains("UP") || content.contains("up") || content.to_uppercase().contains("-- UP");
        let has_down =
            content.contains("DOWN") || content.contains("down") || content.to_uppercase().contains("-- DOWN");

        if !has_up && !has_down {
            // 如果没有UP/DOWN标记，只要包含SQL语句即可
            let sql_statements = ["CREATE", "ALTER", "DROP", "INSERT", "UPDATE", "DELETE"];
            let contains_sql = sql_statements.iter().any(|stmt| content.to_uppercase().contains(stmt));

            if !contains_sql {
                return Err("Migration file does not contain recognizable SQL statements".to_string());
            }
        }

        Ok(())
    }
}

/// 迁移计划
#[derive(Debug, Clone)]
pub struct MigrationPlan {
    /// 待执行的迁移列表
    pub migrations: Vec<Migration>,
    /// 执行方向（向上或向下）
    pub direction: MigrationDirection,
}

/// 迁移方向
#[derive(Debug, Clone)]
pub enum MigrationDirection {
    /// 向上迁移（应用新版本）
    Up,
    /// 向下迁移（回滚版本）
    Down,
}

/// 迁移工具 CLI 命令
#[derive(Debug, Clone)]
pub enum MigrationCommand {
    /// 创建新的迁移文件
    Create {
        /// 迁移描述
        description: String,
        /// 目录路径
        directory: String,
    },
    /// 应用迁移
    Up {
        /// 目标版本号，None 表示应用所有迁移
        target_version: Option<u32>,
    },
    /// 回滚迁移
    Down {
        /// 目标版本号，None 表示回滚到初始状态
        target_version: Option<u32>,
    },
    /// 查看迁移状态
    Status,
    /// 生成迁移文件
    Generate {
        /// 从模式生成迁移
        from_schema: String,
        /// 到模式
        to_schema: String,
        /// 输出文件
        output_file: String,
    },
}

/// Schema 差异计算器
pub struct SchemaDiffer {
    /// 源 Schema
    old_schema: Schema,
    /// 目标 Schema
    new_schema: Schema,
}

impl SchemaDiffer {
    /// 创建新的 SchemaDiffer
    pub fn new(old_schema: Schema, new_schema: Schema) -> Self {
        Self { old_schema, new_schema }
    }

    /// 计算差异并生成 Migration
    pub fn diff(&self) -> Vec<Migration> {
        let mut migrations = Vec::new();
        let mut migration = Migration::new(1, "Schema changes".to_string());

        // 检测新增的表
        for new_table in &self.new_schema.tables {
            if !self.old_schema.has_table(&new_table.name) {
                migration.add_table_change(TableChange::CreateTable(new_table.clone()));
            }
        }

        // 检测删除的表
        for old_table in &self.old_schema.tables {
            if !self.new_schema.has_table(&old_table.name) {
                migration.add_table_change(TableChange::DropTable {
                    table_name: old_table.name.clone(),
                });
            }
        }

        // 检测修改的表
        for new_table in &self.new_schema.tables {
            if let Some(old_table) = self.old_schema.get_table(&new_table.name) {
                // 检测列变更
                let column_changes = self.detect_column_changes(old_table, new_table);
                let added_columns = self.detect_added_columns(old_table, new_table);
                let removed_columns = self.detect_removed_columns(old_table, new_table);
                let added_indexes = self.detect_added_indexes(old_table, new_table);
                let removed_indexes = self.detect_removed_indexes(old_table, new_table);
                let added_foreign_keys = self.detect_added_foreign_keys(old_table, new_table);
                let removed_foreign_keys = self.detect_removed_foreign_keys(old_table, new_table);

                if !column_changes.is_empty()
                    || !added_columns.is_empty()
                    || !removed_columns.is_empty()
                    || !added_indexes.is_empty()
                    || !removed_indexes.is_empty()
                    || !added_foreign_keys.is_empty()
                    || !removed_foreign_keys.is_empty()
                {
                    migration.add_table_change(TableChange::AlterTable {
                        table_name: new_table.name.clone(),
                        column_changes,
                        added_columns,
                        removed_columns,
                        added_indexes,
                        removed_indexes,
                        added_foreign_keys,
                        removed_foreign_keys,
                    });
                }
            }
        }

        if !migration.table_changes.is_empty() {
            migrations.push(migration);
        }

        migrations
    }

    /// 检测列变更
    fn detect_column_changes(&self, old_table: &Table, new_table: &Table) -> Vec<ColumnChange> {
        let mut changes = Vec::new();

        for new_column in &new_table.columns {
            if let Some(old_column) = old_table.columns.iter().find(|c| c.name == new_column.name) {
                // 检测类型变更
                if old_column.column_type != new_column.column_type {
                    changes.push(ColumnChange::TypeChanged {
                        column_name: new_column.name.clone(),
                        old_type: old_column.column_type.clone(),
                        new_type: new_column.column_type.clone(),
                    });
                }

                // 检测可空性变更
                if old_column.is_nullable != new_column.is_nullable {
                    changes.push(ColumnChange::NullabilityChanged {
                        column_name: new_column.name.clone(),
                        old_nullable: old_column.is_nullable,
                        new_nullable: new_column.is_nullable,
                    });
                }

                // 检测默认值变更
                if old_column.default_value != new_column.default_value {
                    changes.push(ColumnChange::DefaultChanged {
                        column_name: new_column.name.clone(),
                        old_default: old_column.default_value.clone(),
                        new_default: new_column.default_value.clone(),
                    });
                }
            }
        }

        changes
    }

    /// 检测新增的列
    fn detect_added_columns(&self, old_table: &Table, new_table: &Table) -> Vec<Column> {
        new_table
            .columns
            .iter()
            .filter(|c| !old_table.columns.iter().any(|oc| oc.name == c.name))
            .cloned()
            .collect()
    }

    /// 检测删除的列
    fn detect_removed_columns(&self, old_table: &Table, new_table: &Table) -> Vec<String> {
        old_table
            .columns
            .iter()
            .filter(|c| !new_table.columns.iter().any(|nc| nc.name == c.name))
            .map(|c| c.name.clone())
            .collect()
    }

    /// 检测新增的索引
    fn detect_added_indexes(&self, old_table: &Table, new_table: &Table) -> Vec<Index> {
        new_table
            .indexes
            .iter()
            .filter(|i| !old_table.indexes.iter().any(|oi| oi.name == i.name))
            .cloned()
            .collect()
    }

    /// 检测删除的索引
    fn detect_removed_indexes(&self, old_table: &Table, new_table: &Table) -> Vec<String> {
        old_table
            .indexes
            .iter()
            .filter(|i| !new_table.indexes.iter().any(|ni| ni.name == i.name))
            .map(|i| i.name.clone())
            .collect()
    }

    /// 检测新增的外键
    fn detect_added_foreign_keys(&self, old_table: &Table, new_table: &Table) -> Vec<ForeignKey> {
        new_table
            .foreign_keys
            .iter()
            .filter(|fk| !old_table.foreign_keys.iter().any(|ofk| ofk.name == fk.name))
            .cloned()
            .collect()
    }

    /// 检测删除的外键
    fn detect_removed_foreign_keys(&self, old_table: &Table, new_table: &Table) -> Vec<String> {
        old_table
            .foreign_keys
            .iter()
            .filter(|fk| !new_table.foreign_keys.iter().any(|nfk| nfk.name == fk.name))
            .map(|fk| fk.name.clone())
            .collect()
    }
}

/// SQL 生成器
#[derive(Debug, Clone)]
pub struct SqlGenerator {
    /// 数据库类型
    pub db_type: DatabaseType,
}

impl SqlGenerator {
    /// 创建新的 SQLGenerator
    pub fn new(db_type: DatabaseType) -> Self {
        Self { db_type }
    }

    /// 生成列定义的 SQL（仅类型部分，用于测试）
    pub fn generate_column_def(&self, column_type: &ColumnType) -> String {
        column_type.to_sql(self.db_type)
    }

    /// 生成创建表的 SQL
    pub fn generate_create_table_sql(&self, table: &Table) -> String {
        let mut sql = format!("CREATE TABLE {} (\n", table.name);

        let column_defs: Vec<String> = table
            .columns
            .iter()
            .map(|col| self.generate_column_definition(col, &table.primary_key_columns))
            .collect();

        sql.push_str(&column_defs.join(",\n"));

        // 添加主键约束
        if !table.primary_key_columns.is_empty() {
            sql.push_str(",\n");
            sql.push_str(&format!("    PRIMARY KEY ({})", table.primary_key_columns.join(", ")));
        }

        sql.push_str("\n);");

        // 生成索引
        for index in &table.indexes {
            if !index.is_constraint {
                sql.push_str("\n\n");
                sql.push_str(&self.generate_create_index_sql(index));
            }
        }

        // 生成外键
        for fk in &table.foreign_keys {
            sql.push_str("\n\n");
            sql.push_str(&self.generate_add_foreign_key_sql(fk));
        }

        sql
    }

    /// 生成列定义
    fn generate_column_definition(&self, column: &Column, _pk_columns: &[String]) -> String {
        let mut def = format!("    {} {}", column.name, column.column_type.to_sql(self.db_type));

        // 自增列不需要指定
        if column.is_auto_increment && column.is_primary_key {
            match self.db_type {
                DatabaseType::MySQL => def.push_str(" AUTO_INCREMENT"),
                DatabaseType::SQLite => def.push_str(" PRIMARY KEY AUTOINCREMENT"),
                _ => {}
            }
        }

        if !column.is_nullable {
            def.push_str(" NOT NULL");
        }

        if let Some(default) = &column.default_value {
            def.push_str(&format!(" DEFAULT {}", default));
        }

        // 主键列如果有自增，不需要单独 PRIMARY KEY
        if column.is_primary_key && !column.is_auto_increment {
            // 主键已在表级别处理
        }

        def
    }

    /// 生成创建索引的 SQL
    pub fn generate_create_index_sql(&self, index: &Index) -> String {
        let unique = if index.is_unique { "UNIQUE " } else { "" };
        format!(
            "CREATE {}INDEX {} ON {} ({})",
            unique,
            index.name,
            index.table_name,
            index.columns.join(", ")
        )
    }

    /// 生成添加外键的 SQL
    fn generate_add_foreign_key_sql(&self, fk: &ForeignKey) -> String {
        let mut sql = format!(
            "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({})",
            fk.table_name, fk.name, fk.column_name, fk.referenced_table_name, fk.referenced_column_name
        );

        if let Some(on_delete) = &fk.on_delete {
            sql.push_str(&format!(" ON DELETE {}", on_delete));
        }

        if let Some(on_update) = &fk.on_update {
            sql.push_str(&format!(" ON UPDATE {}", on_update));
        }

        sql.push(';');
        sql
    }

    /// 生成删除表的 SQL
    pub fn generate_drop_table_sql(&self, table_name: &str) -> String {
        format!("DROP TABLE {};", table_name)
    }

    /// 生成添加列的 SQL
    pub fn generate_add_column_sql(&self, table_name: &str, column: &Column) -> String {
        let col_def = self.generate_column_definition(column, &Vec::new());
        format!("ALTER TABLE {} ADD {};", table_name, col_def.trim_start_matches("    "))
    }

    /// 生成删除列的 SQL
    pub fn generate_drop_column_sql(&self, table_name: &str, column_name: &str) -> String {
        match self.db_type {
            DatabaseType::MySQL => {
                format!("ALTER TABLE {} DROP COLUMN {};", table_name, column_name)
            }
            DatabaseType::Postgres => {
                format!("ALTER TABLE {} DROP COLUMN {};", table_name, column_name)
            }
            DatabaseType::SQLite => {
                // SQLite 不支持直接删除列，需要重建表
                format!(
                    "-- SQLite 不支持直接删除列，请手动重建表 {}
ALTER TABLE {} DROP COLUMN {};",
                    table_name, table_name, column_name
                )
            }
        }
    }

    /// 生成迁移的完整 SQL
    pub fn generate_migration_sql(&self, migration: &Migration) -> String {
        let mut sql = String::new();

        for change in &migration.table_changes {
            match change {
                TableChange::CreateTable(table) => {
                    sql.push_str(&format!("-- 创建表: {}\n", table.name));
                    sql.push_str(&self.generate_create_table_sql(table));
                    sql.push_str("\n\n");
                }
                TableChange::DropTable { table_name } => {
                    sql.push_str(&format!("-- 删除表: {}\n", table_name));
                    sql.push_str(&self.generate_drop_table_sql(table_name));
                    sql.push_str("\n\n");
                }
                TableChange::AlterTable {
                    table_name,
                    added_columns,
                    removed_columns,
                    added_indexes,
                    removed_indexes,
                    added_foreign_keys,
                    removed_foreign_keys,
                    ..
                } => {
                    sql.push_str(&format!("-- 修改表: {}\n", table_name));

                    for col in added_columns {
                        sql.push_str(&format!("-- 添加列: {}\n", col.name));
                        sql.push_str(&self.generate_add_column_sql(table_name, col));
                        sql.push('\n');
                    }

                    for col_name in removed_columns {
                        sql.push_str(&format!("-- 删除列: {}\n", col_name));
                        sql.push_str(&self.generate_drop_column_sql(table_name, col_name));
                        sql.push('\n');
                    }

                    for index in added_indexes {
                        sql.push_str(&format!("-- 添加索引: {}\n", index.name));
                        sql.push_str(&self.generate_create_index_sql(index));
                        sql.push('\n');
                    }

                    for index_name in removed_indexes {
                        sql.push_str(&format!("-- 删除索引: {}\n", index_name));
                        sql.push_str(&format!("DROP INDEX {};\n", index_name));
                    }

                    for fk in added_foreign_keys {
                        sql.push_str(&format!("-- 添加外键: {}\n", fk.name));
                        sql.push_str(&self.generate_add_foreign_key_sql(fk));
                        sql.push('\n');
                    }

                    for fk_name in removed_foreign_keys {
                        sql.push_str(&format!("-- 删除外键: {}\n", fk_name));
                        sql.push_str(&format!("ALTER TABLE {} DROP CONSTRAINT {};\n", table_name, fk_name));
                    }

                    sql.push('\n');
                }
            }
        }

        sql.trim_end().to_string()
    }
}

/// Rust 结构体解析器
///
/// 从 Rust 实体结构体定义中解析数据库表结构，
/// 支持从 `#[sea_orm(...)]` 属性中提取列信息
#[derive(Debug, Clone)]
pub struct RustEntityParser;

impl RustEntityParser {
    /// 解析 Rust 实体定义
    ///
    /// 通过解析 Rust 源代码，提取实体结构体中的字段信息，
    /// 并转换为数据库表结构。
    ///
    /// # Arguments
    ///
    /// * `entity_code` - Rust 实体结构体源代码
    /// * `table_name` - 目标数据库表名
    ///
    /// # Returns
    ///
    /// 解析后的表结构定义
    ///
    /// # Example
    ///
    /// ```ignore
    /// let code = r#"
    /// #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    /// #[sea_orm(table_name = "users")]
    /// pub struct Model {
    ///     #[sea_orm(primary_key)]
    ///     pub id: i32,
    ///     #[sea_orm(column_type = "String(255)")]
    ///     pub name: String,
    /// }
    /// "#;
    ///
    /// let table = RustEntityParser::parse_entity(code, "users").unwrap();
    /// ```
    pub fn parse_entity(entity_code: &str, table_name: &str) -> Result<Table, String> {
        // 简化实现：解析 sea-orm 属性
        // 实际实现需要完整的 Rust 解析器 (syn/quote)
        let columns = Self::extract_columns_from_code(entity_code)?;

        let primary_key_columns = columns
            .iter()
            .filter(|c| c.is_primary_key)
            .map(|c| c.name.clone())
            .collect();

        Ok(Table {
            name: table_name.to_string(),
            columns,
            primary_key_columns,
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
            comment: None,
        })
    }

    /// 从代码中提取列信息
    fn extract_columns_from_code(entity_code: &str) -> Result<Vec<Column>, String> {
        let mut columns = Vec::new();

        // 解析属性和字段
        let lines: Vec<&str> = entity_code.lines().collect();

        let mut current_field_name: Option<String> = None;
        let mut current_field_type: Option<String> = None;
        let mut current_column_type: Option<ColumnType> = None;
        let mut field_column_type: Option<ColumnType> = None; // 当前字段开始前设置的 column_type
        let mut is_primary_key = false;
        let mut is_nullable = true;
        let mut is_auto_increment = false;

        for line in &lines {
            let line = line.trim();

            // 提取字段名和类型
            if let Some((field_name, field_type)) = Self::extract_field_and_type(line) {
                // 保存之前的字段（如果存在且有类型）
                if let Some(ref prev_field_name) = current_field_name {
                    let col_type = field_column_type
                        .take()
                        .or_else(|| Self::infer_column_type(&current_field_type));

                    if let Some(type_result) = col_type {
                        if !columns.iter().any(|c: &Column| c.name == *prev_field_name) {
                            columns.push(Column {
                                name: prev_field_name.clone(),
                                column_type: type_result,
                                is_primary_key,
                                is_nullable,
                                has_default: false,
                                default_value: None,
                                is_auto_increment,
                                comment: None,
                            });
                        }
                    }

                    // 保存完后重置属性，为新字段做准备
                    is_primary_key = false;
                    is_nullable = true;
                    is_auto_increment = false;
                }

                // 将当前属性行的 column_type 移到 field_column_type
                field_column_type = current_column_type.take();

                // 设置新字段
                current_field_name = Some(field_name);
                current_field_type = Some(field_type);
                continue;
            }

            // 提取列类型
            if line.contains("column_type") {
                current_column_type = Self::extract_column_type(line);
            }

            // 检测主键
            if line.contains("primary_key") {
                is_primary_key = true;
            }

            // 检测可空性
            if line.contains("NotNull") || line.contains("not_null") {
                is_nullable = false;
            }

            // 检测自增
            if line.contains("AutoIncrement") || line.contains("auto_increment") {
                is_auto_increment = true;
            }

            // 如果遇到新属性行，跳过
            if line.starts_with("#[") {
                continue;
            }
        }

        // 处理最后一个字段
        if let Some(ref field_name) = current_field_name {
            // 使用当前字段开始前设置的 column_type
            let col_type = field_column_type
                .take()
                .or_else(|| Self::infer_column_type(&current_field_type));

            if let Some(type_result) = col_type {
                columns.push(Column {
                    name: field_name.clone(),
                    column_type: type_result,
                    is_primary_key,
                    is_nullable,
                    has_default: false,
                    default_value: None,
                    is_auto_increment,
                    comment: None,
                });
            }
        }

        if columns.is_empty() {
            return Err("未能解析到任何列".to_string());
        }

        Ok(columns)
    }

    /// 从字段行提取字段名和类型
    fn extract_field_and_type(line: &str) -> Option<(String, String)> {
        let trimmed = line.trim();

        // 跳过属性行
        if trimmed.starts_with("#[") {
            return None;
        }

        // 跳过结构体定义行: pub struct Xxx {
        if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
            return None;
        }

        // 匹配模式: pub name: String, 或 name: String,
        // 必须有冒号才是字段定义
        let colon_idx = trimmed.find(':')?;
        let before_colon = &trimmed[..colon_idx];
        let after_colon = &trimmed[colon_idx + 1..];

        // 清理字段名 - 去除 pub 前缀
        let mut field_name = before_colon.trim_end().trim_end_matches(',').trim().to_string();
        if field_name.starts_with("pub ") {
            field_name = field_name[4..].to_string();
        }

        // 跳过非字段行
        if field_name.starts_with("#[") || field_name.starts_with("fn ") || field_name.is_empty() {
            return None;
        }

        // 提取类型（直到逗号或右花括号）
        let mut type_str = after_colon.trim();
        let type_end = type_str
            .find(',')
            .unwrap_or_else(|| type_str.find('}').unwrap_or(type_str.len()));
        type_str = &type_str[..type_end];

        if field_name.is_empty() || type_str.is_empty() {
            return None;
        }

        Some((field_name.to_string(), type_str.to_string()))
    }

    /// 从属性行提取列类型
    fn extract_column_type(attr_line: &str) -> Option<ColumnType> {
        // 匹配: #[sea_orm(column_type = "String(255)")]
        // 或: #[sea_orm(column_type = "Text")]
        if let Some(start) = attr_line.find("column_type") {
            let after = &attr_line[start..];
            if let Some(eq_idx) = after.find('=') {
                let type_str = &after[eq_idx + 1..];
                // 提取引号内的内容
                if let Some(quote_start) = type_str.find('"') {
                    if let Some(quote_end) = type_str[quote_start + 1..].find('"') {
                        let type_content = &type_str[quote_start + 1..quote_start + 1 + quote_end];
                        return Some(Self::parse_column_type_str(type_content));
                    }
                }
            }
        }
        None
    }

    /// 解析列类型字符串
    fn parse_column_type_str(type_str: &str) -> ColumnType {
        match type_str {
            "Integer" | "Int" | "i32" => ColumnType::Integer,
            "BigInteger" | "BigInt" => ColumnType::BigInteger,
            "String" => ColumnType::String(Some(255)),
            s if s.starts_with("String(") => {
                if let Some(len_str) = s.strip_prefix("String(").and_then(|s| s.strip_suffix(')')) {
                    if let Ok(len) = len_str.parse() {
                        return ColumnType::String(Some(len));
                    }
                }
                ColumnType::String(Some(255))
            }
            "Text" => ColumnType::Text,
            "Boolean" | "Bool" | "bool" => ColumnType::Boolean,
            "Float" | "f32" => ColumnType::Float,
            "Double" | "f64" => ColumnType::Double,
            "Date" => ColumnType::Date,
            "Time" => ColumnType::Time,
            "DateTime" | "DateTimeUtc" => ColumnType::DateTime,
            "Timestamp" | "TimestampUtc" => ColumnType::Timestamp,
            "Json" | "JsonValue" => ColumnType::Json,
            "Binary" | "Vec<u8>" => ColumnType::Binary,
            _ => ColumnType::Custom(type_str.to_string()),
        }
    }

    /// 从 Rust 类型推断列类型
    fn infer_column_type(field_type: &Option<String>) -> Option<ColumnType> {
        let type_str = field_type.as_ref()?.to_lowercase();

        // 处理 Option<T>
        let inner_type = if type_str.starts_with("option<") {
            if let Some(end) = type_str.find('>') {
                &type_str[7..end]
            } else {
                &type_str
            }
        } else {
            &type_str
        };

        // 映射 Rust 类型到 ColumnType
        match inner_type {
            t if t.contains("i32") || t == "integer" || t == "int" => Some(ColumnType::Integer),
            t if t.contains("i64") || t == "biginteger" || t == "bigint" => Some(ColumnType::BigInteger),
            t if t.contains("string") || t.contains("&str") => {
                // 检查是否有长度指定
                if let Some(len_start) = t.find('<') {
                    if let Some(len_end) = t[len_start..].find('>') {
                        let len_str = &t[len_start + 1..len_start + len_end];
                        if let Ok(len) = len_str.parse() {
                            return Some(ColumnType::String(Some(len)));
                        }
                    }
                }
                Some(ColumnType::String(Some(255)))
            }
            t if t.contains("text") || t.contains("string") => Some(ColumnType::Text),
            t if t.contains("bool") => Some(ColumnType::Boolean),
            t if t.contains("f32") | t.contains("float") => Some(ColumnType::Float),
            t if t.contains("f64") | t.contains("double") => Some(ColumnType::Double),
            t if t.contains("date") && t.contains("time") => Some(ColumnType::DateTime),
            t if t.contains("date") => Some(ColumnType::Date),
            t if t.contains("time") => Some(ColumnType::Time),
            t if t.contains("timestamp") => Some(ColumnType::Timestamp),
            t if t.contains("json") => Some(ColumnType::Json),
            t if t.contains("vec<u8>") || t.contains("binary") => Some(ColumnType::Binary),
            _ => None,
        }
    }

    /// 生成从实体到表的迁移
    ///
    /// # Arguments
    ///
    /// * `entity_code` - Rust 实体结构体源代码
    /// * `table_name` - 目标数据库表名
    /// * `db_type` - 目标数据库类型
    ///
    /// # Returns
    ///
    /// 创建表的 SQL 语句
    pub fn generate_migration_sql(
        entity_code: &str,
        table_name: &str,
        db_type: DatabaseType,
    ) -> Result<String, String> {
        let table = Self::parse_entity(entity_code, table_name)?;
        let generator = SqlGenerator::new(db_type);
        Ok(generator.generate_create_table_sql(&table))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-020: ColumnType SQL 生成测试
    #[test]
    fn test_column_type_to_sql() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let mysql = SqlGenerator::new(DatabaseType::MySQL);
        let sqlite = SqlGenerator::new(DatabaseType::SQLite);

        // Integer
        assert_eq!(pg.generate_column_def(&ColumnType::Integer), "INTEGER");
        assert_eq!(mysql.generate_column_def(&ColumnType::Integer), "INTEGER");
        assert_eq!(sqlite.generate_column_def(&ColumnType::Integer), "INTEGER");

        // Boolean
        assert_eq!(pg.generate_column_def(&ColumnType::Boolean), "BOOLEAN");
        assert_eq!(mysql.generate_column_def(&ColumnType::Boolean), "BOOLEAN");
        assert_eq!(sqlite.generate_column_def(&ColumnType::Boolean), "INTEGER");
    }

    /// TEST-U-021: Schema 差异检测测试
    #[test]
    fn test_schema_diff_new_table() {
        let old_schema = Schema::new(DatabaseType::Postgres);
        let mut new_schema = Schema::new(DatabaseType::Postgres);

        // old: no tables
        // new: has users table
        let users_table = Table {
            name: "users".to_string(),
            columns: vec![Column {
                name: "id".to_string(),
                column_type: ColumnType::Integer,
                is_primary_key: true,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: true,
                comment: None,
            }],
            primary_key_columns: vec!["id".to_string()],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };
        new_schema.add_table(users_table);

        let differ = SchemaDiffer::new(old_schema, new_schema);
        let migrations = differ.diff();

        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].table_changes.len(), 1);

        if let TableChange::CreateTable(table) = &migrations[0].table_changes[0] {
            assert_eq!(table.name, "users");
        } else {
            panic!("Expected CreateTable change");
        }
    }

    /// TEST-U-022: Schema 差异检测 - 删除表
    #[test]
    fn test_schema_diff_drop_table() {
        let mut old_schema = Schema::new(DatabaseType::Postgres);
        let new_schema = Schema::new(DatabaseType::Postgres);

        let users_table = Table {
            name: "users".to_string(),
            columns: vec![],
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };
        old_schema.add_table(users_table);
        // new_schema is empty

        let differ = SchemaDiffer::new(old_schema, new_schema);
        let migrations = differ.diff();

        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].table_changes.len(), 1);

        if let TableChange::DropTable { table_name } = &migrations[0].table_changes[0] {
            assert_eq!(table_name, "users");
        } else {
            panic!("Expected DropTable change");
        }
    }

    /// TEST-U-023: SQL 生成测试
    #[test]
    fn test_sql_generation() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);

        let table = Table {
            name: "users".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    column_type: ColumnType::Integer,
                    is_primary_key: true,
                    is_nullable: false,
                    has_default: false,
                    default_value: None,
                    is_auto_increment: true,
                    comment: None,
                },
                Column {
                    name: "name".to_string(),
                    column_type: ColumnType::String(Some(255)),
                    is_primary_key: false,
                    is_nullable: false,
                    has_default: false,
                    default_value: None,
                    is_auto_increment: false,
                    comment: None,
                },
            ],
            primary_key_columns: vec!["id".to_string()],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };

        let sql = pg.generate_create_table_sql(&table);

        assert!(sql.contains("CREATE TABLE users"));
        assert!(sql.contains("id INTEGER"));
        assert!(sql.contains("name VARCHAR(255)"));
        assert!(sql.contains("NOT NULL"));
        assert!(sql.contains("PRIMARY KEY (id)"));
    }

    /// TEST-U-024: Rust 实体解析测试 - 基础解析
    #[test]
    fn test_rust_entity_parser_basic() {
        let entity_code = r#"
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(column_type = "String(255)")]
    pub name: String,
    #[sea_orm(column_type = "Text")]
    pub bio: Option<String>,
}
"#;

        let table = RustEntityParser::parse_entity(entity_code, "users").unwrap();

        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 3);
        assert_eq!(table.primary_key_columns, vec!["id"]);

        // 检查 id 列
        let id_col = table.columns.iter().find(|c| c.name == "id").unwrap();
        assert_eq!(id_col.column_type, ColumnType::Integer);
        assert!(id_col.is_primary_key);

        // 检查 name 列
        let name_col = table.columns.iter().find(|c| c.name == "name").unwrap();
        assert_eq!(name_col.column_type, ColumnType::String(Some(255)));
    }

    /// TEST-U-025: Rust 实体解析测试 - 生成迁移 SQL
    #[test]
    fn test_rust_entity_generate_migration() {
        let entity_code = r#"
#[sea_orm(table_name = "posts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "String(255)")]
    pub title: String,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    #[sea_orm(column_type = "DateTime")]
    pub created_at: DateTimeUtc,
}
"#;

        let sql = RustEntityParser::generate_migration_sql(entity_code, "posts", DatabaseType::Postgres).unwrap();

        assert!(sql.contains("CREATE TABLE posts"));
        assert!(sql.contains("id BIGINT"));
        assert!(sql.contains("title VARCHAR(255)"));
        assert!(sql.contains("content TEXT"));
        assert!(sql.contains("created_at TIMESTAMP"));
    }

    /// TEST-U-026: Rust 实体解析测试 - 列类型解析
    #[test]
    fn test_parse_column_type_string() {
        assert_eq!(RustEntityParser::parse_column_type_str("Integer"), ColumnType::Integer);
        assert_eq!(
            RustEntityParser::parse_column_type_str("String(100)"),
            ColumnType::String(Some(100))
        );
        assert_eq!(RustEntityParser::parse_column_type_str("Text"), ColumnType::Text);
        assert_eq!(RustEntityParser::parse_column_type_str("Boolean"), ColumnType::Boolean);
        assert_eq!(
            RustEntityParser::parse_column_type_str("DateTime"),
            ColumnType::DateTime
        );
        assert_eq!(RustEntityParser::parse_column_type_str("Json"), ColumnType::Json);
    }
}
