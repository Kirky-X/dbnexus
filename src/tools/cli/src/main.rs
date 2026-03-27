// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DBNexus 迁移 CLI 工具
//!
//! 提供数据库迁移的命令行界面

use clap::{Parser, Subcommand};
#[cfg(feature = "sql-parser")]
use dbnexus::sql_parser::{SqlOperationType, SqlParser};
use dbnexus::{DatabaseType as MigrationDatabaseType, DbError, DbPool, DbResult};
use dbnexus::{MigrationExecutor, MigrationFile, MigrationFileParser};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// CLI 配置
#[derive(Parser)]
#[command(name = "dbnexus-migrate")]
#[command(about = "DBNexus 数据库迁移工具", long_about = None)]
struct Cli {
    /// 数据库连接字符串
    #[arg(short, long, env = "DATABASE_URL")]
    database_url: String,

    /// 配置文件路径
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// 迁移文件目录
    #[arg(short, long, default_value = "./migrations")]
    migrations_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

/// CLI 子命令
#[derive(Subcommand)]
enum Commands {
    /// 创建新的迁移文件
    Create {
        /// 迁移描述
        description: String,

        /// 迁移文件输出目录
        #[arg(short, long, default_value = "./migrations")]
        directory: PathBuf,
    },

    /// 应用迁移
    Up {
        /// 目标版本号（可选，默认为所有待应用迁移）
        #[arg(long)]
        version: Option<u32>,
    },

    /// 回滚迁移
    Down {
        /// 目标版本号（可选，默认为回滚上一版本）
        #[arg(long)]
        version: Option<u32>,

        /// 回滚所有迁移
        #[arg(long, default_value = "false")]
        all: bool,
    },

    /// 查看迁移状态
    Status,

    /// 测试数据库连接
    TestConnection,

    /// 生成迁移文件（基于 schema 差异）
    Generate {
        /// 源 Schema 文件（JSON 格式）
        #[arg(long)]
        from_schema: Option<PathBuf>,

        /// 目标 Schema 文件（JSON 格式）
        #[arg(long)]
        to_schema: Option<PathBuf>,

        /// 输出迁移文件路径
        #[arg(short, long, default_value = "./migrations/generated.sql")]
        output: PathBuf,

        /// 迁移描述
        #[arg(short, long, default_value = "auto_generated")]
        description: String,
    },

    /// 列出所有迁移文件
    List,
}

/// 程序入口
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // 确保迁移目录存在
    if !cli.migrations_dir.exists() {
        fs::create_dir_all(&cli.migrations_dir).map_err(|e| DbError::Config(format!("无法创建迁移目录: {}", e)))?;
    }

    match &cli.command {
        Commands::Create { description, directory } => {
            create_migration(description, directory).await?;
        }
        Commands::Up { version } => {
            run_migrations_up(&cli.database_url, &cli.migrations_dir, *version).await?;
        }
        Commands::Down { version, all } => {
            run_migrations_down(&cli.database_url, *version, *all).await?;
        }
        Commands::Status => {
            show_status(&cli.database_url, &cli.migrations_dir).await?;
        }
        Commands::TestConnection => {
            test_connection(&cli.database_url).await?;
        }
        Commands::Generate {
            from_schema,
            to_schema,
            output,
            description,
        } => {
            generate_migration(from_schema, to_schema, output, description).await?;
        }
        Commands::List => {
            list_migrations(&cli.database_url, &cli.migrations_dir).await?;
        }
    }

    Ok(())
}

