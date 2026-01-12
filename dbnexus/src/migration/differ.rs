// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 迁移差异计算和 SQL 生成
//!
//! 计算模式差异并生成迁移 SQL

use super::schema::*;
use super::types::*;
use crate::config::DatabaseType;

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
                DatabaseType::MySql => def.push_str(" AUTO_INCREMENT"),
                DatabaseType::Sqlite => def.push_str(" PRIMARY KEY AUTOINCREMENT"),
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
            DatabaseType::MySql => {
                format!("ALTER TABLE {} DROP COLUMN {};", table_name, column_name)
            }
            DatabaseType::Postgres => {
                format!("ALTER TABLE {} DROP COLUMN {};", table_name, column_name)
            }
            DatabaseType::Sqlite => {
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
    /// ```rust,no_run
    /// use dbnexus::migration::RustEntityParser;
    ///
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

        let table = RustEntityParser::parse_entity(entity_code, "users").expect("Failed to parse entity code");

        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 3);
        assert_eq!(table.primary_key_columns, vec!["id"]);

        // 检查 id 列
        let id_col = table
            .columns
            .iter()
            .find(|c| c.name == "id")
            .expect("id column should exist");
        assert_eq!(id_col.column_type, ColumnType::Integer);
        assert!(id_col.is_primary_key);

        // 检查 name 列
        let name_col = table
            .columns
            .iter()
            .find(|c| c.name == "name")
            .expect("name column should exist");
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

        let sql = RustEntityParser::generate_migration_sql(entity_code, "posts", DatabaseType::Postgres)
            .expect("Failed to generate migration SQL");

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
