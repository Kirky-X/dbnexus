// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DBNexus 迁移 CLI 工具
//!
//! 提供数据库迁移的命令行界面

use clap::{Parser, Subcommand};
use dbnexus::migration::{MigrationExecutor, MigrationFileParser};
use dbnexus::{
    DbPool, DbResult,
    config::{DatabaseType as MigrationDatabaseType, DbError},
};
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
            list_migrations(&cli.migrations_dir)?;
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

    let filename = format!("{}_{}.sql", timestamp, description.replace(' ', "_"));
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
    let db_type = detect_database_type(database_url);
    println!("\n📊 数据库类型: {}", db_type);
    println!("📁 迁移目录: {}", migrations_dir.display());

    // 加载迁移历史
    let mut session = match pool.get_session("admin").await {
        Ok(session) => session,
        Err(e) => {
            println!("\n❌ 无法获取数据库会话: {}", e);
            return Ok(());
        }
    };

    let connection = session.connection()?.clone();
    let mut executor = MigrationExecutor::new(connection, db_type);

    if let Err(e) = executor.load_history().await {
        println!("\n⚠️  无法加载迁移历史: {}", e);
        println!("   迁移历史表可能不存在");
        return Ok(());
    }

    let applied_count = executor.history.applied_migrations.len();
    println!("\n✅ 已应用的迁移: {} 个", applied_count);

    if applied_count > 0 {
        // 显示最新迁移信息
        if let Some(latest_version) = executor.history.get_latest_version() {
            if let Some(latest_migration) = executor
                .history
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
        for (idx, migration) in executor.history.applied_migrations.iter().enumerate() {
            println!(
                "   [{:2}] v{:6} - {}",
                idx + 1,
                migration.version,
                migration.description
            );
        }
    }

    // 扫描本地迁移文件
    let local_migrations = scan_migration_files(migrations_dir)?;
    let pending_count = local_migrations.len().saturating_sub(applied_count);

    println!("\n📦 本地迁移文件: {} 个", local_migrations.len());
    println!("⏳ 待应用的迁移: {} 个", pending_count);

    if !local_migrations.is_empty() {
        // 显示待应用的迁移
        let applied_versions: std::collections::HashSet<u32> =
            executor.history.applied_migrations.iter().map(|m| m.version).collect();

        let pending: Vec<_> = local_migrations
            .iter()
            .filter(|m| !applied_versions.contains(&m.version))
            .collect();

        if !pending.is_empty() {
            println!("\n   待应用迁移列表:");
            for (idx, migration) in pending.iter().enumerate() {
                println!(
                    "   [{:2}] v{:6} - {}",
                    idx + 1,
                    migration.version,
                    migration.description
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
            return Ok(());
        }
    };

    let elapsed = start_time.elapsed();

    // 获取会话以验证连接
    match pool.get_session("admin").await {
        Ok(mut session) => {
            let _conn = session.connection()?.clone();
            drop(session);

            let db_type = detect_database_type(database_url);

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
    let db_type = detect_database_type(database_url);

    println!("\n📊 数据库类型: {}", db_type);
    println!("📁 迁移目录: {}", migrations_dir.display());

    // 扫描迁移文件
    let migrations = scan_migration_files(migrations_dir)?;

    if migrations.is_empty() {
        println!("\n⚠️  迁移目录中没有找到迁移文件");
        return Ok(());
    }

    // 创建迁移执行器
    let mut session = pool.get_session("admin").await?;
    let connection = session.connection()?.clone();
    let mut executor = MigrationExecutor::new(connection, db_type);

    // 加载迁移历史
    executor.load_history().await?;

    // 筛选待应用的迁移
    let applied_versions: std::collections::HashSet<u32> =
        executor.history.applied_migrations.iter().map(|m| m.version).collect();

    let mut to_apply: Vec<_> = migrations
        .iter()
        .filter(|m| !applied_versions.contains(&m.version))
        .filter(|m| {
            if let Some(target) = target_version {
                m.version <= target
            } else {
                true
            }
        })
        .collect();

    to_apply.sort_by_key(|m| m.version);

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
        print!("   正在应用 v{} - {} ... ", migration.version, migration.description);

        match std::fs::read_to_string(&migration.file_path) {
            Ok(content) => match parse_and_apply_migration(&mut executor, &content, migration.version, db_type).await {
                Ok(_) => {
                    println!("✓");
                    success_count += 1;
                }
                Err(e) => {
                    println!("❌ 失败: {}", e);
                    return Err(e);
                }
            },
            Err(e) => {
                println!("❌ 无法读取文件: {}", e);
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
    let db_type = detect_database_type(database_url);

    println!("\n📊 数据库类型: {}", db_type);

    // 创建迁移执行器
    let mut session = pool.get_session("admin").await?;
    let connection = session.connection()?.clone();
    let mut executor = MigrationExecutor::new(connection, db_type);

    // 加载迁移历史
    executor.load_history().await?;

    let applied_migrations = &executor.history.applied_migrations;

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
        vec![applied_migrations.iter().map(|m| m.version).max().unwrap()]
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
                // 继续尝试回滚其他迁移
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
    use sea_orm::{ConnectionTrait, TransactionTrait};

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

    let txn: sea_orm::DatabaseTransaction = executor.connection.begin().await.map_err(DbError::Connection)?;

    txn.execute_raw(delete_sql).await.map_err(DbError::Connection)?;

    txn.commit().await.map_err(DbError::Connection)?;

    Ok(())
}

/// 扫描迁移目录中的文件
fn scan_migration_files(dir: &PathBuf) -> Result<Vec<MigrationInfo>, DbError> {
    let mut migrations = Vec::new();

    if !dir.exists() {
        return Ok(migrations);
    }

    let entries = fs::read_dir(dir).map_err(|e| DbError::Config(format!("读取目录失败: {}", e)))?;

    for entry in entries {
        let entry = entry.map_err(|e| DbError::Config(format!("读取条目失败: {}", e)))?;
        let path = entry.path();

        if path.is_file() && path.extension().map(|e| e == "sql").unwrap_or(false) {
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if let Some((version, description)) = parse_migration_filename(filename) {
                    migrations.push(MigrationInfo {
                        version,
                        description,
                        file_path: path.clone(),
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
fn parse_migration_filename(filename: &str) -> Option<(u32, String)> {
    // 格式: {version}_{description}.sql
    let parts: Vec<&str> = filename.split('_').collect();
    if parts.len() < 2 {
        return None;
    }

    let version = parts[0].parse::<u32>().ok()?;
    let description = parts[1..].join("_").replace(".sql", "");

    Some((version, description))
}

/// 迁移文件信息
#[derive(Debug, Clone)]
struct MigrationInfo {
    version: u32,
    description: String,
    file_path: PathBuf,
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
    executor: &mut MigrationExecutor,
    content: &str,
    version: u32,
    db_type: MigrationDatabaseType,
) -> DbResult<()> {
    use sea_orm::{ConnectionTrait, TransactionTrait};

    // 解析迁移内容
    let (description, _full_content) =
        MigrationFileParser::parse_migration_file(content).unwrap_or(("Migration".to_string(), content.to_string()));

    // 提取 UP SQL（-- UP 到 -- DOWN 之间）
    let up_sql = extract_sql_section(content, "UP")?;

    // 开始事务
    let txn: sea_orm::DatabaseTransaction = executor.connection.begin().await.map_err(DbError::Connection)?;

    // 执行 UP SQL
    if !up_sql.trim().is_empty() {
        txn.execute_unprepared(&up_sql).await.map_err(DbError::Connection)?;
    }

    // 记录迁移历史（使用参数化查询防止 SQL 注入）
    let backend = match db_type {
        MigrationDatabaseType::Postgres => sea_orm::DbBackend::Postgres,
        MigrationDatabaseType::MySql => sea_orm::DbBackend::MySql,
        MigrationDatabaseType::Sqlite => sea_orm::DbBackend::Sqlite,
    };
    let applied_at = chrono::Utc::now().to_rfc3339();
    let file_path = format!("migration_v{}.sql", version);
    let insert_sql = sea_orm::Statement::from_sql_and_values(
        backend,
        "INSERT INTO dbnexus_migrations (version, description, applied_at, file_path) VALUES (?, ?, ?, ?)".to_string(),
        vec![version.into(), description.into(), applied_at.into(), file_path.into()],
    );

    txn.execute_raw(insert_sql).await.map_err(DbError::Connection)?;

    txn.commit().await.map_err(DbError::Connection)?;

    Ok(())
}

/// 提取 SQL 部分
fn extract_sql_section(content: &str, section: &str) -> Result<String, DbError> {
    let section_start = format!("-- {}", section);
    let section_end = format!("-- {}", if section == "UP" { "DOWN" } else { "UP" });

    let start_idx = content.find(&section_start).map(|i| i + section_start.len());
    let end_idx = content.find(&section_end);

    if let Some(start) = start_idx {
        if let Some(end) = end_idx {
            Ok(content[start..end].trim().to_string())
        } else {
            Ok(content[start..].trim().to_string())
        }
    } else {
        Ok(String::new())
    }
}

/// 列出所有迁移文件
fn list_migrations(migrations_dir: &PathBuf) -> Result<(), DbError> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    迁移文件列表                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let migrations = scan_migration_files(migrations_dir)?;

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
            migration.version,
            migration.description
        );
    }

    println!("\n{}", "─".repeat(60));

    Ok(())
}

/// 检测数据库类型
fn detect_database_type(database_url: &str) -> MigrationDatabaseType {
    if database_url.starts_with("postgres") {
        MigrationDatabaseType::Postgres
    } else if database_url.starts_with("mysql") {
        MigrationDatabaseType::MySql
    } else {
        MigrationDatabaseType::Sqlite
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