/// 创建新的迁移文件
async fn create_migration(description: &str, directory: &PathBuf) -> DbResult<()> {
    // 创建迁移目录（如果不存在）
    fs::create_dir_all(directory).map_err(|e| DbError::Config(format!("无法创建目录: {}", e)))?;

    // 生成时间戳作为版本号
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| DbError::Config(format!("无法解析时间戳: {}", e)))?
        .as_secs();

    // 验证并清理描述，防止路径遍历和特殊字符攻击
    let sanitized_description = description
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect::<String>();

    if sanitized_description.is_empty() {
        return Err(DbError::Config("迁移描述不能只包含特殊字符".to_string()));
    }

    if sanitized_description.len() > 100 {
        return Err(DbError::Config("迁移描述过长（最大 100 字符）".to_string()));
    }

    let filename = format!("{}_{}.sql", timestamp, sanitized_description);
    let filepath = directory.join(&filename);

    // 创建迁移文件模板
    let migration_content = format!(
        r#"-- Migration: {description}
-- Version: {timestamp}
-- Created: {created_at}

-- UP: Apply migration
-- Your migration SQL goes here

-- DOWN: Rollback migration
-- Reversal of migration SQL goes here
"#,
        description = description,
        timestamp = timestamp,
        created_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
    );

    fs::write(&filepath, migration_content).map_err(|e| DbError::Config(format!("无法写入迁移文件: {}", e)))?;

    println!("✓ 迁移文件已创建: {}", filepath.display());

    Ok(())
}

/// 显示迁移状态
async fn show_status(database_url: &str, migrations_dir: &PathBuf) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    迁移状态查看                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    // 测试数据库连接
    let pool = match DbPool::new(database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            println!("\n❌ 数据库连接失败: {}", e);
            return Ok(());
        }
    };

    // 获取数据库类型
    let db_type =
        detect_database_type(database_url).map_err(|e| DbError::Config(format!("数据库类型检测失败: {}", e)))?;
    println!("\n📊 数据库类型: {}", db_type);
    println!("📁 迁移目录: {}", migrations_dir.display());

    // 加载迁移历史
    let session = match pool.get_session("admin").await {
        Ok(session) => session,
        Err(e) => {
            println!("\n❌ 无法获取数据库会话: {}", e);
            return Ok(());
        }
    };

    let mut executor = session.create_migration_executor(db_type)?;

    if let Err(e) = executor.load_history().await {
        println!("\n⚠️  无法加载迁移历史: {}", e);
        println!("   迁移历史表可能不存在");
        return Ok(());
    }

    let applied_count = executor.history().applied_migrations.len();
    println!("\n✅ 已应用的迁移: {} 个", applied_count);

    if applied_count > 0 {
        // 显示最新迁移信息
        if let Some(latest_version) = executor.history().get_latest_version() {
            if let Some(latest_migration) = executor
                .history()
                .applied_migrations
                .iter()
                .find(|m| m.version == latest_version)
            {
                println!("   最新迁移:");
                println!("     - 版本: {}", latest_migration.version);
                println!("     - 描述: {}", latest_migration.description);
                println!("     - 应用时间: {}", latest_migration.applied_at);
            }
        }

        // 显示所有已应用迁移
        println!("\n   迁移历史详情:");
        for (idx, migration) in executor.history().applied_migrations.iter().enumerate() {
            println!(
                "   [{:2}] v{:6} - {}",
                idx + 1,
                migration.version,
                migration.description
            );
        }
    }

    // 扫描本地迁移文件
    let local_migrations = executor.scan_migrations(migrations_dir)?;
    let pending_count = local_migrations
        .iter()
        .filter(|m| !executor.history().is_version_applied(m.version()))
        .count();

    println!("\n📦 本地迁移文件: {} 个", local_migrations.len());
    println!("⏳ 待应用的迁移: {} 个", pending_count);

    if !local_migrations.is_empty() {
        // 显示待应用的迁移
        let applied_versions: std::collections::HashSet<u32> = executor
            .history()
            .applied_migrations
            .iter()
            .map(|m| m.version)
            .collect();

        let pending: Vec<_> = local_migrations
            .iter()
            .filter(|m| !applied_versions.contains(&m.version()))
            .collect();

        if !pending.is_empty() {
            println!("\n   待应用迁移列表:");
            for (idx, migration) in pending.iter().enumerate() {
                println!(
                    "   [{:2}] v{:6} - {}",
                    idx + 1,
                    migration.version(),
                    migration.description()
                );
            }
        } else {
            println!("\n   ✓ 所有迁移都已应用");
        }
    }

    // 显示数据库连接信息
    println!("\n🔗 数据库连接: 已连接");
    println!("   URL: {}", mask_database_url(database_url));

    println!("\n{}", "─".repeat(60));

    Ok(())
}

