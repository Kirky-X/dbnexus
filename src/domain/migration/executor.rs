// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 迁移执行器
//!
//! 负责执行数据库迁移操作

use super::differ::SqlGenerator;
use super::schema::*;
use crate::foundation::config::DatabaseType;
use crate::foundation::error::DbError;
use sea_orm::{ConnectionTrait, TransactionTrait};
use std::path::PathBuf;

/// 迁移执行器
///
/// 负责执行数据库迁移操作，内部字段已封装以防止未授权访问
pub struct MigrationExecutor {
    /// 数据库连接
    pub connection: sea_orm::DatabaseConnection,
    /// SQL 生成器
    pub(crate) sql_generator: SqlGenerator,
    /// 迁移历史记录
    pub(crate) history: MigrationHistory,
}

fn build_placeholder_list(backend: sea_orm::DbBackend, count: usize) -> String {
    match backend {
        sea_orm::DbBackend::Postgres => (1..=count)
            .map(|index| format!("${}", index))
            .collect::<Vec<_>>()
            .join(", "),
        _ => std::iter::repeat_n("?", count).collect::<Vec<_>>().join(", "),
    }
}

fn build_migration_insert_sql(backend: sea_orm::DbBackend) -> String {
    match backend {
        sea_orm::DbBackend::Postgres => {
            "INSERT INTO dbnexus_migrations (version, description, applied_at, file_path) VALUES ($1, $2, CAST($3 AS TIMESTAMP), $4)".to_string()
        }
        _ => format!(
            "INSERT INTO dbnexus_migrations (version, description, applied_at, file_path) VALUES ({})",
            build_placeholder_list(backend, 4)
        ),
    }
}

fn sql_escape_single_quotes(s: &str) -> String {
    s.replace('\'', "''")
}

fn format_mysql_applied_at(applied_at: time::OffsetDateTime) -> String {
    let applied_at = applied_at.to_offset(time::UtcOffset::UTC);
    match time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]") {
        Ok(format) => applied_at.format(&format).unwrap_or_else(|_| applied_at.to_string()),
        Err(_) => applied_at.to_string(),
    }
}

fn format_applied_at_for_backend(backend: sea_orm::DbBackend, applied_at: time::OffsetDateTime) -> String {
    match backend {
        sea_orm::DbBackend::MySql => format_mysql_applied_at(applied_at),
        _ => applied_at.to_string(),
    }
}

fn parse_mysql_applied_at(value: &str) -> Option<time::OffsetDateTime> {
    let format_with_subseconds =
        time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond]").ok();
    if let Some(format) = format_with_subseconds {
        if let Ok(dt) = time::PrimitiveDateTime::parse(value, &format) {
            return Some(dt.assume_utc());
        }
    }

    let format_without_subseconds =
        time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]").ok();
    if let Some(format) = format_without_subseconds {
        if let Ok(dt) = time::PrimitiveDateTime::parse(value, &format) {
            return Some(dt.assume_utc());
        }
    }

    None
}

fn parse_applied_at_for_db(db_type: DatabaseType, value: &str) -> Option<time::OffsetDateTime> {
    match db_type {
        DatabaseType::MySql => parse_mysql_applied_at(value),
        _ => time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok(),
    }
}

impl MigrationExecutor {
    /// 创建新的迁移执行器
    pub fn new(connection: sea_orm::DatabaseConnection, db_type: DatabaseType) -> Self {
        Self {
            connection,
            sql_generator: SqlGenerator::new(db_type),
            history: MigrationHistory::new(),
        }
    }

