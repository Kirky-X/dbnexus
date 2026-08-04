// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DBNexus 迁移 CLI 工具
//!
//! 提供数据库迁移的命令行界面

use clap::{Parser, Subcommand};
use dbnexus::foundation::DatabaseType as MigrationDatabaseType;
use dbnexus::i18n;
use dbnexus::{DbError, DbPool, DbResult};
use dbnexus::{MigrationExecutor, MigrationFile, MigrationFileParser};
use std::fs;
use std::path::{Path, PathBuf};
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

    /// 手动指定语言 (en, zh)
    #[arg(long)]
    lang: Option<String>,

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

    // 初始化语言设置
    if let Some(ref lang) = cli.lang {
        i18n::set_locale(lang)?;
    }

    // 确保迁移目录存在
    if !cli.migrations_dir.exists() {
        fs::create_dir_all(&cli.migrations_dir)
            .map_err(|e| DbError::Config(i18n::t("cli-dir-create-failed", &[("error", e.to_string())])))?;
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
async fn create_migration(description: &str, directory: &Path) -> DbResult<()> {
    // 创建迁移目录（如果不存在）
    fs::create_dir_all(directory)
        .map_err(|e| DbError::Config(i18n::t("cli-dir-create-failed", &[("error", e.to_string())])))?;

    // 生成时间戳作为版本号
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| DbError::Config(i18n::t("cli-timestamp-parse-failed", &[("error", e.to_string())])))?
        .as_secs();

    // 验证并清理描述，防止路径遍历和特殊字符攻击
    let sanitized_description = description
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect::<String>();

    if sanitized_description.is_empty() {
        return Err(DbError::Config(i18n::t_simple("cli-desc-special-chars-only")));
    }

    if sanitized_description.len() > 100 {
        return Err(DbError::Config(i18n::t_simple("cli-desc-too-long")));
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

    fs::write(&filepath, migration_content)
        .map_err(|e| DbError::Config(i18n::t("cli-file-write-failed", &[("error", e.to_string())])))?;

    println!(
        "{}",
        i18n::t("cli-migration-created", &[("path", filepath.display().to_string())])
    );

    Ok(())
}

/// 显示迁移状态
async fn show_status(database_url: &str, migrations_dir: &Path) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  {:58}  ║", i18n::t_simple("cli-status-title"));
    println!("╚══════════════════════════════════════════════════════════════╝");

    // 测试数据库连接
    let pool = match DbPool::new(database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            println!("\n{}", i18n::t("cli-db-connect-failed", &[("error", e.to_string())]));
            return Ok(());
        }
    };

    // 获取数据库类型
    let db_type = detect_database_type(database_url)
        .map_err(|e| DbError::Config(i18n::t("cli-db-type-detect-failed", &[("error", e.to_string())])))?;
    println!("\n{}", i18n::t("cli-db-type", &[("type", db_type.to_string())]));
    println!(
        "{}",
        i18n::t("cli-migrations-dir", &[("path", migrations_dir.display().to_string())])
    );

    // 加载迁移历史
    let session = match pool.get_session("admin").await {
        Ok(session) => session,
        Err(e) => {
            println!("\n{}", i18n::t("cli-session-failed", &[("error", e.to_string())]));
            return Ok(());
        }
    };

    let mut executor = session.create_migration_executor(db_type)?;

    if let Err(e) = executor.load_history().await {
        println!("\n{}", i18n::t("cli-history-load-failed", &[("error", e.to_string())]));
        println!("   {}", i18n::t_simple("cli-history-table-missing"));
        return Ok(());
    }

    let applied_count = executor.history().applied_migrations.len();
    println!(
        "\n{}",
        i18n::t("cli-applied-count", &[("count", applied_count.to_string())])
    );

    if applied_count > 0 {
        // 显示最新迁移信息
        if let Some(latest_version) = executor.history().get_latest_version()
            && let Some(latest_migration) = executor
                .history()
                .applied_migrations
                .iter()
                .find(|m| m.version == latest_version)
        {
            println!("   {}", i18n::t_simple("cli-latest-migration"));
            println!(
                "{}",
                i18n::t("cli-version", &[("version", latest_migration.version.to_string())])
            );
            println!(
                "{}",
                i18n::t(
                    "cli-description",
                    &[("description", latest_migration.description.clone())]
                )
            );
            println!(
                "{}",
                i18n::t("cli-applied-at", &[("time", latest_migration.applied_at.to_string())])
            );
        }

        // 显示所有已应用迁移
        println!("\n{}", i18n::t_simple("cli-history-details"));
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

    println!(
        "\n{}",
        i18n::t("cli-local-files", &[("count", local_migrations.len().to_string())])
    );
    println!(
        "{}",
        i18n::t("cli-pending-count", &[("count", pending_count.to_string())])
    );

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
            println!("\n   {}", i18n::t_simple("cli-pending-list"));
            for (idx, migration) in pending.iter().enumerate() {
                println!(
                    "   [{:2}] v{:6} - {}",
                    idx + 1,
                    migration.version(),
                    migration.description()
                );
            }
        } else {
            println!("\n   {}", i18n::t_simple("cli-all-applied"));
        }
    }

    // 显示数据库连接信息
    println!("\n{}", i18n::t_simple("cli-db-connected"));
    println!("{}", i18n::t("cli-db-url", &[("url", mask_database_url(database_url))]));

    println!("\n{}", "─".repeat(60));

    Ok(())
}