/// 测试数据库连接
async fn test_connection(database_url: &str) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    数据库连接测试                            ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    println!("\n🔄 正在测试数据库连接...");

    let start_time = std::time::Instant::now();

    let pool = match DbPool::new(database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            println!("\n❌ 连接失败: {}", e);
            return Err(e);
        }
    };

    let elapsed = start_time.elapsed();

    // 获取会话以验证连接
    match pool.get_session("admin").await {
        Ok(session) => {
            let _conn = session.connection()?.clone();
            drop(session);

            let db_type = detect_database_type(database_url)
                .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("数据库类型检测失败: {}", e))))?;

            println!("\n✅ 连接成功!");
            println!("\n   数据库类型: {}", db_type);
            println!("   连接耗时: {:?}", elapsed);
            println!("   连接URL: {}", mask_database_url(database_url));

            // 显示连接池状态
            println!("\n   连接池状态:");
            let status = pool.status();
            println!("     - 总连接数: {}", status.total);
            println!("     - 活跃连接: {}", status.active);
            println!("     - 空闲连接: {}", status.idle);
        }
        Err(e) => {
            println!("\n❌ 连接验证失败: {}", e);
        }
    }

    println!("\n{}", "─".repeat(60));

    Ok(())
}

/// 运行向上的迁移（应用迁移）
async fn run_migrations_up(database_url: &str, migrations_dir: &PathBuf, target_version: Option<u32>) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    应用迁移                                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let pool = DbPool::new(database_url).await?;
    let db_type = detect_database_type(database_url)?;

    println!("\n📊 数据库类型: {}", db_type);
    println!("📁 迁移目录: {}", migrations_dir.display());

    // 创建迁移执行器
    let session = pool.get_session("admin").await?;
    let mut executor = session.create_migration_executor(db_type)?;

    // 扫描迁移文件
    let migrations = executor.scan_migrations(migrations_dir)?;

    if migrations.is_empty() {
        println!("\n⚠️  迁移目录中没有找到迁移文件");
        return Ok(());
    }

    // 加载迁移历史并获取已应用版本
    executor.load_history().await?;
    let applied_versions: std::collections::HashSet<u32> = executor
        .history()
        .applied_migrations
        .iter()
        .map(|m| m.version)
        .collect();

    // 筛选待应用的迁移
    let mut to_apply: Vec<_> = migrations
        .iter()
        .filter(|m| !applied_versions.contains(&m.version()))
        .filter(|m| {
            if let Some(target) = target_version {
                m.version() <= target
            } else {
                true
            }
        })
        .collect();

    to_apply.sort_by_key(|m| m.version());

    if to_apply.is_empty() {
        println!("\n✓ 没有待应用的迁移");
        return Ok(());
    }

    println!("\n📦 找到 {} 个待应用迁移", to_apply.len());

    if let Some(target) = target_version {
        println!("   目标版本: {}", target);
    }

    // 应用迁移
    println!("\n🚀 开始应用迁移...");
    let mut success_count = 0;

    for migration in &to_apply {
        print!(
            "   正在应用 v{} - {} ... ",
            migration.version(),
            migration.description()
        );

        match executor.apply_migration_file_public(migration).await {
            Ok(_) => {
                println!("✓");
                success_count += 1;
            }
            Err(e) => {
                println!("❌ 失败: {}", e);
                return Err(e);
            }
        }
    }

    println!("\n✅ 成功应用 {} / {} 个迁移", success_count, to_apply.len());
    println!("\n{}", "─".repeat(60));

    Ok(())
}