    /// 构建迁移历史插入语句（原始 SQL 字符串）
    ///
    /// 根据数据库后端格式化 `applied_at` 并转义文本字段，返回可直接执行的 INSERT 语句。
    #[deprecated(
        since = "0.2.0",
        note = "Use MigrationExecutor::apply_migration_file_public with MigrationFile::new instead"
    )]
    pub fn build_history_insert_sql_raw(
        &self,
        version: u32,
        description: &str,
        applied_at: time::OffsetDateTime,
        file_path: &str,
    ) -> String {
        let backend = match self.sql_generator.db_type {
            DatabaseType::Postgres => sea_orm::DbBackend::Postgres,
            DatabaseType::MySql => sea_orm::DbBackend::MySql,
            DatabaseType::Sqlite => sea_orm::DbBackend::Sqlite,
        };

        let applied_at_value = format_applied_at_for_backend(backend, applied_at);
        let desc_esc = sql_escape_single_quotes(description);
        let path_esc = sql_escape_single_quotes(file_path);

        match backend {
            sea_orm::DbBackend::Postgres => format!(
                "INSERT INTO dbnexus_migrations (version, description, applied_at, file_path) VALUES ({}, '{}', CAST('{}' AS TIMESTAMP), '{}')",
                version, desc_esc, applied_at_value, path_esc
            ),
            _ => format!(
                "INSERT INTO dbnexus_migrations (version, description, applied_at, file_path) VALUES ({}, '{}', '{}', '{}')",
                version, desc_esc, applied_at_value, path_esc
            ),
        }
    }

    /// 获取迁移历史的不可变引用
    ///
    /// 返回迁移历史的只读引用，用于查看已应用的迁移
    pub fn history(&self) -> &MigrationHistory {
        &self.history
    }

    /// 读取数据库中的迁移历史
    pub async fn load_history(&mut self) -> Result<(), DbError> {
        // 确保迁移历史表存在
        self.ensure_migration_table_exists().await?;

        let rows = {
            use sea_orm::sea_query::{Alias, Expr, Order, Query};

            let mut query = Query::select();
            query.from(Alias::new("dbnexus_migrations"));
            query.column(Alias::new("version"));
            query.column(Alias::new("description"));
            query.column(Alias::new("file_path"));

            match self.sql_generator.db_type {
                DatabaseType::Postgres => {
                    query.expr_as(Expr::cust("applied_at::text"), Alias::new("applied_at"));
                }
                DatabaseType::MySql => {
                    query.expr_as(Expr::cust("CAST(applied_at AS CHAR)"), Alias::new("applied_at"));
                }
                DatabaseType::Sqlite => {
                    query.column(Alias::new("applied_at"));
                }
            }

            query.order_by(Alias::new("version"), Order::Asc);

            self.connection.query_all(&query).await.map_err(DbError::Connection)?
        };

        let mut history = MigrationHistory::new();
        for row in rows {
            // 使用更安全的错误处理方式
            let version: Result<i64, _> = row.try_get("", "version");
            let version_val = match version {
                Ok(v) => v,
                Err(_e) => {
                    continue;
                }
            };
            let Ok(version) = u32::try_from(version_val) else {
                continue;
            };

            let description: String = match row.try_get("", "description") {
                Ok(d) => d,
                Err(_e) => String::new(),
            };

            let applied_at_str: String = match row.try_get("", "applied_at") {
                Ok(s) => s,
                Err(_e) => String::new(),
            };
            let applied_at = if applied_at_str.is_empty() {
                time::OffsetDateTime::now_utc()
            } else {
                match parse_applied_at_for_db(self.sql_generator.db_type, &applied_at_str) {
                    Some(dt) => dt,
                    None => time::OffsetDateTime::now_utc(),
                }
            };

            let file_path: String = match row.try_get("", "file_path") {
                Ok(p) => p,
                Err(_e) => String::new(),
            };

            history.add_migration(MigrationVersion {
                version,
                description,
                applied_at,
                file_path,
            });
        }
        self.history = history;

        Ok(())
    }

    /// 确保迁移历史表存在
    async fn ensure_migration_table_exists(&self) -> Result<(), DbError> {
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
            DatabaseType::MySql => {
                "CREATE TABLE IF NOT EXISTS dbnexus_migrations (
                    version INT PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    file_path TEXT
                );"
            }
            DatabaseType::Sqlite => {
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
            .map_err(DbError::Connection)?;
        Ok(())
    }

    /// 应用单个迁移
    pub async fn apply_migration(&mut self, migration: &Migration) -> Result<(), DbError> {
        // 生成迁移 SQL
        let sql = self.sql_generator.generate_migration_sql(migration)?;

        // 开始事务
        let txn = self.connection.begin().await.map_err(DbError::Connection)?;

        // 执行迁移 SQL
        if !sql.is_empty() {
            txn.execute_unprepared(&sql).await.map_err(DbError::Connection)?;
        }

        // 记录迁移历史
        let version_record = MigrationVersion {
            version: migration.version,
            description: migration.description.clone(),
            applied_at: migration.timestamp.unwrap_or_else(time::OffsetDateTime::now_utc),
            file_path: format!("migration_v{}.sql", migration.version),
        };

        // 插入到迁移历史表（使用参数化查询防止 SQL 注入）
        // 使用 Statement::from_sql_and_values 进行参数化查询
        let backend = match self.sql_generator.db_type {
            DatabaseType::Postgres => sea_orm::DbBackend::Postgres,
            DatabaseType::MySql => sea_orm::DbBackend::MySql,
            DatabaseType::Sqlite => sea_orm::DbBackend::Sqlite,
        };

        let insert_sql = build_migration_insert_sql(backend);
        let applied_at_value = format_applied_at_for_backend(backend, version_record.applied_at);

        let stmt = sea_orm::Statement::from_sql_and_values(
            backend,
            insert_sql.to_string(),
            vec![
                migration.version.into(),
                migration.description.clone().into(),
                applied_at_value.into(),
                version_record.file_path.clone().into(),
            ],
        );

        txn.execute_raw(stmt).await.map_err(DbError::Connection)?;
        // 提交事务
        txn.commit().await.map_err(DbError::Connection)?;

        self.history.add_migration(version_record);

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

/// 迁移文件信息
///
/// 存储迁移文件的基本信息，用于扫描和管理迁移文件
#[derive(Debug, Clone)]
pub struct MigrationFile {
    /// 版本号
    pub(crate) version: u32,
    /// 描述
    pub(crate) description: String,
    /// 文件路径
    pub(crate) file_path: PathBuf,
    /// 文件内容
    pub(crate) content: String,
}

impl MigrationFile {
    /// 创建新的迁移文件信息
    ///
    /// # Arguments
    ///
    /// * `version` - 迁移版本号
    /// * `description` - 迁移描述
    /// * `file_path` - 迁移文件路径
    /// * `content` - 迁移文件内容（SQL 语句）
    pub fn new(version: u32, description: String, file_path: PathBuf, content: String) -> Self {
        Self {
            version,
            description,
            file_path,
            content,
        }
    }

    /// 获取迁移版本号
    pub fn version(&self) -> u32 {
        self.version
    }

    /// 获取迁移描述
    pub fn description(&self) -> &str {
        &self.description
    }

    /// 获取文件路径
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    /// 获取文件内容
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// 自动迁移执行器
#[cfg(feature = "auto-migrate")]
impl MigrationExecutor {
    /// 扫描指定目录中的迁移文件
    ///
    /// 迁移文件命名格式: `{version}_{description}.sql`
    ///
    /// # Arguments
    ///
    /// * `dir` - 迁移文件目录路径
    ///
    /// # Returns
    ///
    /// 扫描到的迁移文件列表（按版本号排序）
    pub fn scan_migrations(&self, dir: &std::path::Path) -> Result<Vec<MigrationFile>, DbError> {
        let mut migrations = Vec::new();

        if !dir.exists() {
            return Ok(migrations);
        }

        let entries = std::fs::read_dir(dir)
            .map_err(|e| DbError::Config(format!("Failed to read migration directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| DbError::Config(format!("Failed to read migration entry: {}", e)))?;
            let path = entry.path();

            if path.is_file() && path.extension().map(|e| e == "sql").unwrap_or(false) {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some((version, description)) = Self::parse_filename(filename) {
                        let content = std::fs::read_to_string(&path)
                            .map_err(|e| DbError::Config(format!("Failed to read migration file: {}", e)))?;

                        migrations.push(MigrationFile {
                            version,
                            description,
                            file_path: path,
                            content,
                        });
                    }
                }
            }
        }

        // 按版本号排序
        migrations.sort_by_key(|m| m.version);

        Ok(migrations)
    }

    /// 解析迁移文件名
    pub(crate) fn parse_filename(filename: &str) -> Option<(u32, String)> {
        let parts: Vec<&str> = filename.split('_').collect();
        if parts.is_empty() {
            return None;
        }

        let version = parts[0].parse::<u32>().ok()?;
        let description = parts[1..].join("_").replace(".sql", "");

        Some((version, description))
    }

    /// 运行所有待应用的迁移
    ///
    /// # Arguments
    ///
    /// * `dir` - 迁移文件目录路径
    ///
    /// # Returns
    ///
    /// 成功应用的迁移数量
    pub async fn run_migrations(&mut self, dir: &std::path::Path) -> Result<u32, DbError> {
        // 扫描迁移文件
        let migration_files = self.scan_migrations(dir)?;

        // 批量加载所有已应用的版本（消除 N+1 查询）
        let applied_versions = self.load_applied_versions().await?;

        let pending: Vec<_> = migration_files
            .into_iter()
            .filter(|m| !applied_versions.contains(&m.version))
            .collect();

        if pending.is_empty() {
            return Ok(0);
        }

        // 应用迁移
        let mut applied_count = 0;
        for migration_file in &pending {
            match self.apply_migration_file(migration_file).await {
                Ok(_) => {
                    applied_count += 1;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        Ok(applied_count)
    }

    /// 批量加载所有已应用的迁移版本（消除 N+1 查询）
    ///
    /// 在 `run_migrations` 开始时调用，一次性加载所有已应用的版本。
    /// 避免对每个迁移文件单独调用 `is_migration_applied` 导致的 N+1 查询问题。
    async fn load_applied_versions(&self) -> Result<std::collections::HashSet<u32>, DbError> {
        // 先确保迁移历史表存在
        self.ensure_migration_table_exists().await?;

        use sea_orm::sea_query::{Alias, Query};

        let mut query = Query::select();
        query.column(Alias::new("version"));
        query.from(Alias::new("dbnexus_migrations"));

        let rows = self.connection.query_all(&query).await.map_err(DbError::Connection)?;

        let mut applied_versions = std::collections::HashSet::new();
        for row in rows {
            if let Ok(version) = row.try_get::<i64>("", "version") {
                applied_versions.insert(version as u32);
            }
        }

        Ok(applied_versions)
    }

    /// 应用单个迁移文件
    async fn apply_migration_file(&mut self, migration_file: &MigrationFile) -> Result<(), DbError> {
        // 解析迁移文件内容
        let sql = Self::extract_up_sql(&migration_file.content);

        // 开始事务
        let txn = self.connection.begin().await.map_err(DbError::Connection)?;

        // 执行迁移 SQL
        if !sql.is_empty() {
            txn.execute_unprepared(sql).await.map_err(DbError::Connection)?;
        }

        // 记录迁移历史（使用参数化查询防止 SQL 注入）
        let applied_at = time::OffsetDateTime::now_utc();

        let backend = match self.sql_generator.db_type {
            DatabaseType::Postgres => sea_orm::DbBackend::Postgres,
            DatabaseType::MySql => sea_orm::DbBackend::MySql,
            DatabaseType::Sqlite => sea_orm::DbBackend::Sqlite,
        };
        let insert_sql = build_migration_insert_sql(backend);
        let applied_at_value = format_applied_at_for_backend(backend, applied_at);
        let stmt = sea_orm::Statement::from_sql_and_values(
            backend,
            insert_sql.to_string(),
            vec![
                migration_file.version.into(),
                migration_file.description.clone().into(),
                applied_at_value.into(),
                migration_file.file_path.to_string_lossy().into(),
            ],
        );

        txn.execute_raw(stmt).await.map_err(DbError::Connection)?;

        // 提交事务
        txn.commit().await.map_err(DbError::Connection)?;

        // 添加到历史记录
        self.history.add_migration(MigrationVersion {
            version: migration_file.version,
            description: migration_file.description.clone(),
            applied_at,
            file_path: migration_file.file_path.to_string_lossy().to_string(),
        });

        Ok(())
    }

    #[allow(missing_docs)]
    pub async fn apply_migration_file_public(&mut self, migration_file: &MigrationFile) -> Result<(), DbError> {
        self.apply_migration_file(migration_file).await
    }

    /// 从迁移文件中提取 UP SQL
    fn extract_up_sql(content: &str) -> &str {
        fn find_marker_line(content: &str, markers: &[&str]) -> Option<(usize, usize)> {
            for marker in markers {
                if let Some(pos) = content.find(marker) {
                    let line_start = content[..pos].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
                    let line_end = content[pos..]
                        .find('\n')
                        .map(|idx| pos + idx + 1)
                        .unwrap_or(content.len());
                    return Some((line_start, line_end));
                }
            }
            None
        }

        let up_marker = find_marker_line(content, &["-- UP:", "-- up:", "-- UP", "-- up", "UP:", "UP"]);
        let down_marker = find_marker_line(
            content,
            &["-- DOWN:", "-- down:", "-- DOWN", "-- down", "DOWN:", "DOWN"],
        );

        match (up_marker, down_marker) {
            (Some((_, up_end)), Some((down_start, _))) if down_start > up_end => &content[up_end..down_start],
            (Some((_, up_end)), _) => &content[up_end..],
            (None, Some((down_start, _))) => &content[..down_start],
            (None, None) => content,
        }
        .trim()
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
