// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 迁移差异计算和 SQL 生成
//!
//! 计算模式差异并生成迁移 SQL

use super::schema::*;
use super::types::*;
use crate::foundation::config::DatabaseType;
use regex::Regex;
use std::sync::LazyLock;

/// 验证 SQL 标识符（表名、列名等）
///
/// # Arguments
///
/// * `identifier` - 要验证的标识符
/// * `identifier_type` - 标识符类型描述（用于错误信息）
///
/// # Returns
///
/// 验证通过返回标识符，失败返回错误
fn validate_sql_identifier(identifier: &str, identifier_type: &str) -> Result<String, String> {
    if identifier.is_empty() {
        return Err(format!("{} 不能为空", identifier_type));
    }

    if identifier.len() > 64 {
        return Err(format!("{} 长度不能超过 64 个字符", identifier_type));
    }

    // 验证标识符格式：只允许字母、数字、下划线，且不能以数字开头
    static IDENTIFIER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap());

    if !IDENTIFIER_REGEX.is_match(identifier) {
        return Err(format!(
            "{} '{}' 包含无效字符，只允许字母、数字和下划线，且不能以数字开头",
            identifier_type, identifier
        ));
    }

    // 检查保留关键字
    let reserved_keywords = [
        "select",
        "insert",
        "update",
        "delete",
        "drop",
        "create",
        "alter",
        "table",
        "index",
        "from",
        "where",
        "and",
        "or",
        "not",
        "null",
        "primary",
        "key",
        "foreign",
        "references",
        "constraint",
        "default",
        "unique",
        "check",
        "into",
        "values",
        "set",
        "join",
        "left",
        "right",
        "inner",
        "outer",
    ];

    if reserved_keywords.contains(&identifier.to_lowercase().as_str()) {
        return Err(format!(
            "{} '{}' 是 SQL 保留关键字，不允许使用",
            identifier_type, identifier
        ));
    }

    Ok(identifier.to_string())
}

/// 清理默认值，移除可能的 SQL 注入载荷
fn sanitize_default_value(default: &str) -> String {
    // 如果默认值包含可疑模式，返回安全的默认值
    let suspicious_patterns = [
        "select",
        "insert",
        "update",
        "delete",
        "drop",
        "create",
        "alter",
        "exec",
        "execute",
        "xp_",
        "sp_",
        "--",
        "/*",
        "*/",
        "chr(",
        "char(",
        "concat",
        "union",
        "benchmark",
        "sleep",
    ];

    let lower_default = default.to_lowercase();

    for pattern in &suspicious_patterns {
        if lower_default.contains(pattern) {
            return "'***SANITIZED***'".to_string();
        }
    }

    // 确保默认值被引号包围
    let trimmed = default.trim();

    if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        || (trimmed.starts_with('(') && trimmed.ends_with(')'))
        || trimmed.parse::<i128>().is_ok()
        || trimmed.parse::<f64>().is_ok()
        || trimmed.to_uppercase() == "NULL"
        || trimmed.to_uppercase() == "CURRENT_TIMESTAMP"
        || trimmed.to_uppercase() == "NOW()"
    {
        // 已经有引号、是函数、数字、NULL 或时间戳，不需要额外引号
        trimmed.to_string()
    } else {
        // 其他情况添加单引号
        format!("'{}'", trimmed.replace('\'', "''"))
    }
}

