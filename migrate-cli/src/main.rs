//! DBNexus 迁移 CLI 工具
//!
//! 提供数据库迁移的命令行界面

use clap::{Parser, Subcommand};
use dbnexus::migration::{DatabaseType as MigrationDatabaseType, Migration, MigrationExecutor};
use dbnexus::{config::DbError, DbPool, DbResult};
use std::path::PathBuf;

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

    #[command(subcommand)]
    command: Commands,
}

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
        /// 目标版本号
        #[arg(long)]
        version: Option<u32>,
    },

    /// 回滚迁移
    Down {
        /// 目标版本号
        #[arg(long)]
        version: Option<u32>,
    },

    /// 查看迁移状态
    Status,

    /// 生成迁移文件
    Generate {
        /// 源 Schema 文件
        from_schema: PathBuf,

        /// 目标 Schema 文件
        to_schema: PathBuf,

        /// 输出迁移文件路径
        #[arg(short, long, default_value = "./migrations/generated.sql")]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Create {
            description,
            directory,
        } => {
            create_migration(description, directory).await?;
        }
        Commands::Up {
            version,
        } => {
            run_migrations(&cli.database_url, *version, true).await?;
        }
        Commands::Down {
            version,
        } => {
            run_migrations(&cli.database_url, *version, false).await?;
        }
        Commands::Status => {
            show_status_impl(&cli.database_url).await?;
        }
        Commands::Generate {
            from_schema,
            to_schema,
            output,
        } => {
            generate_migration_impl(from_schema, to_schema, output).await?;
        }
    }

    Ok(())
}

async fn create_migration(description: &str, directory: &PathBuf) -> DbResult<()> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        "-- Migration: {}\n-- Version: {}\n\n-- UP\n-- Your migration SQL goes here\n\n-- DOWN\n-- Reversal of migration SQL goes here\n",
        description, timestamp
    );

    fs::write(&filepath, migration_content).map_err(|e| DbError::Config(format!("无法写入迁移文件: {}", e)))?;
    println!("✓ 迁移文件已创建: {}", filepath.display());

    Ok(())
}

async fn show_status_impl(database_url: &str) -> DbResult<()> {
    use dbnexus::migration::{DatabaseType, MigrationExecutor};

    println!("迁移状态:");
    println!("  数据库 URL: {}", database_url);

    // 根据数据库URL确定数据库类型
    let db_type = if database_url.starts_with("postgres") {
        DatabaseType::Postgres
    } else if database_url.starts_with("mysql") {
        DatabaseType::MySQL
    } else {
        DatabaseType::SQLite
    };

    // 创建连接池并获取连接
    let pool = DbPool::new(database_url).await?;
    let mut session = pool.get_session("admin").await?;
    let connection = session.connection()?.clone();

    // 创建迁移执行器
    let mut executor = MigrationExecutor::new(connection, db_type);

    // 加载迁移历史
    if let Err(e) = executor.load_history().await {
        println!("  状态: 无法加载迁移历史 - {}", e);
        return Ok(());
    }

    let applied_count = executor.history.applied_migrations.len();
    println!("  应用的迁移: {}", applied_count);
    if applied_count > 0 {
        if let Some(latest_version) = executor.history.get_latest_version() {
            // 获取最新版本的详细信息
            if let Some(latest_migration) = executor
                .history
                .applied_migrations
                .iter()
                .find(|m| m.version == latest_version)
            {
                println!(
                    "  最新迁移版本: {} (应用时间: {})",
                    latest_migration.version, latest_migration.applied_at
                );
            }
        }
    }

    // 这里可以显示待应用的迁移数量（需要提供所有迁移定义）
    println!("  待应用的迁移: 未知 (需要提供迁移定义文件)");

    Ok(())
}

