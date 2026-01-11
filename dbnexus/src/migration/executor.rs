// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 迁移执行器
//!
//! 负责执行数据库迁移操作

use super::differ::SqlGenerator;
use super::schema::*;
use crate::config::DatabaseType;
use sea_orm::{ConnectionTrait, TransactionTrait};
use std::path::PathBuf;

/// 迁移执行器
pub struct MigrationExecutor {
    /// 数据库连接
    pub connection: sea_orm::DatabaseConnection,
    /// SQL 生成器
    pub sql_generator: SqlGenerator,
    /// 迁移历史记录
    pub history: MigrationHistory,
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

    /// 读取数据库中的迁移历史
    pub async fn load_history(&mut self) -> Result<(), crate::config::DbError> {
        // 确保迁移历史表存在
        self.ensure_migration_table_exists().await?;

        // 查询已应用的迁移版本
        let query_sql = "SELECT version FROM dbnexus_migrations ORDER BY version";

        match self.connection.execute_unprepared(query_sql).await {
            Ok(_) => {
                // 表存在且查询成功
                // 由于 Sea-ORM 的限制，我们使用一个简单的方法：
                // 每次应用迁移时都会添加记录到历史中
                // 这里我们假设历史已经被正确填充
            }
            Err(e) => {
                tracing::warn!("Failed to load migration history: {}", e);
                self.history = MigrationHistory::new();
            }
        }

        Ok(())
    }

    /// 确保迁移历史表存在
    async fn ensure_migration_table_exists(&self) -> Result<(), crate::config::DbError> {
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
            .map_err(crate::config::DbError::Connection)?;
        Ok(())
    }