/// 迁移计划
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
    pub fn generate_create_table_sql(&self, table: &Table) -> Result<String, String> {
        // 验证表名
        let table_name = validate_sql_identifier(&table.name, "表名")?;

        let mut sql = format!("CREATE TABLE {} (\n", table_name);

        let column_defs: Vec<String> = table
            .columns
            .iter()
            .map(|col| self.generate_column_definition(col, &table.primary_key_columns))
            .collect::<Result<Vec<_>, _>>()?;

        sql.push_str(&column_defs.join(",\n"));

        // 添加表级主键约束
        // SQLite 的自增主键列已在列定义中包含 PRIMARY KEY AUTOINCREMENT，不应在表级别重复声明
        let columns_with_inline_pk: std::collections::HashSet<&str> = if matches!(self.db_type, DatabaseType::Sqlite | DatabaseType::DuckDb) {
            table
                .columns
                .iter()
                .filter(|c| c.is_auto_increment && c.is_primary_key)
                .map(|c| c.name.as_str())
                .collect()
        } else {
            std::collections::HashSet::new()
        };
        let table_level_pk: Vec<&String> = table
            .primary_key_columns
            .iter()
            .filter(|col| !columns_with_inline_pk.contains(col.as_str()))
            .collect();
        if !table_level_pk.is_empty() {
            sql.push_str(",\n");
            let pk_columns: Vec<String> = table_level_pk
                .iter()
                .map(|col| validate_sql_identifier(col, "主键列名"))
                .collect::<Result<Vec<_>, _>>()?;
            sql.push_str(&format!("    PRIMARY KEY ({})", pk_columns.join(", ")));
        }

        sql.push_str("\n);");

        // 生成索引
        for index in &table.indexes {
            if !index.is_constraint {
                sql.push_str("\n\n");
                sql.push_str(&self.generate_create_index_sql(index)?);
            }
        }

        // 生成外键
        for fk in &table.foreign_keys {
            sql.push_str("\n\n");
            sql.push_str(&self.generate_add_foreign_key_sql(fk)?);
        }

        Ok(sql)
    }

    /// 生成列定义
    fn generate_column_definition(&self, column: &Column, _pk_columns: &[String]) -> Result<String, String> {
        // 验证列名
        let column_name = validate_sql_identifier(&column.name, "列名")?;

        let mut def = format!("    {} {}", column_name, column.column_type.to_sql(self.db_type));

        // 自增列不需要指定
        if column.is_auto_increment && column.is_primary_key {
            match self.db_type {
                DatabaseType::MySql => def.push_str(" AUTO_INCREMENT"),
                DatabaseType::Sqlite => def.push_str(" PRIMARY KEY AUTOINCREMENT"),
                DatabaseType::DuckDb => def.push_str(" PRIMARY KEY"),
                _ => {}
            }
        }

        if !column.is_nullable {
            def.push_str(" NOT NULL");
        }

        if let Some(default) = &column.default_value {
            let sanitized_default = sanitize_default_value(default);
            def.push_str(&format!(" DEFAULT {}", sanitized_default));
        }

        // 主键列如果有自增，不需要单独 PRIMARY KEY
        if column.is_primary_key && !column.is_auto_increment {
            // 主键已在表级别处理
        }

        Ok(def)
    }

    /// 生成创建索引的 SQL
    pub fn generate_create_index_sql(&self, index: &Index) -> Result<String, String> {
        // 验证索引名
        let index_name = validate_sql_identifier(&index.name, "索引名")?;

        // 验证表名
        let table_name = validate_sql_identifier(&index.table_name, "表名")?;

        // 验证列名
        let validated_columns: Vec<String> = index
            .columns
            .iter()
            .map(|col| validate_sql_identifier(col, "索引列名"))
            .collect::<Result<Vec<_>, _>>()?;

        let unique = if index.is_unique { "UNIQUE " } else { "" };
        Ok(format!(
            "CREATE {}INDEX {} ON {} ({})",
            unique,
            index_name,
            table_name,
            validated_columns.join(", ")
        ))
    }

    /// 生成添加外键的 SQL
    fn generate_add_foreign_key_sql(&self, fk: &ForeignKey) -> Result<String, String> {
        // 验证所有标识符
        let table_name = validate_sql_identifier(&fk.table_name, "外键表名")?;
        let constraint_name = validate_sql_identifier(&fk.name, "外键约束名")?;
        let column_name = validate_sql_identifier(&fk.column_name, "外键列名")?;
        let referenced_table_name = validate_sql_identifier(&fk.referenced_table_name, "外键引用表名")?;
        let referenced_column_name = validate_sql_identifier(&fk.referenced_column_name, "外键引用列名")?;

        let mut sql = format!(
            "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({})",
            table_name, constraint_name, column_name, referenced_table_name, referenced_column_name
        );

        if let Some(on_delete) = &fk.on_delete {
            sql.push_str(&format!(" ON DELETE {}", on_delete));
        }

        if let Some(on_update) = &fk.on_update {
            sql.push_str(&format!(" ON UPDATE {}", on_update));
        }

        sql.push(';');
        Ok(sql)
    }

    /// 生成删除表的 SQL
    pub fn generate_drop_table_sql(&self, table_name: &str) -> Result<String, String> {
        let validated_name = validate_sql_identifier(table_name, "表名")?;
        Ok(format!("DROP TABLE {};", validated_name))
    }

    /// 生成添加列的 SQL
    pub fn generate_add_column_sql(&self, table_name: &str, column: &Column) -> Result<String, String> {
        // 验证表名
        let validated_table_name = validate_sql_identifier(table_name, "表名")?;

        let col_def = self.generate_column_definition(column, &Vec::new())?;
        Ok(format!(
            "ALTER TABLE {} ADD {};",
            validated_table_name,
            col_def.trim_start_matches("    ")
        ))
    }

    /// 生成删除列的 SQL
    pub fn generate_drop_column_sql(&self, table_name: &str, column_name: &str) -> Result<String, String> {
        // 验证表名和列名
        let validated_table_name = validate_sql_identifier(table_name, "表名")?;
        let validated_column_name = validate_sql_identifier(column_name, "列名")?;

        match self.db_type {
            DatabaseType::MySql => Ok(format!(
                "ALTER TABLE {} DROP COLUMN {};",
                validated_table_name, validated_column_name
            )),
            DatabaseType::Postgres => Ok(format!(
                "ALTER TABLE {} DROP COLUMN {};",
                validated_table_name, validated_column_name
            )),
            DatabaseType::DuckDb => Ok(format!(
                "ALTER TABLE {} DROP COLUMN {};",
                validated_table_name, validated_column_name
            )),
            DatabaseType::Sqlite => {
                // SQLite 不支持直接删除列，需要重建表
                Ok(format!(
                    "-- SQLite 不支持直接删除列，请手动重建表 {}
 ALTER TABLE {} DROP COLUMN {};",
                    validated_table_name, validated_table_name, validated_column_name
                ))
            }
        }
    }

    /// 生成迁移的完整 SQL
    pub fn generate_migration_sql(&self, migration: &Migration) -> Result<String, String> {
        let mut sql = String::new();

        for change in &migration.table_changes {
            match change {
                TableChange::CreateTable(table) => {
                    sql.push_str(&format!("-- 创建表: {}\n", table.name));
                    sql.push_str(&self.generate_create_table_sql(table)?);
                    sql.push_str("\n\n");
                }
                TableChange::DropTable { table_name } => {
                    sql.push_str(&format!("-- 删除表: {}\n", table_name));
                    sql.push_str(&self.generate_drop_table_sql(table_name)?);
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
                        sql.push_str(&self.generate_add_column_sql(table_name, col)?);
                        sql.push('\n');
                    }

                    for col_name in removed_columns {
                        sql.push_str(&format!("-- 删除列: {}\n", col_name));
                        sql.push_str(&self.generate_drop_column_sql(table_name, col_name)?);
                        sql.push('\n');
                    }

                    for index in added_indexes {
                        sql.push_str(&format!("-- 添加索引: {}\n", index.name));
                        sql.push_str(&self.generate_create_index_sql(index)?);
                        sql.push('\n');
                    }

                    for index_name in removed_indexes {
                        sql.push_str(&format!("-- 删除索引: {}\n", index_name));
                        sql.push_str(&format!("DROP INDEX {};\n", index_name));
                    }

                    for fk in added_foreign_keys {
                        sql.push_str(&format!("-- 添加外键: {}\n", fk.name));
                        sql.push_str(&self.generate_add_foreign_key_sql(fk)?);
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

        Ok(sql.trim_end().to_string())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-020: ColumnType SQL 生成测试
    #[test]
    fn test_column_type_to_sql() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let mysql = SqlGenerator::new(DatabaseType::MySql);
        let sqlite = SqlGenerator::new(DatabaseType::Sqlite);

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
            unreachable!("Expected CreateTable change");
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
            unreachable!("Expected DropTable change");
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

        let sql = pg.generate_create_table_sql(&table).expect("Failed to generate SQL");

        assert!(sql.contains("CREATE TABLE users"));
        assert!(sql.contains("id INTEGER"));
        assert!(sql.contains("name VARCHAR(255)"));
        assert!(sql.contains("NOT NULL"));
        assert!(sql.contains("PRIMARY KEY (id)"));
    }

    // ===== validate_sql_identifier 测试 =====

    #[test]
    fn test_validate_sql_identifier_empty() {
        let result = validate_sql_identifier("", "表名");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("不能为空"));
    }

    #[test]
    fn test_validate_sql_identifier_too_long() {
        let long_name = "a".repeat(65);
        let result = validate_sql_identifier(&long_name, "表名");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("长度不能超过 64"));
    }

    #[test]
    fn test_validate_sql_identifier_invalid_start_with_digit() {
        let result = validate_sql_identifier("1table", "表名");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("无效字符"));
    }

    #[test]
    fn test_validate_sql_identifier_invalid_chars() {
        let result = validate_sql_identifier("table-name", "表名");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("无效字符"));
    }

    #[test]
    fn test_validate_sql_identifier_reserved_keyword() {
        for kw in &["select", "INSERT", "Drop", "TABLE"] {
            let result = validate_sql_identifier(kw, "表名");
            assert!(result.is_err(), "应该拒绝保留关键字: {}", kw);
            assert!(result.unwrap_err().contains("保留关键字"));
        }
    }

    #[test]
    fn test_validate_sql_identifier_valid_underscore_start() {
        let result = validate_sql_identifier("_private_table", "表名");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "_private_table");
    }

    #[test]
    fn test_validate_sql_identifier_valid_max_length() {
        let name = "a".repeat(64);
        let result = validate_sql_identifier(&name, "表名");
        assert!(result.is_ok());
    }

    // ===== sanitize_default_value 测试 =====

    #[test]
    fn test_sanitize_default_value_suspicious_select() {
        let result = sanitize_default_value("SELECT * FROM users");
        assert_eq!(result, "'***SANITIZED***'");
    }

    #[test]
    fn test_sanitize_default_value_suspicious_drop() {
        let result = sanitize_default_value("DROP TABLE users");
        assert_eq!(result, "'***SANITIZED***'");
    }

    #[test]
    fn test_sanitize_default_value_suspicious_comment() {
        let result = sanitize_default_value("value--comment");
        assert_eq!(result, "'***SANITIZED***'");
    }

    #[test]
    fn test_sanitize_default_value_null() {
        let result = sanitize_default_value("NULL");
        assert_eq!(result, "NULL");
    }

    #[test]
    fn test_sanitize_default_value_current_timestamp() {
        let result = sanitize_default_value("CURRENT_TIMESTAMP");
        assert_eq!(result, "CURRENT_TIMESTAMP");
    }

    #[test]
    fn test_sanitize_default_value_now_function() {
        let result = sanitize_default_value("NOW()");
        assert_eq!(result, "NOW()");
    }

    #[test]
    fn test_sanitize_default_value_integer() {
        let result = sanitize_default_value("42");
        assert_eq!(result, "42");
    }

    #[test]
    fn test_sanitize_default_value_float() {
        let result = sanitize_default_value("3.14");
        assert_eq!(result, "3.14");
    }

    #[test]
    fn test_sanitize_default_value_negative_integer() {
        let result = sanitize_default_value("-100");
        assert_eq!(result, "-100");
    }

    #[test]
    fn test_sanitize_default_value_quoted_string() {
        let result = sanitize_default_value("'hello'");
        assert_eq!(result, "'hello'");
    }

    #[test]
    fn test_sanitize_default_value_paren_expression() {
        let result = sanitize_default_value("(1 + 2)");
        assert_eq!(result, "(1 + 2)");
    }

    #[test]
    fn test_sanitize_default_value_plain_string_needs_quotes() {
        let result = sanitize_default_value("hello");
        assert_eq!(result, "'hello'");
    }

    #[test]
    fn test_sanitize_default_value_string_with_single_quote() {
        let result = sanitize_default_value("O'Brien");
        assert_eq!(result, "'O''Brien'");
    }

    // ===== SchemaDiffer 测试 =====

    fn make_column(name: &str, col_type: ColumnType, nullable: bool, default: Option<&str>) -> Column {
        Column {
            name: name.to_string(),
            column_type: col_type,
            is_primary_key: false,
            is_nullable: nullable,
            has_default: default.is_some(),
            default_value: default.map(|s| s.to_string()),
            is_auto_increment: false,
            comment: None,
        }
    }

    fn make_table(name: &str, columns: Vec<Column>) -> Table {
        Table {
            name: name.to_string(),
            columns,
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        }
    }

    #[test]
    fn test_diff_no_changes() {
        let table = make_table("users", vec![make_column("id", ColumnType::Integer, false, None)]);
        let mut old_schema = Schema::new(DatabaseType::Postgres);
        let mut new_schema = Schema::new(DatabaseType::Postgres);
        old_schema.add_table(table.clone());
        new_schema.add_table(table);

        let differ = SchemaDiffer::new(old_schema, new_schema);
        let migrations = differ.diff();
        assert!(migrations.is_empty());
    }

    #[test]
    fn test_diff_column_type_changed() {
        let old_table = make_table("users", vec![make_column("age", ColumnType::Integer, true, None)]);
        let new_table = make_table("users", vec![make_column("age", ColumnType::BigInteger, true, None)]);
        let mut old_schema = Schema::new(DatabaseType::Postgres);
        let mut new_schema = Schema::new(DatabaseType::Postgres);
        old_schema.add_table(old_table);
        new_schema.add_table(new_table);

        let differ = SchemaDiffer::new(old_schema, new_schema);
        let migrations = differ.diff();
        assert_eq!(migrations.len(), 1);
        if let TableChange::AlterTable { column_changes, .. } = &migrations[0].table_changes[0] {
            assert!(column_changes.iter().any(|c| matches!(
                c,
                ColumnChange::TypeChanged { .. }
            )));
        } else {
            unreachable!("Expected AlterTable");
        }
    }

    #[test]
    fn test_diff_column_nullability_changed() {
        let old_table = make_table("users", vec![make_column("name", ColumnType::String(None), true, None)]);
        let new_table = make_table("users", vec![make_column("name", ColumnType::String(None), false, None)]);
        let mut old_schema = Schema::new(DatabaseType::Postgres);
        let mut new_schema = Schema::new(DatabaseType::Postgres);
        old_schema.add_table(old_table);
        new_schema.add_table(new_table);

        let differ = SchemaDiffer::new(old_schema, new_schema);
        let migrations = differ.diff();
        assert_eq!(migrations.len(), 1);
        if let TableChange::AlterTable { column_changes, .. } = &migrations[0].table_changes[0] {
            assert!(column_changes.iter().any(|c| matches!(
                c,
                ColumnChange::NullabilityChanged { new_nullable: false, .. }
            )));
        } else {
            unreachable!("Expected AlterTable");
        }
    }

    #[test]
    fn test_diff_column_default_changed() {
        let old_table = make_table("users", vec![make_column("status", ColumnType::Integer, false, Some("0"))]);
        let new_table = make_table("users", vec![make_column("status", ColumnType::Integer, false, Some("1"))]);
        let mut old_schema = Schema::new(DatabaseType::Postgres);
        let mut new_schema = Schema::new(DatabaseType::Postgres);
        old_schema.add_table(old_table);
        new_schema.add_table(new_table);

        let differ = SchemaDiffer::new(old_schema, new_schema);
        let migrations = differ.diff();
        assert_eq!(migrations.len(), 1);
        if let TableChange::AlterTable { column_changes, .. } = &migrations[0].table_changes[0] {
            assert!(column_changes.iter().any(|c| matches!(
                c,
                ColumnChange::DefaultChanged { .. }
            )));
        } else {
            unreachable!("Expected AlterTable");
        }
    }

    #[test]
    fn test_diff_added_and_removed_columns() {
        let old_table = make_table(
            "users",
            vec![
                make_column("id", ColumnType::Integer, false, None),
                make_column("old_col", ColumnType::Text, true, None),
            ],
        );
        let new_table = make_table(
            "users",
            vec![
                make_column("id", ColumnType::Integer, false, None),
                make_column("new_col", ColumnType::Text, true, None),
            ],
        );
        let mut old_schema = Schema::new(DatabaseType::Postgres);
        let mut new_schema = Schema::new(DatabaseType::Postgres);
        old_schema.add_table(old_table);
        new_schema.add_table(new_table);

        let differ = SchemaDiffer::new(old_schema, new_schema);
        let migrations = differ.diff();
        assert_eq!(migrations.len(), 1);
        if let TableChange::AlterTable {
            added_columns,
            removed_columns,
            ..
        } = &migrations[0].table_changes[0] {
            assert_eq!(added_columns.len(), 1);
            assert_eq!(added_columns[0].name, "new_col");
            assert_eq!(removed_columns.len(), 1);
            assert_eq!(removed_columns[0], "old_col");
        } else {
            unreachable!("Expected AlterTable");
        }
    }

    #[test]
    fn test_diff_added_and_removed_indexes() {
        let old_index = Index {
            name: "idx_old".to_string(),
            table_name: "users".to_string(),
            columns: vec!["old_col".to_string()],
            is_unique: false,
            is_constraint: false,
        };
        let new_index = Index {
            name: "idx_new".to_string(),
            table_name: "users".to_string(),
            columns: vec!["new_col".to_string()],
            is_unique: true,
            is_constraint: false,
        };
        let old_table = Table {
            name: "users".to_string(),
            columns: vec![make_column("id", ColumnType::Integer, false, None)],
            primary_key_columns: vec![],
            indexes: vec![old_index],
            foreign_keys: vec![],
            comment: None,
        };
        let new_table = Table {
            name: "users".to_string(),
            columns: vec![make_column("id", ColumnType::Integer, false, None)],
            primary_key_columns: vec![],
            indexes: vec![new_index],
            foreign_keys: vec![],
            comment: None,
        };
        let mut old_schema = Schema::new(DatabaseType::Postgres);
        let mut new_schema = Schema::new(DatabaseType::Postgres);
        old_schema.add_table(old_table);
        new_schema.add_table(new_table);

        let differ = SchemaDiffer::new(old_schema, new_schema);
        let migrations = differ.diff();
        assert_eq!(migrations.len(), 1);
        if let TableChange::AlterTable {
            added_indexes,
            removed_indexes,
            ..
        } = &migrations[0].table_changes[0] {
            assert_eq!(added_indexes.len(), 1);
            assert_eq!(added_indexes[0].name, "idx_new");
            assert_eq!(removed_indexes.len(), 1);
            assert_eq!(removed_indexes[0], "idx_old");
        } else {
            unreachable!("Expected AlterTable");
        }
    }

    #[test]
    fn test_diff_added_and_removed_foreign_keys() {
        let old_fk = ForeignKey {
            name: "fk_old".to_string(),
            table_name: "users".to_string(),
            column_name: "old_id".to_string(),
            referenced_table_name: "old_refs".to_string(),
            referenced_column_name: "id".to_string(),
            on_delete: None,
            on_update: None,
        };
        let new_fk = ForeignKey {
            name: "fk_new".to_string(),
            table_name: "users".to_string(),
            column_name: "new_id".to_string(),
            referenced_table_name: "new_refs".to_string(),
            referenced_column_name: "id".to_string(),
            on_delete: Some(ForeignKeyAction::Cascade),
            on_update: Some(ForeignKeyAction::Restrict),
        };
        let old_table = Table {
            name: "users".to_string(),
            columns: vec![make_column("id", ColumnType::Integer, false, None)],
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![old_fk],
            comment: None,
        };
        let new_table = Table {
            name: "users".to_string(),
            columns: vec![make_column("id", ColumnType::Integer, false, None)],
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![new_fk],
            comment: None,
        };
        let mut old_schema = Schema::new(DatabaseType::Postgres);
        let mut new_schema = Schema::new(DatabaseType::Postgres);
        old_schema.add_table(old_table);
        new_schema.add_table(new_table);

        let differ = SchemaDiffer::new(old_schema, new_schema);
        let migrations = differ.diff();
        assert_eq!(migrations.len(), 1);
        if let TableChange::AlterTable {
            added_foreign_keys,
            removed_foreign_keys,
            ..
        } = &migrations[0].table_changes[0] {
            assert_eq!(added_foreign_keys.len(), 1);
            assert_eq!(added_foreign_keys[0].name, "fk_new");
            assert_eq!(removed_foreign_keys.len(), 1);
            assert_eq!(removed_foreign_keys[0], "fk_old");
        } else {
            unreachable!("Expected AlterTable");
        }
    }

    // ===== SqlGenerator 测试 =====

    #[test]
    fn test_generate_create_table_sql_with_indexes_and_fks() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let table = Table {
            name: "orders".to_string(),
            columns: vec![
                make_column("id", ColumnType::Integer, false, None),
                make_column("user_id", ColumnType::Integer, false, None),
            ],
            primary_key_columns: vec!["id".to_string()],
            indexes: vec![Index {
                name: "idx_user_id".to_string(),
                table_name: "orders".to_string(),
                columns: vec!["user_id".to_string()],
                is_unique: false,
                is_constraint: false,
            }],
            foreign_keys: vec![ForeignKey {
                name: "fk_user".to_string(),
                table_name: "orders".to_string(),
                column_name: "user_id".to_string(),
                referenced_table_name: "users".to_string(),
                referenced_column_name: "id".to_string(),
                on_delete: Some(ForeignKeyAction::Cascade),
                on_update: None,
            }],
            comment: None,
        };

        let sql = pg.generate_create_table_sql(&table).expect("SQL generation failed");
        assert!(sql.contains("CREATE TABLE orders"));
        assert!(sql.contains("PRIMARY KEY (id)"));
        assert!(sql.contains("CREATE INDEX idx_user_id ON orders (user_id)"));
        assert!(sql.contains("ALTER TABLE orders ADD CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(id)"));
        assert!(sql.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn test_generate_create_table_sql_unique_index() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let table = Table {
            name: "users".to_string(),
            columns: vec![make_column("email", ColumnType::String(None), false, None)],
            primary_key_columns: vec![],
            indexes: vec![Index {
                name: "idx_email".to_string(),
                table_name: "users".to_string(),
                columns: vec!["email".to_string()],
                is_unique: true,
                is_constraint: false,
            }],
            foreign_keys: vec![],
            comment: None,
        };

        let sql = pg.generate_create_table_sql(&table).expect("SQL generation failed");
        assert!(sql.contains("CREATE UNIQUE INDEX idx_email ON users (email)"));
    }

    #[test]
    fn test_generate_create_table_sql_invalid_name() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let table = Table {
            name: "1invalid".to_string(),
            columns: vec![],
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };
        let result = pg.generate_create_table_sql(&table);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_create_table_sql_mysql_auto_increment() {
        let mysql = SqlGenerator::new(DatabaseType::MySql);
        let table = Table {
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

        let sql = mysql.generate_create_table_sql(&table).expect("SQL generation failed");
        assert!(sql.contains("AUTO_INCREMENT"));
    }

    #[test]
    fn test_generate_create_table_sql_sqlite_auto_increment() {
        let sqlite = SqlGenerator::new(DatabaseType::Sqlite);
        let table = Table {
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

        let sql = sqlite.generate_create_table_sql(&table).expect("SQL generation failed");
        assert!(sql.contains("PRIMARY KEY AUTOINCREMENT"));
    }

    #[test]
    fn test_generate_create_table_sql_with_default_value() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let table = Table {
            name: "users".to_string(),
            columns: vec![Column {
                name: "status".to_string(),
                column_type: ColumnType::Integer,
                is_primary_key: false,
                is_nullable: false,
                has_default: true,
                default_value: Some("0".to_string()),
                is_auto_increment: false,
                comment: None,
            }],
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };

        let sql = pg.generate_create_table_sql(&table).expect("SQL generation failed");
        assert!(sql.contains("DEFAULT 0"));
    }

    #[test]
    fn test_generate_create_table_sql_nullable_column() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let table = Table {
            name: "users".to_string(),
            columns: vec![make_column("bio", ColumnType::Text, true, None)],
            primary_key_columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        };

        let sql = pg.generate_create_table_sql(&table).expect("SQL generation failed");
        // nullable 列不应包含 NOT NULL
        assert!(!sql.contains("NOT NULL"));
    }

    #[test]
    fn test_generate_create_index_sql_basic() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let index = Index {
            name: "idx_name".to_string(),
            table_name: "users".to_string(),
            columns: vec!["name".to_string()],
            is_unique: false,
            is_constraint: false,
        };

        let sql = pg.generate_create_index_sql(&index).expect("SQL generation failed");
        assert_eq!(sql, "CREATE INDEX idx_name ON users (name)");
    }

    #[test]
    fn test_generate_create_index_sql_unique_multi_column() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let index = Index {
            name: "idx_multi".to_string(),
            table_name: "users".to_string(),
            columns: vec!["first_name".to_string(), "last_name".to_string()],
            is_unique: true,
            is_constraint: false,
        };

        let sql = pg.generate_create_index_sql(&index).expect("SQL generation failed");
        assert_eq!(sql, "CREATE UNIQUE INDEX idx_multi ON users (first_name, last_name)");
    }

    #[test]
    fn test_generate_create_index_sql_invalid_name() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let index = Index {
            name: "1invalid".to_string(),
            table_name: "users".to_string(),
            columns: vec!["name".to_string()],
            is_unique: false,
            is_constraint: false,
        };

        let result = pg.generate_create_index_sql(&index);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_create_index_sql_invalid_column() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let index = Index {
            name: "idx".to_string(),
            table_name: "users".to_string(),
            columns: vec!["invalid-col".to_string()],
            is_unique: false,
            is_constraint: false,
        };

        let result = pg.generate_create_index_sql(&index);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_drop_table_sql() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let sql = pg.generate_drop_table_sql("users").expect("SQL generation failed");
        assert_eq!(sql, "DROP TABLE users;");
    }

    #[test]
    fn test_generate_drop_table_sql_invalid_name() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let result = pg.generate_drop_table_sql("1invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_add_column_sql() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let column = make_column("age", ColumnType::Integer, true, None);
        let sql = pg.generate_add_column_sql("users", &column).expect("SQL generation failed");
        assert_eq!(sql, "ALTER TABLE users ADD age INTEGER;");
    }

    #[test]
    fn test_generate_add_column_sql_with_default() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let column = Column {
            name: "status".to_string(),
            column_type: ColumnType::Integer,
            is_primary_key: false,
            is_nullable: false,
            has_default: true,
            default_value: Some("0".to_string()),
            is_auto_increment: false,
            comment: None,
        };
        let sql = pg.generate_add_column_sql("users", &column).expect("SQL generation failed");
        assert!(sql.contains("DEFAULT 0"));
    }

    #[test]
    fn test_generate_add_column_sql_invalid_table() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let column = make_column("age", ColumnType::Integer, true, None);
        let result = pg.generate_add_column_sql("1invalid", &column);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_drop_column_sql_mysql() {
        let mysql = SqlGenerator::new(DatabaseType::MySql);
        let sql = mysql.generate_drop_column_sql("users", "age").expect("SQL generation failed");
        assert_eq!(sql, "ALTER TABLE users DROP COLUMN age;");
    }

    #[test]
    fn test_generate_drop_column_sql_postgres() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let sql = pg.generate_drop_column_sql("users", "age").expect("SQL generation failed");
        assert_eq!(sql, "ALTER TABLE users DROP COLUMN age;");
    }

    #[test]
    fn test_generate_drop_column_sql_sqlite() {
        let sqlite = SqlGenerator::new(DatabaseType::Sqlite);
        let sql = sqlite.generate_drop_column_sql("users", "age").expect("SQL generation failed");
        assert!(sql.contains("-- SQLite 不支持直接删除列"));
        assert!(sql.contains("ALTER TABLE users DROP COLUMN age;"));
    }

    #[test]
    fn test_generate_drop_column_sql_invalid_table() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let result = pg.generate_drop_column_sql("1invalid", "age");
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_migration_sql_create_table() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let mut migration = Migration::new(1, "create users".to_string());
        migration.add_table_change(TableChange::CreateTable(make_table(
            "users",
            vec![make_column("id", ColumnType::Integer, false, None)],
        )));

        let sql = pg.generate_migration_sql(&migration).expect("SQL generation failed");
        assert!(sql.contains("-- 创建表: users"));
        assert!(sql.contains("CREATE TABLE users"));
    }

    #[test]
    fn test_generate_migration_sql_drop_table() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let mut migration = Migration::new(1, "drop users".to_string());
        migration.add_table_change(TableChange::DropTable {
            table_name: "users".to_string(),
        });

        let sql = pg.generate_migration_sql(&migration).expect("SQL generation failed");
        assert!(sql.contains("-- 删除表: users"));
        assert!(sql.contains("DROP TABLE users;"));
    }

    #[test]
    fn test_generate_migration_sql_alter_table_full() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let mut migration = Migration::new(1, "alter users".to_string());
        migration.add_table_change(TableChange::AlterTable {
            table_name: "users".to_string(),
            column_changes: vec![],
            added_columns: vec![make_column("age", ColumnType::Integer, true, None)],
            removed_columns: vec!["old_col".to_string()],
            added_indexes: vec![Index {
                name: "idx_age".to_string(),
                table_name: "users".to_string(),
                columns: vec!["age".to_string()],
                is_unique: false,
                is_constraint: false,
            }],
            removed_indexes: vec!["idx_old".to_string()],
            added_foreign_keys: vec![ForeignKey {
                name: "fk_role".to_string(),
                table_name: "users".to_string(),
                column_name: "role_id".to_string(),
                referenced_table_name: "roles".to_string(),
                referenced_column_name: "id".to_string(),
                on_delete: Some(ForeignKeyAction::SetNull),
                on_update: None,
            }],
            removed_foreign_keys: vec!["fk_old".to_string()],
        });

        let sql = pg.generate_migration_sql(&migration).expect("SQL generation failed");
        assert!(sql.contains("-- 修改表: users"));
        assert!(sql.contains("-- 添加列: age"));
        assert!(sql.contains("ALTER TABLE users ADD age INTEGER;"));
        assert!(sql.contains("-- 删除列: old_col"));
        assert!(sql.contains("ALTER TABLE users DROP COLUMN old_col;"));
        assert!(sql.contains("-- 添加索引: idx_age"));
        assert!(sql.contains("CREATE INDEX idx_age ON users (age)"));
        assert!(sql.contains("-- 删除索引: idx_old"));
        assert!(sql.contains("DROP INDEX idx_old;"));
        assert!(sql.contains("-- 添加外键: fk_role"));
        assert!(sql.contains("ALTER TABLE users ADD CONSTRAINT fk_role FOREIGN KEY (role_id) REFERENCES roles(id)"));
        assert!(sql.contains("ON DELETE SET NULL"));
        assert!(sql.contains("-- 删除外键: fk_old"));
        assert!(sql.contains("ALTER TABLE users DROP CONSTRAINT fk_old;"));
    }

    #[test]
    fn test_generate_migration_sql_empty() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let migration = Migration::new(1, "empty".to_string());

        let sql = pg.generate_migration_sql(&migration).expect("SQL generation failed");
        assert!(sql.is_empty());
    }

    #[test]
    fn test_generate_migration_sql_invalid_table_in_create() {
        let pg = SqlGenerator::new(DatabaseType::Postgres);
        let mut migration = Migration::new(1, "bad".to_string());
        migration.add_table_change(TableChange::CreateTable(make_table(
            "1invalid",
            vec![make_column("id", ColumnType::Integer, false, None)],
        )));

        let result = pg.generate_migration_sql(&migration);
        assert!(result.is_err());
    }

    // ===== MigrationCommand / MigrationPlan 类型测试 =====

    #[test]
    fn test_migration_direction_variants() {
        let up = MigrationDirection::Up;
        let down = MigrationDirection::Down;
        assert!(matches!(up, MigrationDirection::Up));
        assert!(matches!(down, MigrationDirection::Down));
    }

    #[test]
    fn test_migration_plan_construction() {
        let plan = MigrationPlan {
            migrations: vec![Migration::new(1, "v1".to_string())],
            direction: MigrationDirection::Up,
        };
        assert_eq!(plan.migrations.len(), 1);
        assert!(matches!(plan.direction, MigrationDirection::Up));
    }

    #[test]
    fn test_migration_command_create() {
        let cmd = MigrationCommand::Create {
            description: "init".to_string(),
            directory: "migrations".to_string(),
        };
        match cmd {
            MigrationCommand::Create { description, directory } => {
                assert_eq!(description, "init");
                assert_eq!(directory, "migrations");
            }
            _ => panic!("expected Create"),
        }
    }

    #[test]
    fn test_migration_command_up_with_target() {
        let cmd = MigrationCommand::Up { target_version: Some(5) };
        match cmd {
            MigrationCommand::Up { target_version: Some(5) } => {}
            _ => panic!("expected Up with target 5"),
        }
    }

    #[test]
    fn test_migration_command_up_no_target() {
        let cmd = MigrationCommand::Up { target_version: None };
        match cmd {
            MigrationCommand::Up { target_version: None } => {}
            _ => panic!("expected Up with no target"),
        }
    }

    #[test]
    fn test_migration_command_down() {
        let cmd = MigrationCommand::Down { target_version: Some(2) };
        match cmd {
            MigrationCommand::Down { target_version: Some(2) } => {}
            _ => panic!("expected Down with target 2"),
        }
    }

    #[test]
    fn test_migration_command_status() {
        let cmd = MigrationCommand::Status;
        assert!(matches!(cmd, MigrationCommand::Status));
    }

    #[test]
    fn test_migration_command_generate() {
        let cmd = MigrationCommand::Generate {
            from_schema: "old".to_string(),
            to_schema: "new".to_string(),
            output_file: "out.sql".to_string(),
        };
        match cmd {
            MigrationCommand::Generate { from_schema, to_schema, output_file } => {
                assert_eq!(from_schema, "old");
                assert_eq!(to_schema, "new");
                assert_eq!(output_file, "out.sql");
            }
            _ => panic!("expected Generate"),
        }
    }
}