/// 测试数据库连接
async fn test_connection(database_url: &str) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  {:58}  ║", i18n::t_simple("cli-test-connection-title"));
    println!("╚══════════════════════════════════════════════════════════════╝");

    println!("\n{}", i18n::t_simple("cli-testing-connection"));

    let start_time = std::time::Instant::now();

    let pool = match DbPool::new(database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            println!("\n{}", i18n::t("cli-connection-failed", &[("error", e.to_string())]));
            return Err(e);
        }
    };

    let elapsed = start_time.elapsed();

    // 获取会话以验证连接
    match pool.get_session("admin").await {
        Ok(session) => {
            let _conn = session.connection()?.clone();
            drop(session);

            let db_type = detect_database_type(database_url).map_err(|e| {
                DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                    "cli-db-type-detect-failed",
                    &[("error", e.to_string())],
                )))
            })?;

            println!("\n{}", i18n::t_simple("cli-connection-success"));
            println!("\n{}", i18n::t("cli-db-type", &[("type", db_type.to_string())]));
            println!(
                "{}",
                i18n::t("cli-connection-time", &[("duration", format!("{:?}", elapsed))])
            );
            println!(
                "{}",
                i18n::t("cli-connection-url", &[("url", mask_database_url(database_url))])
            );

            // 显示连接池状态
            println!("\n   {}", i18n::t_simple("cli-pool-status"));
            let status = pool.status();
            println!(
                "     - {}",
                i18n::t("cli-total-connections", &[("count", status.total.to_string())])
            );
            println!(
                "     - {}",
                i18n::t("cli-active-connections", &[("count", status.active.to_string())])
            );
            println!(
                "     - {}",
                i18n::t("cli-idle-connections", &[("count", status.idle.to_string())])
            );
        }
        Err(e) => {
            println!(
                "\n{}",
                i18n::t("cli-connection-verify-failed", &[("error", e.to_string())])
            );
        }
    }

    println!("\n{}", "─".repeat(60));

    Ok(())
}