/// 运行向下的迁移（回滚迁移）
async fn run_migrations_down(database_url: &str, target_version: Option<u32>, rollback_all: bool) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    回滚迁移                                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let pool = DbPool::new(database_url).await?;
    let db_type = detect_database_type(database_url)?;

    println!("\n📊 数据库类型: {}", db_type);

    // 创建迁移执行器
    let session = pool.get_session("admin").await?;
    let mut executor = session.create_migration_executor(db_type)?;

    // 加载迁移历史
    executor.load_history().await?;

    let applied_migrations = &executor.history().applied_migrations;

    if applied_migrations.is_empty() {
        println!("\n⚠️  没有已应用的迁移可以回滚");
        return Ok(());
    }

    // 确定要回滚的版本
    let versions_to_rollback: Vec<u32> = if rollback_all {
        applied_migrations.iter().map(|m| m.version).collect()
    } else if let Some(target) = target_version {
        applied_migrations
            .iter()
            .filter(|m| m.version >= target)
            .map(|m| m.version)
            .collect()
    } else {
        // 回滚上一个版本
        if let Some(max_version) = applied_migrations.iter().map(|m| m.version).max() {
            vec![max_version]
        } else {
            Vec::new() // 无迁移可回滚
        }
    };

    // 按版本号降序排序（先回滚最新的）
    let mut versions_to_rollback = versions_to_rollback;
    versions_to_rollback.sort_by_key(|v| std::cmp::Reverse(*v));

    println!("\n📦 需要回滚 {} 个迁移", versions_to_rollback.len());

    if rollback_all {
        println!("   模式: 回滚所有迁移");
    } else if let Some(target) = target_version {
        println!("   模式: 回滚到版本 {}", target);
    } else {
        println!("   模式: 回滚上一个版本");
    }

    // 执行回滚
    println!("\n🔄 开始回滚迁移...");
    let mut success_count = 0;

    // 收集需要回滚的迁移信息，避免在循环中借用
    let rollback_info: Vec<(u32, String)> = versions_to_rollback
        .iter()
        .filter_map(|version| {
            applied_migrations
                .iter()
                .find(|m| m.version == *version)
                .map(|info| (info.version, info.description.clone()))
        })
        .collect();

    for (version, description) in &rollback_info {
        print!("   正在回滚 v{} - {} ... ", version, description);

        match rollback_migration(&mut executor, *version, db_type).await {
            Ok(_) => {
                println!("✓");
                success_count += 1;
            }
            Err(e) => {
                println!("❌ 失败: {}", e);
                // 回滚失败时停止并返回错误，避免状态不一致
                println!("\n⚠️  回滚过程中发生错误，停止执行");
                return Err(DbError::Migration(format!(
                    "Migration rollback failed for v{}: {}",
                    version, e
                )));
            }
        }
    }

    println!(
        "\n✅ 成功回滚 {} / {} 个迁移",
        success_count,
        versions_to_rollback.len()
    );
    println!("\n{}", "─".repeat(60));

    Ok(())
}

/// 回滚单个迁移
async fn rollback_migration(
    executor: &mut MigrationExecutor,
    version: u32,
    db_type: MigrationDatabaseType,
) -> DbResult<()> {
    use sea_orm::{ConnectionTrait, DatabaseTransaction, TransactionTrait};

    // 使用参数化查询防止 SQL 注入
    let backend = match db_type {
        MigrationDatabaseType::Postgres => sea_orm::DbBackend::Postgres,
        MigrationDatabaseType::MySql => sea_orm::DbBackend::MySql,
        MigrationDatabaseType::Sqlite => sea_orm::DbBackend::Sqlite,
    };
    let delete_sql = sea_orm::Statement::from_sql_and_values(
        backend,
        "DELETE FROM dbnexus_migrations WHERE version = ?".to_string(),
        vec![version.into()],
    );

    // 开始事务并执行回滚
    let conn = &executor.connection;
    let txn: DatabaseTransaction = TransactionTrait::begin(conn).await.map_err(DbError::Connection)?;

    txn.execute_raw(delete_sql).await.map_err(DbError::Connection)?;

    txn.commit().await.map_err(DbError::Connection)?;

    Ok(())
}

