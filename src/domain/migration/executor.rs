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
    #[allow(deprecated)]
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
    #[allow(deprecated)]
    let format_with_subseconds =
        time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond]").ok();
    if let Some(format) = format_with_subseconds {
        if let Ok(dt) = time::PrimitiveDateTime::parse(value, &format) {
            return Some(dt.assume_utc());
        }
    }

    #[allow(deprecated)]
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
            DatabaseType::DuckDb => sea_orm::DbBackend::Postgres,
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
                DatabaseType::DuckDb => {
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

            let description: String = row.try_get("", "description").unwrap_or_default();

            let applied_at_str: String = row.try_get("", "applied_at").unwrap_or_default();
            let applied_at = if applied_at_str.is_empty() {
                time::OffsetDateTime::now_utc()
            } else {
                match parse_applied_at_for_db(self.sql_generator.db_type, &applied_at_str) {
                    Some(dt) => dt,
                    None => time::OffsetDateTime::now_utc(),
                }
            };

            let file_path: String = row.try_get("", "file_path").unwrap_or_default();

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
    ///
    /// PostgreSQL 已知问题：并发 `CREATE TABLE IF NOT EXISTS` 可能因 `pg_type` 类型注册冲突
    /// 失败（SQLSTATE 23505，`pg_type_typname_nsp_index`）。`IF NOT EXISTS` 仅跳过表已存在的
    /// 情况，不保护并发类型注册。此方法在捕获该错误后等待 50ms 再重试，确保另一会话的
    /// `CREATE TABLE` 已提交，使重试的 `IF NOT EXISTS` 成为真正的 no-op。
    async fn ensure_migration_table_exists(&self) -> Result<(), DbError> {
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
            DatabaseType::DuckDb => {
                "CREATE TABLE IF NOT EXISTS dbnexus_migrations (
                    version INTEGER PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    file_path TEXT
                );"
            }
        };

        match self.connection.execute_unprepared(create_table_sql).await {
            Ok(_) => {
                eprintln!("[DEBUG ensure_migration_table_exists] CREATE TABLE succeeded");
                Ok(())
            }
            Err(e) => {
                let err_str = e.to_string();
                eprintln!("[DEBUG ensure_migration_table_exists] error: {err_str}");
                if err_str.contains("pg_type_typname_nsp_index") {
                    // 等待并发 CREATE TABLE 提交后重试，使 IF NOT EXISTS 成为 no-op
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    match self.connection.execute_unprepared(create_table_sql).await {
                        Ok(_) => {
                            eprintln!("[DEBUG ensure_migration_table_exists] retry succeeded");
                            Ok(())
                        }
                        Err(e2) => {
                            let err_str2 = e2.to_string();
                            eprintln!("[DEBUG ensure_migration_table_exists] retry error: {err_str2}");
                            // 重试后仍为 pg_type 冲突或表已存在，视为成功
                            if err_str2.contains("pg_type_typname_nsp_index") || err_str2.contains("already exists") {
                                eprintln!(
                                    "[DEBUG ensure_migration_table_exists] treating as success (pg_type/already exists)"
                                );
                                Ok(())
                            } else {
                                Err(DbError::Connection(e2))
                            }
                        }
                    }
                } else {
                    Err(DbError::Connection(e))
                }
            }
        }
    }

    /// 应用单个迁移
    pub async fn apply_migration(&mut self, migration: &Migration) -> Result<(), DbError> {
        // 确保迁移历史表存在
        self.ensure_migration_table_exists().await?;

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
            DatabaseType::DuckDb => sea_orm::DbBackend::Postgres,
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

        eprintln!(
            "[DEBUG run_migrations] applied_versions={:?}, migration_files={:?}",
            applied_versions,
            migration_files.iter().map(|m| m.version).collect::<Vec<_>>()
        );

        let pending: Vec<_> = migration_files
            .into_iter()
            .filter(|m| !applied_versions.contains(&m.version))
            .collect();

        eprintln!("[DEBUG run_migrations] pending count={}", pending.len());

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
            match row.try_get::<i64>("", "version") {
                Ok(version) => {
                    applied_versions.insert(version as u32);
                }
                Err(e) => {
                    eprintln!("[DEBUG load_applied_versions] try_get failed: {e}");
                }
            }
        }

        eprintln!(
            "[DEBUG load_applied_versions] loaded {} versions: {:?}",
            applied_versions.len(),
            applied_versions
        );

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
            DatabaseType::DuckDb => sea_orm::DbBackend::Postgres,
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
            if let Some(stripped) = trimmed.strip_prefix("-- Migration:") {
                return stripped.trim().to_string();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::migration::types::TableChange;

    // =====================================================================
    // build_placeholder_list
    // =====================================================================

    #[test]
    fn test_build_placeholder_list_postgres() {
        let result = build_placeholder_list(sea_orm::DbBackend::Postgres, 3);
        assert_eq!(result, "$1, $2, $3");
    }

    #[test]
    fn test_build_placeholder_list_postgres_single() {
        let result = build_placeholder_list(sea_orm::DbBackend::Postgres, 1);
        assert_eq!(result, "$1");
    }

    #[test]
    fn test_build_placeholder_list_sqlite() {
        let result = build_placeholder_list(sea_orm::DbBackend::Sqlite, 4);
        assert_eq!(result, "?, ?, ?, ?");
    }

    #[test]
    fn test_build_placeholder_list_mysql() {
        let result = build_placeholder_list(sea_orm::DbBackend::MySql, 2);
        assert_eq!(result, "?, ?");
    }

    #[test]
    fn test_build_placeholder_list_zero() {
        assert_eq!(build_placeholder_list(sea_orm::DbBackend::Postgres, 0), "");
        assert_eq!(build_placeholder_list(sea_orm::DbBackend::Sqlite, 0), "");
    }

    // =====================================================================
    // build_migration_insert_sql
    // =====================================================================

    #[test]
    fn test_build_migration_insert_sql_postgres() {
        let sql = build_migration_insert_sql(sea_orm::DbBackend::Postgres);
        assert!(sql.contains("INSERT INTO dbnexus_migrations"));
        assert!(sql.contains("$1, $2, CAST($3 AS TIMESTAMP), $4"));
    }

    #[test]
    fn test_build_migration_insert_sql_sqlite() {
        let sql = build_migration_insert_sql(sea_orm::DbBackend::Sqlite);
        assert!(sql.contains("INSERT INTO dbnexus_migrations"));
        assert!(sql.contains("?, ?, ?, ?"));
    }

    #[test]
    fn test_build_migration_insert_sql_mysql() {
        let sql = build_migration_insert_sql(sea_orm::DbBackend::MySql);
        assert!(sql.contains("INSERT INTO dbnexus_migrations"));
        assert!(sql.contains("?, ?, ?, ?"));
    }

    // =====================================================================
    // sql_escape_single_quotes
    // =====================================================================

    #[test]
    fn test_sql_escape_single_quotes_no_quotes() {
        assert_eq!(sql_escape_single_quotes("hello world"), "hello world");
    }

    #[test]
    fn test_sql_escape_single_quotes_single_quote() {
        assert_eq!(sql_escape_single_quotes("it's"), "it''s");
    }

    #[test]
    fn test_sql_escape_single_quotes_multiple_quotes() {
        assert_eq!(sql_escape_single_quotes("'a'b'"), "''a''b''");
    }

    #[test]
    fn test_sql_escape_single_quotes_empty() {
        assert_eq!(sql_escape_single_quotes(""), "");
    }

    // =====================================================================
    // format_mysql_applied_at
    // =====================================================================

    #[test]
    fn test_format_mysql_applied_at() {
        let dt = time::Date::from_calendar_date(2026, time::Month::June, 25)
            .unwrap()
            .with_hms(12, 30, 45)
            .unwrap()
            .assume_utc();
        let result = format_mysql_applied_at(dt);
        assert_eq!(result, "2026-06-25 12:30:45");
    }

    // =====================================================================
    // format_applied_at_for_backend
    // =====================================================================

    #[test]
    fn test_format_applied_at_for_backend_mysql() {
        let dt = time::Date::from_calendar_date(2026, time::Month::January, 1)
            .unwrap()
            .with_hms(0, 0, 0)
            .unwrap()
            .assume_utc();
        let result = format_applied_at_for_backend(sea_orm::DbBackend::MySql, dt);
        assert_eq!(result, "2026-01-01 00:00:00");
    }

    #[test]
    fn test_format_applied_at_for_backend_non_mysql() {
        let dt = time::Date::from_calendar_date(2026, time::Month::January, 1)
            .unwrap()
            .with_hms(0, 0, 0)
            .unwrap()
            .assume_utc();
        let result = format_applied_at_for_backend(sea_orm::DbBackend::Sqlite, dt);
        // 非 MySQL 使用 OffsetDateTime::to_string()（Rfc3339 格式）
        assert!(result.contains("2026-01-01"));
    }

    // =====================================================================
    // parse_mysql_applied_at
    // =====================================================================

    #[test]
    fn test_parse_mysql_applied_at_without_subseconds() {
        let result = parse_mysql_applied_at("2026-06-25 12:30:45");
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2026);
        assert_eq!(dt.month(), time::Month::June);
        assert_eq!(dt.day(), 25);
    }

    #[test]
    fn test_parse_mysql_applied_at_with_subseconds() {
        let result = parse_mysql_applied_at("2026-06-25 12:30:45.123");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_mysql_applied_at_invalid() {
        assert!(parse_mysql_applied_at("not a date").is_none());
        assert!(parse_mysql_applied_at("").is_none());
    }

    // =====================================================================
    // parse_applied_at_for_db
    // =====================================================================

    #[test]
    fn test_parse_applied_at_for_db_mysql() {
        let result = parse_applied_at_for_db(DatabaseType::MySql, "2026-06-25 12:30:45");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_applied_at_for_db_sqlite_rfc3339() {
        let result = parse_applied_at_for_db(DatabaseType::Sqlite, "2026-06-25T12:30:45Z");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_applied_at_for_db_invalid() {
        assert!(parse_applied_at_for_db(DatabaseType::Sqlite, "invalid").is_none());
        assert!(parse_applied_at_for_db(DatabaseType::MySql, "invalid").is_none());
    }

    // =====================================================================
    // MigrationFile
    // =====================================================================

    #[test]
    fn test_migration_file_new_and_getters() {
        let file = MigrationFile::new(
            1,
            "create_users".to_string(),
            PathBuf::from("/migrations/001_create_users.sql"),
            "CREATE TABLE users (id INTEGER);".to_string(),
        );
        assert_eq!(file.version(), 1);
        assert_eq!(file.description(), "create_users");
        assert_eq!(file.file_path(), &PathBuf::from("/migrations/001_create_users.sql"));
        assert_eq!(file.content(), "CREATE TABLE users (id INTEGER);");
    }

    // =====================================================================
    // MigrationFileParser
    // =====================================================================

    #[test]
    fn test_migration_file_parser_valid_with_up_down() {
        let content =
            "-- Migration: create users table\n-- UP:\nCREATE TABLE users (id INTEGER);\n-- DOWN:\nDROP TABLE users;\n";
        let result = MigrationFileParser::parse_migration_file(content);
        assert!(result.is_ok());
        let (desc, _) = result.unwrap();
        assert_eq!(desc, "create users table");
    }

    #[test]
    fn test_migration_file_parser_valid_with_create() {
        let content = "CREATE TABLE users (id INTEGER);";
        let result = MigrationFileParser::parse_migration_file(content);
        assert!(result.is_ok());
        let (desc, _) = result.unwrap();
        // 无 "-- Migration:" 注释时返回默认描述
        assert_eq!(desc, "Migration");
    }

    #[test]
    fn test_migration_file_parser_invalid_no_sql() {
        let content = "-- just a comment\n-- nothing else\n";
        let result = MigrationFileParser::parse_migration_file(content);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("does not contain recognizable SQL statements"));
    }

    #[test]
    fn test_migration_file_parser_extract_description_with_marker() {
        let content = "-- Migration: add index on users\nCREATE INDEX idx_users_email ON users(email);";
        let result = MigrationFileParser::parse_migration_file(content);
        assert!(result.is_ok());
        let (desc, _) = result.unwrap();
        assert_eq!(desc, "add index on users");
    }

    #[test]
    fn test_migration_file_parser_extract_description_default() {
        let content = "CREATE TABLE t (id INTEGER);";
        let result = MigrationFileParser::parse_migration_file(content);
        assert!(result.is_ok());
        let (desc, _) = result.unwrap();
        assert_eq!(desc, "Migration");
    }

    #[test]
    fn test_migration_file_parser_validate_sql_with_alter() {
        let content = "ALTER TABLE users ADD COLUMN name TEXT;";
        let result = MigrationFileParser::parse_migration_file(content);
        assert!(result.is_ok());
    }

    #[test]
    fn test_migration_file_parser_validate_sql_with_drop() {
        let content = "DROP TABLE old_table;";
        let result = MigrationFileParser::parse_migration_file(content);
        assert!(result.is_ok());
    }

    // =====================================================================
    // MigrationExecutor - non-database methods (需要 sqlite 以构造执行器)
    // =====================================================================

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_migration_executor_get_all_versions_empty() {
        let executor = create_sqlite_executor().await;
        assert!(executor.get_all_versions().is_empty());
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_migration_executor_get_latest_migration_empty() {
        let executor = create_sqlite_executor().await;
        assert!(executor.get_latest_migration().is_none());
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_migration_executor_is_fully_migrated_empty() {
        let executor = create_sqlite_executor().await;
        // 0 applied == 0 total → fully migrated
        assert!(executor.is_fully_migrated(0));
        assert!(!executor.is_fully_migrated(1));
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_migration_executor_history_empty() {
        let executor = create_sqlite_executor().await;
        assert!(executor.history().applied_migrations.is_empty());
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_migration_executor_build_history_insert_sql_raw_sqlite() {
        let executor = create_sqlite_executor().await;
        let dt = time::OffsetDateTime::now_utc();
        #[allow(deprecated)]
        let sql = executor.build_history_insert_sql_raw(1, "test migration", dt, "/path/to/file.sql");
        assert!(sql.contains("INSERT INTO dbnexus_migrations"));
        assert!(sql.contains("1"));
        assert!(sql.contains("test migration"));
        assert!(sql.contains("/path/to/file.sql"));
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_migration_executor_build_history_insert_sql_raw_escapes_quotes() {
        let executor = create_sqlite_executor().await;
        let dt = time::OffsetDateTime::now_utc();
        #[allow(deprecated)]
        let sql = executor.build_history_insert_sql_raw(1, "it's a 'test'", dt, "/path/to/file.sql");
        // 单引号应被转义为 ''
        assert!(sql.contains("it''s a ''test''"));
    }

    // =====================================================================
    // MigrationExecutor - auto-migrate feature methods
    // =====================================================================

    #[cfg(feature = "auto-migrate")]
    #[test]
    fn test_parse_filename_valid() {
        let result = MigrationExecutor::parse_filename("001_create_users.sql");
        assert_eq!(result, Some((1, "create_users".to_string())));
    }

    #[cfg(feature = "auto-migrate")]
    #[test]
    fn test_parse_filename_multi_part() {
        let result = MigrationExecutor::parse_filename("002_add_index_to_users_table.sql");
        assert_eq!(result, Some((2, "add_index_to_users_table".to_string())));
    }

    #[cfg(feature = "auto-migrate")]
    #[test]
    fn test_parse_filename_invalid_version() {
        let result = MigrationExecutor::parse_filename("abc_create_users.sql");
        assert!(result.is_none());
    }

    #[cfg(feature = "auto-migrate")]
    #[test]
    fn test_parse_filename_no_underscore() {
        // "123.sql" split('_') = ["123.sql"]，parts[0]="123.sql" 无法 parse::<u32>()
        // 因为 "123.sql" 不是纯数字
        let result = MigrationExecutor::parse_filename("123.sql");
        assert!(result.is_none());
    }

    #[cfg(feature = "auto-migrate")]
    #[test]
    fn test_extract_up_sql_with_up_and_down() {
        let content = "-- UP:\nCREATE TABLE users (id INTEGER);\n-- DOWN:\nDROP TABLE users;\n";
        let result = MigrationExecutor::extract_up_sql(content);
        assert!(result.contains("CREATE TABLE users"));
        assert!(!result.contains("DROP TABLE"));
    }

    #[cfg(feature = "auto-migrate")]
    #[test]
    fn test_extract_up_sql_only_up() {
        let content = "-- UP:\nCREATE TABLE users (id INTEGER);\n";
        let result = MigrationExecutor::extract_up_sql(content);
        assert!(result.contains("CREATE TABLE users"));
    }

    #[cfg(feature = "auto-migrate")]
    #[test]
    fn test_extract_up_sql_no_markers() {
        let content = "CREATE TABLE users (id INTEGER);";
        let result = MigrationExecutor::extract_up_sql(content);
        // 无标记时返回整个内容
        assert!(result.contains("CREATE TABLE users"));
    }

    #[cfg(feature = "auto-migrate")]
    #[test]
    fn test_extract_up_sql_case_insensitive_markers() {
        let content = "-- up:\nCREATE TABLE t (id INTEGER);\n-- down:\nDROP TABLE t;\n";
        let result = MigrationExecutor::extract_up_sql(content);
        assert!(result.contains("CREATE TABLE t"));
        assert!(!result.contains("DROP TABLE"));
    }

    #[cfg(feature = "auto-migrate")]
    #[test]
    fn test_extract_up_sql_only_down() {
        let content = "-- DOWN:\nDROP TABLE users;\n";
        let result = MigrationExecutor::extract_up_sql(content);
        // 只有 DOWN 标记时，UP 部分为 DOWN 之前的内容（空）
        assert!(result.is_empty());
    }

    // =====================================================================
    // MigrationExecutor - scan_migrations (需要 auto-migrate)
    // =====================================================================

    #[cfg(all(feature = "auto-migrate", feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_scan_migrations_empty_dir() {
        let executor = create_sqlite_executor().await;
        let dir = tempfile::tempdir().unwrap();
        let result = executor.scan_migrations(dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[cfg(all(feature = "auto-migrate", feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_scan_migrations_nonexistent_dir() {
        let executor = create_sqlite_executor().await;
        let result = executor.scan_migrations(std::path::Path::new("/nonexistent/path"));
        // 不存在的目录返回空 Vec
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[cfg(all(feature = "auto-migrate", feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_scan_migrations_with_files() {
        let executor = create_sqlite_executor().await;
        let dir = tempfile::tempdir().unwrap();

        // 创建迁移文件
        std::fs::write(
            dir.path().join("002_add_column.sql"),
            "ALTER TABLE t ADD COLUMN c TEXT;",
        )
        .unwrap();
        std::fs::write(dir.path().join("001_create_table.sql"), "CREATE TABLE t (id INTEGER);").unwrap();
        // 非SQL文件应被忽略
        std::fs::write(dir.path().join("readme.txt"), "not a migration").unwrap();
        // 无效文件名应被忽略（无法解析版本号）
        std::fs::write(dir.path().join("invalid.sql"), "CREATE TABLE t (id INTEGER);").unwrap();

        let result = executor.scan_migrations(dir.path());
        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 2);
        // 按版本号排序
        assert_eq!(files[0].version(), 1);
        assert_eq!(files[0].description(), "create_table");
        assert_eq!(files[1].version(), 2);
        assert_eq!(files[1].description(), "add_column");
    }

    // =====================================================================
    // MigrationExecutor - 数据库测试 (需要 sqlite feature)
    // =====================================================================

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_ensure_migration_table_exists() {
        let mut executor = create_sqlite_executor().await;
        // 调用 load_history 会先 ensure_migration_table_exists
        let result = executor.load_history().await;
        assert!(result.is_ok());
        // 历史应为空
        assert!(executor.history().applied_migrations.is_empty());
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_apply_migration_creates_table() {
        let mut executor = create_sqlite_executor().await;
        // 先确保迁移历史表存在
        executor.load_history().await.unwrap();

        let mut migration = Migration::new(1, "create_users".into());
        migration.add_table_change(TableChange::CreateTable(Table {
            name: "users".into(),
            columns: vec![
                Column {
                    name: "id".into(),
                    column_type: ColumnType::Integer,
                    is_primary_key: true,
                    is_nullable: false,
                    has_default: false,
                    default_value: None,
                    is_auto_increment: false,
                    comment: None,
                },
                Column {
                    name: "name".into(),
                    column_type: ColumnType::String(Some(255)),
                    is_primary_key: false,
                    is_nullable: false,
                    has_default: false,
                    default_value: None,
                    is_auto_increment: false,
                    comment: None,
                },
            ],
            primary_key_columns: vec!["id".into()],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        }));

        let result = executor.apply_migration(&migration).await;
        assert!(result.is_ok(), "apply_migration failed: {:?}", result.err());

        // 验证历史记录
        assert_eq!(executor.get_all_versions(), vec![1]);
        assert!(executor.get_latest_migration().is_some());
        assert_eq!(executor.get_latest_migration().unwrap().version, 1);
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_apply_migration_multiple_versions() {
        let mut executor = create_sqlite_executor().await;
        executor.load_history().await.unwrap();

        // 应用第一个迁移
        let mut m1 = Migration::new(1, "create_table".into());
        m1.add_table_change(TableChange::CreateTable(Table {
            name: "t1".into(),
            columns: vec![Column {
                name: "id".into(),
                column_type: ColumnType::Integer,
                is_primary_key: true,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: false,
                comment: None,
            }],
            primary_key_columns: vec!["id".into()],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        }));
        executor.apply_migration(&m1).await.unwrap();

        // 应用第二个迁移
        let mut m2 = Migration::new(2, "add_column".into());
        m2.add_table_change(TableChange::AlterTable {
            table_name: "t1".into(),
            column_changes: vec![],
            added_columns: vec![Column {
                name: "name".into(),
                column_type: ColumnType::Text,
                is_primary_key: false,
                is_nullable: true,
                has_default: false,
                default_value: None,
                is_auto_increment: false,
                comment: None,
            }],
            removed_columns: vec![],
            added_indexes: vec![],
            removed_indexes: vec![],
            added_foreign_keys: vec![],
            removed_foreign_keys: vec![],
        });
        executor.apply_migration(&m2).await.unwrap();

        // 验证
        assert_eq!(executor.get_all_versions(), vec![1, 2]);
        assert_eq!(executor.get_latest_migration().unwrap().version, 2);
        assert!(executor.is_fully_migrated(2));
        assert!(!executor.is_fully_migrated(3));
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    #[tokio::test]
    async fn test_load_history_after_apply() {
        let mut executor = create_sqlite_executor().await;
        executor.load_history().await.unwrap();

        // 应用迁移
        let mut m = Migration::new(1, "test".into());
        m.add_table_change(TableChange::CreateTable(Table {
            name: "test_table".into(),
            columns: vec![Column {
                name: "id".into(),
                column_type: ColumnType::Integer,
                is_primary_key: true,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: false,
                comment: None,
            }],
            primary_key_columns: vec!["id".into()],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        }));
        executor.apply_migration(&m).await.unwrap();

        // 创建新的执行器（模拟重启），从数据库加载历史
        let connection = executor.connection.clone();
        let mut executor2 = MigrationExecutor::new(connection, DatabaseType::Sqlite);
        let result = executor2.load_history().await;
        assert!(result.is_ok());
        assert_eq!(executor2.get_all_versions(), vec![1]);
        let latest = executor2.get_latest_migration().unwrap();
        assert_eq!(latest.version, 1);
        assert_eq!(latest.description, "test");
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls", feature = "auto-migrate"))]
    #[tokio::test]
    async fn test_run_migrations_from_files() {
        let mut executor = create_sqlite_executor().await;
        let dir = tempfile::tempdir().unwrap();

        // 创建迁移文件
        let sql = "-- UP:\nCREATE TABLE test_table (id INTEGER PRIMARY KEY);\n";
        std::fs::write(dir.path().join("001_create_test_table.sql"), sql).unwrap();

        let result = executor.run_migrations(dir.path()).await;
        assert!(result.is_ok(), "run_migrations failed: {:?}", result.err());
        assert_eq!(result.unwrap(), 1); // 1 个迁移被应用

        // 再次运行应返回 0（已应用）
        let result = executor.run_migrations(dir.path()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls", feature = "auto-migrate"))]
    #[tokio::test]
    async fn test_run_migrations_empty_dir() {
        let mut executor = create_sqlite_executor().await;
        let dir = tempfile::tempdir().unwrap();

        let result = executor.run_migrations(dir.path()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls", feature = "auto-migrate"))]
    #[tokio::test]
    async fn test_get_pending_migrations() {
        let mut executor = create_sqlite_executor().await;
        executor.load_history().await.unwrap();

        // 先应用版本 1
        let mut m1 = Migration::new(1, "first".into());
        m1.add_table_change(TableChange::CreateTable(Table {
            name: "t1".into(),
            columns: vec![Column {
                name: "id".into(),
                column_type: ColumnType::Integer,
                is_primary_key: true,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: false,
                comment: None,
            }],
            primary_key_columns: vec!["id".into()],
            indexes: vec![],
            foreign_keys: vec![],
            comment: None,
        }));
        executor.apply_migration(&m1).await.unwrap();

        // 检查待应用迁移
        let all = vec![
            m1,
            Migration::new(2, "second".into()),
            Migration::new(3, "third".into()),
        ];
        let pending = executor.get_pending_migrations(&all).await;
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].version, 2);
        assert_eq!(pending[1].version, 3);
    }

    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls", feature = "auto-migrate"))]
    #[tokio::test]
    async fn test_apply_migration_file_public() {
        let mut executor = create_sqlite_executor().await;
        executor.load_history().await.unwrap();

        let file = MigrationFile::new(
            1,
            "create_test".to_string(),
            PathBuf::from("/migrations/001_create_test.sql"),
            "CREATE TABLE test_table (id INTEGER PRIMARY KEY);".to_string(),
        );

        let result = executor.apply_migration_file_public(&file).await;
        assert!(result.is_ok(), "apply_migration_file_public failed: {:?}", result.err());
        assert_eq!(executor.get_all_versions(), vec![1]);
    }

    // =====================================================================
    // 辅助函数
    // =====================================================================

    /// 创建基于 SQLite 内存数据库的 MigrationExecutor
    #[cfg(all(feature = "sqlite", feature = "runtime-tokio-rustls"))]
    async fn create_sqlite_executor() -> MigrationExecutor {
        let connection = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        MigrationExecutor::new(connection, DatabaseType::Sqlite)
    }
}