/// 运行向上的迁移（应用迁移）
async fn run_migrations_up(database_url: &str, migrations_dir: &Path, target_version: Option<u32>) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  {:58}  ║", i18n::t_simple("cli-apply-title"));
    println!("╚══════════════════════════════════════════════════════════════╝");

    let pool = DbPool::new(database_url).await?;
    let db_type = detect_database_type(database_url)?;

    println!("\n{}", i18n::t("cli-db-type", &[("type", db_type.to_string())]));
    println!(
        "{}",
        i18n::t("cli-migrations-dir", &[("path", migrations_dir.display().to_string())])
    );

    // 创建迁移执行器
    let session = pool.get_session("admin").await?;
    let mut executor = session.create_migration_executor(db_type)?;

    // 扫描迁移文件
    let migrations = executor.scan_migrations(migrations_dir)?;

    if migrations.is_empty() {
        println!("\n⚠️  {}", i18n::t_simple("cli-no-migration-files"));
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
        println!("\n✓ {}", i18n::t_simple("cli-no-pending"));
        return Ok(());
    }

    println!(
        "\n📦 {}",
        i18n::t("cli-found-pending", &[("count", to_apply.len().to_string())])
    );

    if let Some(target) = target_version {
        println!(
            "   {}",
            i18n::t("cli-target-version", &[("version", target.to_string())])
        );
    }

    // 应用迁移
    println!("\n🚀 {}", i18n::t_simple("cli-starting-apply"));
    let mut success_count = 0;

    for migration in &to_apply {
        print!(
            "   {} ",
            i18n::t(
                "cli-applying",
                &[
                    ("version", migration.version().to_string()),
                    ("description", migration.description().to_string())
                ]
            )
        );

        match executor.apply_migration_file_public(migration).await {
            Ok(_) => {
                println!("✓");
                success_count += 1;
            }
            Err(e) => {
                println!("❌ {}", i18n::t("cli-connection-failed", &[("error", e.to_string())]));
                return Err(e);
            }
        }
    }

    println!(
        "\n✅ {}",
        i18n::t(
            "cli-apply-success",
            &[
                ("success", success_count.to_string()),
                ("total", to_apply.len().to_string())
            ]
        )
    );
    println!("\n{}", "─".repeat(60));

    Ok(())
}