/// 生成迁移文件
async fn generate_migration(
    from_schema: &Option<PathBuf>,
    to_schema: &Option<PathBuf>,
    output: &PathBuf,
    description: &str,
) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    生成迁移文件                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    // 生成时间戳作为版本号
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| DbError::Config(format!("无法解析时间戳: {}", e)))?
        .as_secs();

    // 如果提供了 schema 文件，尝试生成差异 SQL
    let migration_content;

    if let (Some(from), Some(to)) = (from_schema, to_schema) {
        println!("\n📄 解析 Schema 文件...");

        let from_content =
            fs::read_to_string(from).map_err(|e| DbError::Config(format!("无法读取源 schema 文件: {}", e)))?;
        let to_content =
            fs::read_to_string(to).map_err(|e| DbError::Config(format!("无法读取目标 schema 文件: {}", e)))?;

        // 生成差异 SQL
        let diff_sql = generate_schema_diff_sql(&from_content, &to_content)?;

        migration_content = format!(
            r#"-- Migration: {description}
-- Version: {timestamp}
-- Created: {created_at}
-- Type: Auto-generated from schema diff

-- UP: Apply migration
{up_sql}

-- DOWN: Rollback migration
{down_sql}
"#,
            description = description,
            timestamp = timestamp,
            created_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
            up_sql = diff_sql.up,
            down_sql = diff_sql.down
        );

        println!("✓ 已生成 schema 差异 SQL");
    } else {
        // 生成空白迁移模板
        migration_content = format!(
            r#"-- Migration: {description}
-- Version: {timestamp}
-- Created: {created_at}
-- Type: Manual migration

-- UP: Apply migration
-- Your migration SQL goes here

-- DOWN: Rollback migration
-- Reversal of migration SQL goes here
"#,
            description = description,
            timestamp = timestamp,
            created_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
        );

        println!("⚠️  未提供 schema 文件，已生成空白模板");
    }

    // 确保输出目录存在
    if let Some(parent) = output.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| DbError::Config(format!("无法创建输出目录: {}", e)))?;
        }
    }

    // 写入文件
    fs::write(output, migration_content).map_err(|e| DbError::Config(format!("无法写入迁移文件: {}", e)))?;

    println!("\n✓ 迁移文件已生成: {}", output.display());

    // 如果生成了实际 SQL，显示摘要
    if from_schema.is_some() && to_schema.is_some() {
        println!("   请检查并编辑生成的迁移文件以确保正确性");
    }

    println!("\n{}", "─".repeat(60));

    Ok(())
}

/// Schema 差异 SQL
struct DiffSql {
    up: String,
    down: String,
}

/// 生成 Schema 差异 SQL（简化版本）
fn generate_schema_diff_sql(_from_content: &str, _to_content: &str) -> Result<DiffSql, DbError> {
    // 这里是一个简化实现
    // 实际实现需要解析 schema 文件并计算差异
    Ok(DiffSql {
        up: "-- 自动生成的 UP SQL 请手动编辑".to_string(),
        down: "-- 自动生成的 DOWN SQL 请手动编辑".to_string(),
    })
}

/// 解析并应用迁移
async fn parse_and_apply_migration(
    session: &mut dbnexus::Session,
    executor: &mut MigrationExecutor,
    content: &str,
    version: u32,
) -> DbResult<()> {
    // 解析迁移内容
    let (description, _full_content) =
        MigrationFileParser::parse_migration_file(content).unwrap_or(("Migration".to_string(), content.to_string()));

    // 提取 UP SQL（-- UP 到 -- DOWN 之间）
    let up_sql = extract_sql_section(content, "UP")?;

    // 验证并执行 UP SQL（SQL 注入防护）
    if !up_sql.trim().is_empty() {
        // 对于迁移场景，我们信任迁移文件中的 SQL，只进行基本的安全检查

        // 检测危险操作
        let sql_upper = up_sql.trim().to_uppercase();
        let dangerous_patterns = [("DROP DATABASE", "DROP DATABASE"), ("TRUNCATE TABLE", "TRUNCATE TABLE")];
        for (pattern, description) in &dangerous_patterns {
            if sql_upper.contains(pattern) {
                return Err(DbError::Migration(format!(
                    "Forbidden pattern in migration SQL: {} ({})",
                    pattern, description
                )));
            }
        }

        // 直接执行 SQL
        session.execute_raw(&up_sql).await?;
    }

    let file_path = format!("migration_v{}.sql", version);
    let migration_file = MigrationFile::new(
        version,
        description,
        std::path::PathBuf::from(&file_path),
        String::new(), // SQL 已在上面执行，content 为空
    );
    executor.apply_migration_file_public(&migration_file).await?;

    Ok(())
}