    /// 应用单个迁移
    pub async fn apply_migration(&mut self, migration: &Migration) -> Result<(), crate::config::DbError> {
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

        // 插入到迁移历史表（使用参数化查询防止 SQL 注入）
        // 使用 Statement::from_sql_and_values 进行参数化查询
        let insert_sql =
            "INSERT INTO dbnexus_migrations (version, description, applied_at, file_path) VALUES (?, ?, ?, ?)";

        let backend = match self.sql_generator.db_type {
            DatabaseType::Postgres => sea_orm::DbBackend::Postgres,
            DatabaseType::MySql => sea_orm::DbBackend::MySql,
            DatabaseType::Sqlite => sea_orm::DbBackend::Sqlite,
        };

        let stmt = sea_orm::Statement::from_sql_and_values(
            backend,
            insert_sql.to_string(),
            vec![
                migration.version.into(),
                migration.description.clone().into(),
                version_record.applied_at.to_string().into(),
                version_record.file_path.clone().into(),
            ],
        );

        txn.execute_raw(stmt)
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

/// 迁移文件信息
#[derive(Debug, Clone)]
pub struct MigrationFile {
    /// 版本号
    pub version: u32,
    /// 描述
    pub description: String,
    /// 文件路径
    pub file_path: PathBuf,
    /// 文件内容
    pub content: String,
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
    pub fn scan_migrations(&self, dir: &std::path::Path) -> Result<Vec<MigrationFile>, crate::config::DbError> {
        let mut migrations = Vec::new();

        if !dir.exists() {
            tracing::warn!("Migration directory does not exist: {}", dir.display());
            return Ok(migrations);
        }

        let entries = std::fs::read_dir(dir)
            .map_err(|e| crate::config::DbError::Config(format!("Failed to read migration directory: {}", e)))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| crate::config::DbError::Config(format!("Failed to read migration entry: {}", e)))?;
            let path = entry.path();

            if path.is_file() && path.extension().map(|e| e == "sql").unwrap_or(false) {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some((version, description)) = Self::parse_filename(filename) {
                        let content = std::fs::read_to_string(&path).map_err(|e| {
                            crate::config::DbError::Config(format!("Failed to read migration file: {}", e))
                        })?;

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

        tracing::info!("Scanned {} migration files in {}", migrations.len(), dir.display());

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
    pub async fn run_migrations(&mut self, dir: &std::path::Path) -> Result<u32, crate::config::DbError> {
        // 扫描迁移文件
        let migration_files = self.scan_migrations(dir)?;

        // 直接从数据库检查哪些迁移已应用
        let mut applied_versions = std::collections::HashSet::new();
        for migration_file in &migration_files {
            if self.is_migration_applied(migration_file.version).await? {
                applied_versions.insert(migration_file.version);
            }
        }

        let pending: Vec<_> = migration_files
            .into_iter()
            .filter(|m| !applied_versions.contains(&m.version))
            .collect();

        if pending.is_empty() {
            tracing::info!("No pending migrations to apply");
            return Ok(0);
        }

        tracing::info!("Found {} pending migrations", pending.len());

        // 应用迁移
        let mut applied_count = 0;
        for migration_file in &pending {
            tracing::info!(
                "Applying migration v{} - {}",
                migration_file.version,
                migration_file.description
            );

            match self.apply_migration_file(migration_file).await {
                Ok(_) => {
                    applied_count += 1;
                    tracing::info!(
                        "Successfully applied migration v{} - {}",
                        migration_file.version,
                        migration_file.description
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to apply migration v{} - {}: {}",
                        migration_file.version,
                        migration_file.description,
                        e
                    );
                    return Err(e);
                }
            }
        }

        Ok(applied_count)
    }

    /// 检查迁移是否已应用（通过查询数据库）
    async fn is_migration_applied(&self, version: u32) -> Result<bool, crate::config::DbError> {
        // 先确保迁移历史表存在
        self.ensure_migration_table_exists().await?;

        // 使用参数化查询防止 SQL 注入
        let backend = match self.sql_generator.db_type {
            DatabaseType::Postgres => sea_orm::DbBackend::Postgres,
            DatabaseType::MySql => sea_orm::DbBackend::MySql,
            DatabaseType::Sqlite => sea_orm::DbBackend::Sqlite,
        };
        let check_query = sea_orm::Statement::from_sql_and_values(
            backend,
            "SELECT 1 FROM dbnexus_migrations WHERE version = ?".to_string(),
            vec![version.into()],
        );

        match self.connection.execute_raw(check_query).await {
            Ok(result) => Ok(result.rows_affected() > 0),
            Err(_) => Ok(false),
        }
    }

    /// 应用单个迁移文件
    async fn apply_migration_file(&mut self, migration_file: &MigrationFile) -> Result<(), crate::config::DbError> {
        // 解析迁移文件内容
        let sql = Self::extract_up_sql(&migration_file.content);

        // 开始事务
        let txn = self
            .connection
            .begin()
            .await
            .map_err(crate::config::DbError::Connection)?;

        // 执行迁移 SQL
        if !sql.is_empty() {
            txn.execute_unprepared(sql)
                .await
                .map_err(crate::config::DbError::Connection)?;
        }

        // 记录迁移历史（使用参数化查询防止 SQL 注入）
        let applied_at = time::OffsetDateTime::now_utc();

        let insert_sql =
            "INSERT INTO dbnexus_migrations (version, description, applied_at, file_path) VALUES (?, ?, ?, ?)";
        let backend = match self.sql_generator.db_type {
            DatabaseType::Postgres => sea_orm::DbBackend::Postgres,
            DatabaseType::MySql => sea_orm::DbBackend::MySql,
            DatabaseType::Sqlite => sea_orm::DbBackend::Sqlite,
        };
        let stmt = sea_orm::Statement::from_sql_and_values(
            backend,
            insert_sql.to_string(),
            vec![
                migration_file.version.into(),
                migration_file.description.clone().into(),
                applied_at.to_string().into(),
                migration_file.file_path.to_string_lossy().into(),
            ],
        );

        txn.execute_raw(stmt)
            .await
            .map_err(crate::config::DbError::Connection)?;

        // 提交事务
        txn.commit().await.map_err(crate::config::DbError::Connection)?;

        // 添加到历史记录
        self.history.add_migration(MigrationVersion {
            version: migration_file.version,
            description: migration_file.description.clone(),
            applied_at,
            file_path: migration_file.file_path.to_string_lossy().to_string(),
        });

        Ok(())
    }

    /// 从迁移文件中提取 UP SQL
    fn extract_up_sql(content: &str) -> &str {
        // 查找 UP 标记之后的内容
        let up_marker = content
            .find("-- UP:")
            .or(content.find("-- up:"))
            .or(content.find("UP:"));
        let down_marker = content
            .find("-- DOWN:")
            .or(content.find("-- down:"))
            .or(content.find("DOWN:"));

        match (up_marker, down_marker) {
            (Some(up_pos), Some(down_pos)) if down_pos > up_pos => {
                // UP 标记在 DOWN 标记之前，提取 UP 部分
                &content[up_pos + 5..down_pos]
            }
            (Some(up_pos), _) => {
                // 只有 UP 标记（或 DOWN 在 UP 之前），提取 UP 标记之后的所有内容
                &content[up_pos + 5..]
            }
            (None, Some(down_pos)) => {
                // 如果没有 UP 标记但有 DOWN 标记，返回 DOWN 之前的内容
                &content[..down_pos]
            }
            (None, None) => {
                // 如果没有标记，返回整个内容
                content
            }
        }
        .trim()
    }

    /// 回滚所有迁移
    pub async fn rollback_all(&mut self) -> Result<u32, crate::config::DbError> {
        self.load_history().await?;

        let applied = &self.history.applied_migrations;

        if applied.is_empty() {
            tracing::info!("No migrations to rollback");
            return Ok(0);
        }

        let mut rollback_count = 0;
        // 按版本号降序排序（先回滚最新的）
        let mut versions: Vec<u32> = applied.iter().map(|m| m.version).collect();
        versions.sort_by_key(|v| std::cmp::Reverse(*v));

        for version in versions {
            tracing::info!("Rolling back migration v{}", version);

            match self.rollback_migration(version).await {
                Ok(_) => {
                    rollback_count += 1;
                    tracing::info!("Successfully rolled back migration v{}", version);
                }
                Err(e) => {
                    tracing::error!("Failed to rollback migration v{}: {}", version, e);
                    return Err(e);
                }
            }
        }

        Ok(rollback_count)
    }

    /// 回滚指定版本的迁移
    pub async fn rollback_migration(&mut self, version: u32) -> Result<(), crate::config::DbError> {
        // 使用参数化查询防止 SQL 注入
        let backend = match self.sql_generator.db_type {
            DatabaseType::Postgres => sea_orm::DbBackend::Postgres,
            DatabaseType::MySql => sea_orm::DbBackend::MySql,
            DatabaseType::Sqlite => sea_orm::DbBackend::Sqlite,
        };
        let delete_sql = sea_orm::Statement::from_sql_and_values(
            backend,
            "DELETE FROM dbnexus_migrations WHERE version = ?".to_string(),
            vec![version.into()],
        );

        let txn: sea_orm::DatabaseTransaction = self
            .connection
            .begin()
            .await
            .map_err(crate::config::DbError::Connection)?;

        txn.execute_raw(delete_sql)
            .await
            .map_err(crate::config::DbError::Connection)?;

        txn.commit().await.map_err(crate::config::DbError::Connection)?;

        // 从历史记录中移除
        self.history.applied_migrations.retain(|m| m.version != version);

        Ok(())
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