async fn generate_migration_impl(from_schema: &PathBuf, to_schema: &PathBuf, output: &PathBuf) -> DbResult<()> {
    use std::fs;

    // 读取源和目标 schema 文件
    let _from_content =
        fs::read_to_string(from_schema).map_err(|e| DbError::Config(format!("无法读取源 schema 文件: {}", e)))?;
    let _to_content =
        fs::read_to_string(to_schema).map_err(|e| DbError::Config(format!("无法读取目标 schema 文件: {}", e)))?;

    // 这里应该解析 schema 并生成差异 SQL
    // 为了示例，我们生成一个简单的模板
    let migration_content = format!(
        "-- 自动生成的迁移文件\n-- 从: {}\n-- 到: {}\n\n-- 请手动编辑此文件以包含实际的迁移 SQL\n",
        from_schema.display(),
        to_schema.display()
    );

    fs::write(output, migration_content).map_err(|e| DbError::Config(format!("无法写入迁移文件: {}", e)))?;

    println!("✓ 迁移文件已生成: {}", output.display());

    Ok(())
}

async fn run_migrations(database_url: &str, target_version: Option<u32>, up: bool) -> DbResult<()> {
    // 根据数据库URL确定数据库类型
    let db_type = if database_url.starts_with("postgres") {
        MigrationDatabaseType::Postgres
    } else if database_url.starts_with("mysql") {
        MigrationDatabaseType::MySQL
    } else {
        MigrationDatabaseType::SQLite
    };

    // 创建连接池
    let pool = DbPool::new(database_url).await?;
    let mut session = pool.get_session("admin").await?;

    // 获取数据库连接
    let connection = session.connection()?.clone();

    // 创建迁移执行器
    let mut executor = MigrationExecutor::new(connection, db_type);

    if up {
        // 应用迁移
        if let Some(version) = target_version {
            println!("正在应用到版本 {} 的迁移...", version);
        } else {
            println!("正在应用所有待处理的迁移...");
        }

        // 这里需要加载迁移文件并应用
        // 为了示例，我们创建一个空迁移
        let migration = Migration::new(1, "Initial migration".to_string());
        executor.apply_migration(&migration).await?;

        println!("✓ 迁移应用完成");
    } else {
        // 回滚迁移
        if let Some(version) = target_version {
            println!("正在回滚到版本 {} ...", version);
        } else {
            println!("正在回滚所有应用的迁移...");
        }

        println!("⚠️  回滚功能待实现");
    }

    Ok(())
}

#[allow(dead_code)]
async fn show_status(database_url: &str) -> DbResult<()> {
    println!("迁移状态:");
    println!("  数据库 URL: {}", database_url);
    println!("  状态: 待连接...");

    // 这里可以显示实际的迁移状态
    println!("  应用的迁移: 0");
    println!("  待应用的迁移: 0");

    Ok(())
}

#[allow(dead_code)]
async fn generate_migration(from_schema: &PathBuf, to_schema: &PathBuf, output: &PathBuf) -> DbResult<()> {
    use std::fs;

    // 读取源和目标 schema 文件（变量暂未使用，保留用于后续实现）
    let _from_content =
        fs::read_to_string(from_schema).map_err(|e| DbError::Config(format!("无法读取源 schema 文件: {}", e)))?;
    let _to_content =
        fs::read_to_string(to_schema).map_err(|e| DbError::Config(format!("无法读取目标 schema 文件: {}", e)))?;

    // 这里应该解析 schema 并生成差异 SQL
    // 为了示例，我们生成一个简单的模板
    let migration_content = format!(
        "-- 自动生成的迁移文件\n-- 从: {}\n-- 到: {}\n\n-- 请手动编辑此文件以包含实际的迁移 SQL\n",
        from_schema.display(),
        to_schema.display()
    );

    fs::write(output, migration_content).map_err(|e| DbError::Config(format!("无法写入迁移文件: {}", e)))?;

    println!("✓ 迁移文件已生成: {}", output.display());

    Ok(())
}