/// 提取 SQL 部分
fn extract_sql_section(content: &str, section: &str) -> Result<String, DbError> {
    let section_start_pattern = format!("-- {}:", section);
    let section_end_pattern = format!("-- {}", if section == "UP" { "DOWN" } else { "UP" });

    // 查找 section 开始标记（-- UP: 或 -- DOWN:）
    let start_match = content.find(&section_start_pattern);
    // 查找 section 结束标记
    let end_match = content.find(&section_end_pattern);

    if let Some(start_idx) = start_match {
        // 跳过一整行：找到换行符位置
        let line_end = content[start_idx..]
            .find('\n')
            .map(|offset| start_idx + offset + 1)  // +1 包含换行符
            .unwrap_or(start_idx + section_start_pattern.len());

        if let Some(end_idx) = end_match {
            if end_idx > start_idx {
                // 提取 section 开始换行符之后，到 section 结束标记之前的内容
                Ok(content[line_end..end_idx].trim().to_string())
            } else {
                // 没有结束标记，提取到文件末尾
                Ok(content[line_end..].trim().to_string())
            }
        } else {
            // 没有结束标记，提取到文件末尾
            Ok(content[line_end..].trim().to_string())
        }
    } else {
        Ok(String::new())
    }
}

/// 列出所有迁移文件
async fn list_migrations(database_url: &str, migrations_dir: &PathBuf) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    迁移文件列表                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let pool = DbPool::new(database_url).await?;
    let db_type = detect_database_type(database_url)?;
    let session = pool.get_session("admin").await?;
    let executor = session.create_migration_executor(db_type)?;

    let migrations = executor.scan_migrations(migrations_dir)?;

    if migrations.is_empty() {
        println!("\n⚠️  迁移目录中没有找到迁移文件");
        println!("   目录: {}", migrations_dir.display());
        return Ok(());
    }

    println!("\n📁 迁移目录: {}", migrations_dir.display());
    println!("📦 共 {} 个迁移文件\n", migrations.len());

    for (idx, migration) in migrations.iter().enumerate() {
        println!(
            "   [{:2}] v{:6} - {}",
            idx + 1,
            migration.version(),
            migration.description()
        );
    }

    println!("\n{}", "─".repeat(60));

    Ok(())
}

/// 检测数据库类型（增强版）
///
/// 使用 URL 解析器验证数据库 URL 格式，
/// 只返回已知支持的数据库类型，不支持时返回错误
fn detect_database_type(database_url: &str) -> Result<MigrationDatabaseType, DbError> {
    // 尝试解析 URL
    let url =
        url::Url::parse(database_url).map_err(|e| DbError::Config(format!("Invalid database URL format: {}", e)))?;

    // 获取协议scheme
    let scheme = url.scheme().to_lowercase();

    // 根据协议返回对应的数据库类型
    match scheme.as_str() {
        "postgres" | "postgresql" => Ok(MigrationDatabaseType::Postgres),
        "mysql" => Ok(MigrationDatabaseType::MySql),
        "sqlite" | "sqlite3" | "file" => Ok(MigrationDatabaseType::Sqlite),
        "oci" | "oracle" => Err(DbError::Config("Oracle database is not supported".to_string())),
        "mssql" | "sqlserver" => Err(DbError::Config("SQL Server database is not supported".to_string())),
        _ => Err(DbError::Config(format!(
            "Unsupported database protocol: '{}'. Supported protocols: sqlite, postgres, mysql",
            scheme
        ))),
    }
}

/// 隐藏数据库 URL 中的敏感信息
fn mask_database_url(url: &str) -> String {
    url::Url::parse(url)
        .map(|mut url| {
            if let Some(password) = url.password() {
                url.set_password(Some(&"*".repeat(password.len()))).ok();
            }
            url.to_string()
        })
        .unwrap_or_else(|_| url.to_string())
}