/// 运行向下的迁移（回滚迁移）
async fn run_migrations_down(database_url: &str, target_version: Option<u32>, rollback_all: bool) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  {:58}  ║", i18n::t_simple("cli-rollback-title"));
    println!("╚══════════════════════════════════════════════════════════════╝");

    let pool = DbPool::new(database_url).await?;
    let db_type = detect_database_type(database_url)?;

    println!("\n{}", i18n::t("cli-db-type", &[("type", db_type.to_string())]));

    // 创建迁移执行器
    let session = pool.get_session("admin").await?;
    let mut executor = session.create_migration_executor(db_type)?;

    // 加载迁移历史
    executor.load_history().await?;

    let applied_migrations = &executor.history().applied_migrations;

    if applied_migrations.is_empty() {
        println!("\n⚠️  {}", i18n::t_simple("cli-no-applied-rollback"));
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

    println!(
        "\n📦 {}",
        i18n::t(
            "cli-to-rollback-count",
            &[("count", versions_to_rollback.len().to_string())]
        )
    );

    if rollback_all {
        println!("   {}", i18n::t_simple("cli-mode-rollback-all"));
    } else if let Some(target) = target_version {
        println!(
            "   {}",
            i18n::t("cli-mode-rollback-version", &[("version", target.to_string())])
        );
    } else {
        println!("   {}", i18n::t_simple("cli-mode-rollback-last"));
    }

    // 执行回滚
    println!("\n🔄 {}", i18n::t_simple("cli-starting-rollback"));
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
        print!(
            "   {} ",
            i18n::t(
                "cli-rolling-back",
                &[("version", version.to_string()), ("description", description.clone())]
            )
        );

        match rollback_migration(&mut executor, *version, db_type).await {
            Ok(_) => {
                println!("✓");
                success_count += 1;
            }
            Err(e) => {
                println!("❌ {}", i18n::t("cli-connection-failed", &[("error", e.to_string())]));
                // 回滚失败时停止并返回错误，避免状态不一致
                println!("\n⚠️  {}", i18n::t_simple("cli-rollback-error-stop"));
                return Err(DbError::Migration(format!(
                    "Migration rollback failed for v{}: {}",
                    version, e
                )));
            }
        }
    }

    println!(
        "\n✅ {}",
        i18n::t(
            "cli-rollback-success",
            &[
                ("success", success_count.to_string()),
                ("total", versions_to_rollback.len().to_string())
            ]
        )
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
        // DuckDB 走独立连接路径，不通过 SeaORM DbBackend 执行迁移回滚
        MigrationDatabaseType::DuckDb => {
            return Err(DbError::Config(
                "Migration rollback for DuckDB is not supported via SeaORM backend".to_string(),
            ));
        }
        MigrationDatabaseType::Ladybug | MigrationDatabaseType::Neo4j => {
            return Err(DbError::Config(
                "Migration rollback for graph databases is not supported".to_string(),
            ));
        }
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
    output: &Path,
    description: &str,
) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  {:58}  ║", i18n::t_simple("cli-generate-title"));
    println!("╚══════════════════════════════════════════════════════════════╝");

    // 生成时间戳作为版本号
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| DbError::Config(i18n::t("cli-timestamp-parse-failed", &[("error", e.to_string())])))?
        .as_secs();

    // 如果提供了 schema 文件，尝试生成差异 SQL
    let migration_content;

    if let (Some(from), Some(to)) = (from_schema, to_schema) {
        println!("\n📄 {}", i18n::t_simple("cli-parsing-schema"));

        let from_content = fs::read_to_string(from)
            .map_err(|e| DbError::Config(i18n::t("cli-schema-read-source-failed", &[("error", e.to_string())])))?;
        let to_content = fs::read_to_string(to)
            .map_err(|e| DbError::Config(i18n::t("cli-schema-read-target-failed", &[("error", e.to_string())])))?;

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

        println!("✓ {}", i18n::t_simple("cli-schema-diff-generated"));
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

        println!("⚠️  {}", i18n::t_simple("cli-no-schema-template"));
    }

    // 确保输出目录存在
    if let Some(parent) = output.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .map_err(|e| DbError::Config(i18n::t("cli-output-dir-create-failed", &[("error", e.to_string())])))?;
    }

    // 写入文件
    fs::write(output, migration_content)
        .map_err(|e| DbError::Config(i18n::t("cli-file-write-failed", &[("error", e.to_string())])))?;

    println!(
        "\n✓ {}",
        i18n::t("cli-migration-created", &[("path", output.display().to_string())])
    );

    // 如果生成了实际 SQL，显示摘要
    if from_schema.is_some() && to_schema.is_some() {
        println!("   {}", i18n::t_simple("cli-check-edit-file"));
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
async fn list_migrations(database_url: &str, migrations_dir: &Path) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  {:58}  ║", i18n::t_simple("cli-list-title"));
    println!("╚══════════════════════════════════════════════════════════════╝");

    let pool = DbPool::new(database_url).await?;
    let db_type = detect_database_type(database_url)?;
    let session = pool.get_session("admin").await?;
    let executor = session.create_migration_executor(db_type)?;

    let migrations = executor.scan_migrations(migrations_dir)?;

    if migrations.is_empty() {
        println!("\n⚠️  {}", i18n::t_simple("cli-no-migration-files"));
        println!(
            "   {}",
            i18n::t("cli-list-directory", &[("path", migrations_dir.display().to_string())])
        );
        return Ok(());
    }

    println!(
        "\n{}",
        i18n::t("cli-migrations-dir", &[("path", migrations_dir.display().to_string())])
    );
    println!(
        "📦 {}\n",
        i18n::t("cli-list-total-count", &[("count", migrations.len().to_string())])
    );

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
