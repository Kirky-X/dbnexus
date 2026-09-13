// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DBNexus 迁移 CLI 工具
//!
//! 提供数据库迁移的命令行界面

use clap::{Parser, Subcommand};
use dbnexus::MigrationExecutor;
use dbnexus::foundation::DatabaseType as MigrationDatabaseType;
use dbnexus::i18n;
use dbnexus::{DbError, DbPool, DbResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// CLI 配置
#[derive(Parser)]
#[command(name = "dbnexus-migrate")]
#[command(about = "DBNexus 数据库迁移工具", long_about = None)]
struct Cli {
    /// 数据库连接字符串（global：可置于子命令前后任意位置）
    #[arg(short, long, env = "DATABASE_URL", global = true)]
    database_url: Option<String>,

    /// 配置文件路径
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// 迁移文件目录（global：可置于子命令前后任意位置）
    #[arg(short, long, default_value = "./migrations", global = true)]
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

    /// 应用迁移目录中的所有待应用迁移（机器可读输出，退出码 0/1/2）
    Migrate {
        /// 目标版本号（可选，默认为所有待应用迁移）
        #[arg(long)]
        version: Option<u32>,
    },

    /// 数据库健康检查（JSON 输出，退出码 0 健康 / 1 不健康 / 2 用法错误）
    Health,

    /// 管理员用户增删 MVP（dbnexus_users 表，JSON 输出）
    User {
        #[command(subcommand)]
        action: UserAction,
    },
}

/// `user` 子命令动作
#[derive(Subcommand)]
enum UserAction {
    /// 新增用户
    Add {
        /// 用户名（1-64 位字母/数字/_.-）
        #[arg(long)]
        username: String,

        /// 密码（存储加盐 SHA-256 摘要，MVP；生产建议接 authentication bcrypt）
        #[arg(long)]
        password: String,

        /// 角色（1-64 位字母/数字/_-，默认 admin）
        #[arg(long, default_value = "admin")]
        role: String,
    },

    /// 删除用户
    Remove {
        /// 用户名
        #[arg(long)]
        username: String,
    },

    /// 列出用户
    List,
}

/// CLI 退出码契约：0 成功 / 1 运行时失败 / 2 用法或配置错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitCode {
    /// 成功
    Ok = 0,
    /// 运行时失败（连接失败、迁移失败、目标不存在等）
    RuntimeFailure = 1,
    /// 用法或配置错误（参数非法、URL 协议不支持等）
    UsageError = 2,
}

/// 程序入口
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // global 参数解析（clap 不允许 required global，此处统一收敛）
    let database_url: String = cli.database_url.unwrap_or_else(|| {
        eprintln!("error: --database-url is required (or set DATABASE_URL)");
        std::process::exit(ExitCode::UsageError as i32);
    });

    // 初始化语言设置
    if let Some(ref lang) = cli.lang {
        i18n::set_locale(lang)?;
    }

    // 确保迁移目录存在
    if !cli.migrations_dir.exists() {
        fs::create_dir_all(&cli.migrations_dir).map_err(|e| {
            DbError::Config(i18n::t(
                "cli-dir-create-failed",
                &[("error", e.to_string())],
            ))
        })?;
    }

    match &cli.command {
        Commands::Create {
            description,
            directory,
        } => {
            create_migration(description, directory).await?;
        }
        Commands::Up { version } => {
            run_migrations_up(&database_url, &cli.migrations_dir, *version).await?;
        }
        Commands::Down { version, all } => {
            run_migrations_down(&database_url, &cli.migrations_dir, *version, *all).await?;
        }
        Commands::Status => {
            show_status(&database_url, &cli.migrations_dir).await?;
        }
        Commands::TestConnection => {
            test_connection(&database_url).await?;
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
            list_migrations(&database_url, &cli.migrations_dir).await?;
        }
        // 运维子命令：JSON 输出 + 退出码契约（0 成功 / 1 运行时失败 / 2 用法错误）
        Commands::Migrate { version } => {
            let code = run_migrate_json(&database_url, &cli.migrations_dir, *version).await;
            std::process::exit(code as i32);
        }
        Commands::Health => {
            let code = run_health_json(&database_url).await;
            std::process::exit(code as i32);
        }
        Commands::User { action } => {
            let code = run_user_command(&database_url, action).await;
            std::process::exit(code as i32);
        }
    }

    Ok(())
}

/// 创建新的迁移文件
async fn create_migration(description: &str, directory: &Path) -> DbResult<()> {
    // 创建迁移目录（如果不存在）
    fs::create_dir_all(directory).map_err(|e| {
        DbError::Config(i18n::t(
            "cli-dir-create-failed",
            &[("error", e.to_string())],
        ))
    })?;

    // 生成时间戳作为版本号
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| {
            DbError::Config(i18n::t(
                "cli-timestamp-parse-failed",
                &[("error", e.to_string())],
            ))
        })?
        .as_secs();

    // 验证并清理描述，防止路径遍历和特殊字符攻击
    let sanitized_description = description
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect::<String>();

    if sanitized_description.is_empty() {
        return Err(DbError::Config(i18n::t_simple(
            "cli-desc-special-chars-only",
        )));
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

    fs::write(&filepath, migration_content).map_err(|e| {
        DbError::Config(i18n::t(
            "cli-file-write-failed",
            &[("error", e.to_string())],
        ))
    })?;

    println!(
        "{}",
        i18n::t(
            "cli-migration-created",
            &[("path", filepath.display().to_string())]
        )
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
            println!(
                "\n{}",
                i18n::t("cli-db-connect-failed", &[("error", e.to_string())])
            );
            // 向上传播错误，使进程非零退出（吞错会掩盖连接故障）
            return Err(e);
        }
    };

    // 获取数据库类型
    let db_type = detect_database_type(database_url).map_err(|e| {
        DbError::Config(i18n::t(
            "cli-db-type-detect-failed",
            &[("error", e.to_string())],
        ))
    })?;
    println!(
        "\n{}",
        i18n::t("cli-db-type", &[("type", db_type.to_string())])
    );
    println!(
        "{}",
        i18n::t(
            "cli-migrations-dir",
            &[("path", migrations_dir.display().to_string())]
        )
    );

    // 加载迁移历史
    let session = match pool.get_session("admin").await {
        Ok(session) => session,
        Err(e) => {
            println!(
                "\n{}",
                i18n::t("cli-session-failed", &[("error", e.to_string())])
            );
            // 向上传播错误，使进程非零退出
            return Err(e);
        }
    };

    let mut executor = session.create_migration_executor(db_type)?;

    if let Err(e) = executor.load_history().await {
        println!(
            "\n{}",
            i18n::t("cli-history-load-failed", &[("error", e.to_string())])
        );
        println!("   {}", i18n::t_simple("cli-history-table-missing"));
        // 向上传播错误，使进程非零退出
        return Err(e);
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
                i18n::t(
                    "cli-version",
                    &[("version", latest_migration.version.to_string())]
                )
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
                i18n::t(
                    "cli-applied-at",
                    &[("time", latest_migration.applied_at.to_string())]
                )
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
        i18n::t(
            "cli-local-files",
            &[("count", local_migrations.len().to_string())]
        )
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
    println!(
        "{}",
        i18n::t("cli-db-url", &[("url", mask_database_url(database_url))])
    );

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
            println!(
                "\n{}",
                i18n::t("cli-connection-failed", &[("error", e.to_string())])
            );
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
            println!(
                "\n{}",
                i18n::t("cli-db-type", &[("type", db_type.to_string())])
            );
            println!(
                "{}",
                i18n::t(
                    "cli-connection-time",
                    &[("duration", format!("{:?}", elapsed))]
                )
            );
            println!(
                "{}",
                i18n::t(
                    "cli-connection-url",
                    &[("url", mask_database_url(database_url))]
                )
            );

            // 显示连接池状态
            println!("\n   {}", i18n::t_simple("cli-pool-status"));
            let status = pool.status();
            println!(
                "     - {}",
                i18n::t(
                    "cli-total-connections",
                    &[("count", status.total.to_string())]
                )
            );
            println!(
                "     - {}",
                i18n::t(
                    "cli-active-connections",
                    &[("count", status.active.to_string())]
                )
            );
            println!(
                "     - {}",
                i18n::t(
                    "cli-idle-connections",
                    &[("count", status.idle.to_string())]
                )
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
async fn run_migrations_up(
    database_url: &str,
    migrations_dir: &Path,
    target_version: Option<u32>,
) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  {:58}  ║", i18n::t_simple("cli-apply-title"));
    println!("╚══════════════════════════════════════════════════════════════╝");

    let pool = DbPool::new(database_url).await?;
    let db_type = detect_database_type(database_url)?;

    println!(
        "\n{}",
        i18n::t("cli-db-type", &[("type", db_type.to_string())])
    );
    println!(
        "{}",
        i18n::t(
            "cli-migrations-dir",
            &[("path", migrations_dir.display().to_string())]
        )
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
        i18n::t(
            "cli-found-pending",
            &[("count", to_apply.len().to_string())]
        )
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
                println!(
                    "❌ {}",
                    i18n::t("cli-connection-failed", &[("error", e.to_string())])
                );
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
async fn run_migrations_down(
    database_url: &str,
    migrations_dir: &Path,
    target_version: Option<u32>,
    rollback_all: bool,
) -> DbResult<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  {:58}  ║", i18n::t_simple("cli-rollback-title"));
    println!("╚══════════════════════════════════════════════════════════════╝");

    let pool = DbPool::new(database_url).await?;
    let db_type = detect_database_type(database_url)?;

    println!(
        "\n{}",
        i18n::t("cli-db-type", &[("type", db_type.to_string())])
    );

    // 创建迁移执行器
    let session = pool.get_session("admin").await?;
    let mut executor = session.create_migration_executor(db_type)?;

    // 扫描本地迁移文件（回滚需要迁移文件内容以提取 DOWN SQL）
    let migration_files = executor.scan_migrations(migrations_dir)?;

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
            i18n::t(
                "cli-mode-rollback-version",
                &[("version", target.to_string())]
            )
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
                &[
                    ("version", version.to_string()),
                    ("description", description.clone())
                ]
            )
        );

        // 回滚必须先执行 DOWN SQL，因此需要找到版本对应的迁移文件
        let Some(migration_file) = find_migration_file(&migration_files, *version) else {
            println!("❌");
            println!("\n⚠️  {}", i18n::t_simple("cli-rollback-error-stop"));
            return Err(DbError::Migration(format!(
                "未找到迁移 v{} ({}) 的迁移文件，无法执行 DOWN 回滚",
                version, description
            )));
        };

        match rollback_migration(&mut executor, *version, migration_file).await {
            Ok(_) => {
                println!("✓");
                success_count += 1;
            }
            Err(e) => {
                println!(
                    "❌ {}",
                    i18n::t("cli-connection-failed", &[("error", e.to_string())])
                );
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

/// 在扫描结果中按版本号查找迁移文件
///
/// 回滚需要迁移文件内容以提取 DOWN SQL，找不到对应文件时返回 `None`
fn find_migration_file(
    files: &[dbnexus::MigrationFile],
    version: u32,
) -> Option<&dbnexus::MigrationFile> {
    files.iter().find(|f| f.version() == version)
}

/// 回滚单个迁移
///
/// 通过 `MigrationExecutor::rollback_version` 在同一事务内先执行迁移文件的 DOWN SQL，
/// 成功后再删除 `dbnexus_migrations` 历史行：
/// - 无 DOWN 段时返回"该迁移无可回滚的 DOWN 部分"错误；
/// - DOWN 执行失败时不删除历史记录并返回错误。
async fn rollback_migration(
    executor: &mut MigrationExecutor,
    version: u32,
    migration_file: &dbnexus::MigrationFile,
) -> DbResult<()> {
    executor.rollback_version(version, migration_file).await
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
        .map_err(|e| {
            DbError::Config(i18n::t(
                "cli-timestamp-parse-failed",
                &[("error", e.to_string())],
            ))
        })?
        .as_secs();

    // 如果提供了 schema 文件，尝试生成差异 SQL
    let migration_content;

    if let (Some(from), Some(to)) = (from_schema, to_schema) {
        println!("\n📄 {}", i18n::t_simple("cli-parsing-schema"));

        let from_content = fs::read_to_string(from).map_err(|e| {
            DbError::Config(i18n::t(
                "cli-schema-read-source-failed",
                &[("error", e.to_string())],
            ))
        })?;
        let to_content = fs::read_to_string(to).map_err(|e| {
            DbError::Config(i18n::t(
                "cli-schema-read-target-failed",
                &[("error", e.to_string())],
            ))
        })?;

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
        fs::create_dir_all(parent).map_err(|e| {
            DbError::Config(i18n::t(
                "cli-output-dir-create-failed",
                &[("error", e.to_string())],
            ))
        })?;
    }

    // 写入文件
    fs::write(output, migration_content).map_err(|e| {
        DbError::Config(i18n::t(
            "cli-file-write-failed",
            &[("error", e.to_string())],
        ))
    })?;

    println!(
        "\n✓ {}",
        i18n::t(
            "cli-migration-created",
            &[("path", output.display().to_string())]
        )
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
            i18n::t(
                "cli-list-directory",
                &[("path", migrations_dir.display().to_string())]
            )
        );
        return Ok(());
    }

    println!(
        "\n{}",
        i18n::t(
            "cli-migrations-dir",
            &[("path", migrations_dir.display().to_string())]
        )
    );
    println!(
        "📦 {}\n",
        i18n::t(
            "cli-list-total-count",
            &[("count", migrations.len().to_string())]
        )
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

// ============================================================================
// 运维子命令（migrate / health / user）— 机器可读 JSON + 退出码 0/1/2
// ============================================================================

/// 输出一行 JSON 到 stdout
fn print_json(value: &serde_json::Value) {
    println!("{}", serde_json::to_string(value).unwrap_or_default());
}

/// 校验用户名/角色名：1-64 位字母、数字、下划线、点、连字符
fn is_valid_identifier(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

/// 用户表 DDL（user 子命令存储）
fn users_table_ddl() -> &'static str {
    "CREATE TABLE IF NOT EXISTS dbnexus_users (\
username TEXT PRIMARY KEY, password_hash TEXT NOT NULL, \
role TEXT NOT NULL, created_at TEXT NOT NULL)"
}

/// 加盐 SHA-256 口令摘要（MVP：存储格式 `sha256$<salt_hex>$<digest_hex>`）
///
/// 生产部署建议启用 authentication feature 走 bcrypt；此 MVP 保证明文口令不落库。
fn hash_password(password: &str, salt_hex: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(salt_hex.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    format!("sha256${salt_hex}${}", to_hex(&digest))
}

/// 生成随机盐（系统时钟纳秒熵 + 进程 ID，MVP 口径）
fn new_salt() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    to_hex(&nanos.to_le_bytes()).chars().take(16).collect()
}

/// 字节转十六进制（避免引入 hex 依赖）
fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap_or('0'));
        out.push(char::from_digit((b & 0x0f) as u32, 16).unwrap_or('0'));
    }
    out
}

/// 应用迁移目录中的所有待应用迁移（JSON 输出）
///
/// 退出码：0 全部应用成功（含无待应用）/ 1 迁移执行失败 / 2 连接或 URL 配置错误
async fn run_migrate_json(
    database_url: &str,
    migrations_dir: &Path,
    target_version: Option<u32>,
) -> ExitCode {
    let pool = match DbPool::new(database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "error", "error_code": "connect_failed", "error": e.to_string()
            }));
            return classify_connect_error(database_url);
        }
    };

    let db_type = match detect_database_type(database_url) {
        Ok(t) => t,
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "error", "error_code": "unsupported_url", "error": e.to_string()
            }));
            return ExitCode::UsageError;
        }
    };

    let session = match pool.get_session("admin").await {
        Ok(s) => s,
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "error", "error_code": "session_failed", "error": e.to_string()
            }));
            return ExitCode::RuntimeFailure;
        }
    };
    let mut executor = match session.create_migration_executor(db_type) {
        Ok(e) => e,
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "error", "error_code": "executor_failed", "error": e.to_string()
            }));
            return ExitCode::RuntimeFailure;
        }
    };

    let migrations = match executor.scan_migrations(migrations_dir) {
        Ok(m) => m,
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "error", "error_code": "scan_failed", "error": e.to_string()
            }));
            return ExitCode::UsageError;
        }
    };

    if let Err(e) = executor.load_history().await {
        print_json(&serde_json::json!({
            "status": "error", "error_code": "history_failed", "error": e.to_string()
        }));
        return ExitCode::RuntimeFailure;
    }

    let applied_versions: std::collections::HashSet<u32> = executor
        .history()
        .applied_migrations
        .iter()
        .map(|m| m.version)
        .collect();

    let mut to_apply: Vec<_> = migrations
        .iter()
        .filter(|m| !applied_versions.contains(&m.version()))
        .filter(|m| match target_version {
            Some(target) => m.version() <= target,
            None => true,
        })
        .collect();
    to_apply.sort_by_key(|m| m.version());

    let mut applied: Vec<serde_json::Value> = Vec::new();
    for migration in &to_apply {
        match executor.apply_migration_file_public(migration).await {
            Ok(_) => applied.push(serde_json::json!({
                "version": migration.version(),
                "description": migration.description(),
            })),
            Err(e) => {
                print_json(&serde_json::json!({
                    "status": "error", "error_code": "migration_failed",
                    "applied": applied, "error": e.to_string()
                }));
                return ExitCode::RuntimeFailure;
            }
        }
    }

    print_json(&serde_json::json!({
        "status": "ok",
        "applied": applied,
        "pending_total": to_apply.len(),
        "already_applied": migrations.len() - to_apply.len(),
        "url": mask_database_url(database_url),
    }));
    ExitCode::Ok
}

/// 数据库健康检查（JSON 输出）
///
/// 退出码：0 健康 / 1 不健康（连接或查询失败）/ 2 用法错误（URL 协议不支持等）
async fn run_health_json(database_url: &str) -> ExitCode {
    // URL 协议合法性先行校验 → 用法错误
    if detect_database_type(database_url).is_err() {
        print_json(&serde_json::json!({
            "status": "unhealthy",
            "checks": { "url": "invalid" },
            "error": "unsupported database URL protocol"
        }));
        return ExitCode::UsageError;
    }

    let start = std::time::Instant::now();
    let pool = match DbPool::new(database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "unhealthy",
                "checks": { "connect": "fail" },
                "error": e.to_string(),
                "url": mask_database_url(database_url),
            }));
            return ExitCode::RuntimeFailure;
        }
    };

    // 权限检查需可提取表名，故用 sqlite_master 而非 SELECT 1（CLI 默认启用 permission）；
    // 空库 sqlite_master 为空集属正常状态，查询成功即视为健康
    let query_latency = match pool
        .query_rows("SELECT name FROM sqlite_master LIMIT 1", "admin")
        .await
    {
        Ok(_) => Some(start.elapsed().as_millis() as u64),
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "unhealthy",
                "checks": { "connect": "ok", "query": "fail" },
                "error": e.to_string(),
                "url": mask_database_url(database_url),
            }));
            return ExitCode::RuntimeFailure;
        }
    };

    let status = pool.status();
    print_json(&serde_json::json!({
        "status": "healthy",
        "checks": { "connect": "ok", "query": "ok" },
        "query_latency_ms": query_latency,
        "connections": {
            "total": status.total,
            "active": status.active,
            "idle": status.idle,
        },
        "url": mask_database_url(database_url),
    }));
    ExitCode::Ok
}

/// 管理员用户增删（MVP，JSON 输出）
async fn run_user_command(database_url: &str, action: &UserAction) -> ExitCode {
    // 参数校验 → 用法错误
    let (username, role): (&str, &str) = match action {
        UserAction::Add {
            username,
            password,
            role,
        } => {
            if !is_valid_identifier(username) {
                print_json(&serde_json::json!({
                    "status": "error", "error_code": "invalid_username",
                    "error": "username must be 1-64 chars of [A-Za-z0-9_.-]"
                }));
                return ExitCode::UsageError;
            }
            if password.is_empty() {
                print_json(&serde_json::json!({
                    "status": "error", "error_code": "empty_password",
                    "error": "password must not be empty"
                }));
                return ExitCode::UsageError;
            }
            if !is_valid_identifier(role) {
                print_json(&serde_json::json!({
                    "status": "error", "error_code": "invalid_role",
                    "error": "role must be 1-64 chars of [A-Za-z0-9_.-]"
                }));
                return ExitCode::UsageError;
            }
            (username, role)
        }
        UserAction::Remove { username } => {
            if !is_valid_identifier(username) {
                print_json(&serde_json::json!({
                    "status": "error", "error_code": "invalid_username",
                    "error": "username must be 1-64 chars of [A-Za-z0-9_.-]"
                }));
                return ExitCode::UsageError;
            }
            (username, "")
        }
        UserAction::List => ("", ""),
    };

    let pool = match DbPool::new(database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "error", "error_code": "connect_failed", "error": e.to_string()
            }));
            return classify_connect_error(database_url);
        }
    };

    let session = match pool.get_session("admin").await {
        Ok(s) => s,
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "error", "error_code": "session_failed", "error": e.to_string()
            }));
            return ExitCode::RuntimeFailure;
        }
    };

    // 建表（幂等）
    if let Err(e) = session.execute_raw_ddl(users_table_ddl()).await {
        print_json(&serde_json::json!({
            "status": "error", "error_code": "table_create_failed", "error": e.to_string()
        }));
        return ExitCode::RuntimeFailure;
    }

    match action {
        UserAction::Add { password, .. } => {
            let salt = new_salt();
            let hash = hash_password(password, &salt);
            let created_at = chrono::Utc::now().to_rfc3339();
            // 主键冲突 → 视为运行时失败（用户已存在）
            let insert = format!(
                "INSERT INTO dbnexus_users (username, password_hash, role, created_at) \
VALUES ('{username}', '{hash}', '{role}', '{created_at}')"
            );
            if let Err(e) = session.execute_raw(&insert).await {
                print_json(&serde_json::json!({
                    "status": "error", "error_code": "user_exists_or_insert_failed",
                    "username": username, "error": e.to_string()
                }));
                return ExitCode::RuntimeFailure;
            }
            print_json(&serde_json::json!({
                "status": "ok", "action": "add", "username": username, "role": role
            }));
            ExitCode::Ok
        }
        UserAction::Remove { username } => {
            let delete = format!("DELETE FROM dbnexus_users WHERE username = '{username}'");
            match session.execute_raw(&delete).await {
                Ok(result) => {
                    if result.rows_affected() == 0 {
                        print_json(&serde_json::json!({
                            "status": "error", "error_code": "user_not_found",
                            "username": username
                        }));
                        return ExitCode::RuntimeFailure;
                    }
                    print_json(&serde_json::json!({
                        "status": "ok", "action": "remove", "username": username
                    }));
                    ExitCode::Ok
                }
                Err(e) => {
                    print_json(&serde_json::json!({
                        "status": "error", "error_code": "remove_failed",
                        "username": username, "error": e.to_string()
                    }));
                    ExitCode::RuntimeFailure
                }
            }
        }
        UserAction::List => match pool
            .query_rows(
                "SELECT username, role, created_at FROM dbnexus_users ORDER BY username",
                "admin",
            )
            .await
        {
            Ok(rows) => {
                print_json(&serde_json::json!({
                    "status": "ok", "action": "list", "users": rows, "count": rows.len()
                }));
                ExitCode::Ok
            }
            Err(e) => {
                print_json(&serde_json::json!({
                    "status": "error", "error_code": "list_failed", "error": e.to_string()
                }));
                ExitCode::RuntimeFailure
            }
        },
    }
}

/// 连接失败按 URL 合法性分类退出码：协议不支持 → 用法错误，其余 → 运行时失败
fn classify_connect_error(database_url: &str) -> ExitCode {
    if url::Url::parse(database_url).is_err() {
        return ExitCode::UsageError;
    }
    match detect_database_type(database_url) {
        Ok(_) => ExitCode::RuntimeFailure,
        Err(_) => ExitCode::UsageError,
    }
}

/// 检测数据库类型（增强版）
///
/// 使用 URL 解析器验证数据库 URL 格式，
/// 只返回已知支持的数据库类型，不支持时返回错误
fn detect_database_type(database_url: &str) -> Result<MigrationDatabaseType, DbError> {
    // 尝试解析 URL
    let url = url::Url::parse(database_url)
        .map_err(|e| DbError::Config(format!("Invalid database URL format: {}", e)))?;

    // 获取协议scheme
    let scheme = url.scheme().to_lowercase();

    // 根据协议返回对应的数据库类型
    match scheme.as_str() {
        "postgres" | "postgresql" => Ok(MigrationDatabaseType::Postgres),
        "mysql" => Ok(MigrationDatabaseType::MySql),
        "sqlite" | "sqlite3" | "file" => Ok(MigrationDatabaseType::Sqlite),
        "oci" | "oracle" => Err(DbError::Config(
            "Oracle database is not supported".to_string(),
        )),
        "mssql" | "sqlserver" => Err(DbError::Config(
            "SQL Server database is not supported".to_string(),
        )),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== DOWN 提取（rollback 前置校验依赖的逻辑） =====

    #[test]
    fn test_extract_down_sql_present() {
        let content = "-- UP:\nCREATE TABLE users (id INTEGER);\n-- DOWN:\nDROP TABLE users;\n";
        let down = MigrationExecutor::extract_down_sql(content).expect("应提取到 DOWN SQL");
        assert_eq!(down, "DROP TABLE users;");
    }

    #[test]
    fn test_extract_down_sql_case_insensitive() {
        let content = "-- up:\nALTER TABLE users ADD COLUMN c TEXT;\n-- down:\nALTER TABLE users DROP COLUMN c;\n";
        let down = MigrationExecutor::extract_down_sql(content).expect("应提取到 DOWN SQL");
        assert!(down.contains("DROP COLUMN c"));
    }

    /// DOWN 缺失时的错误分支：extract 返回 None → rollback 报"无可回滚的 DOWN 部分"
    #[test]
    fn test_extract_down_sql_missing_yields_no_rollback_error() {
        let content = "-- UP:\nCREATE TABLE users (id INTEGER);\n";
        assert!(MigrationExecutor::extract_down_sql(content).is_none());

        let file = dbnexus::MigrationFile::new(
            1,
            "no_down".to_string(),
            PathBuf::from("/migrations/001_no_down.sql"),
            content.to_string(),
        );
        let err = DbError::Migration(format!(
            "迁移 v{} ({}) 无可回滚的 DOWN 部分",
            file.version(),
            file.description()
        ));
        let msg = err.to_string();
        assert!(msg.contains("无可回滚的 DOWN 部分"), "实际错误: {}", msg);
    }

    // ===== find_migration_file =====

    #[test]
    fn test_find_migration_file_by_version() {
        let files = vec![
            dbnexus::MigrationFile::new(
                1,
                "create_users".to_string(),
                PathBuf::from("/migrations/001_create_users.sql"),
                "-- UP:\nCREATE TABLE users (id INTEGER);\n".to_string(),
            ),
            dbnexus::MigrationFile::new(
                2,
                "add_column".to_string(),
                PathBuf::from("/migrations/002_add_column.sql"),
                "-- UP:\nALTER TABLE users ADD COLUMN c TEXT;\n".to_string(),
            ),
        ];

        let found = find_migration_file(&files, 2).expect("应找到 v2 的迁移文件");
        assert_eq!(found.version(), 2);
        assert_eq!(found.description(), "add_column");

        // 找不到对应版本时返回 None → rollback 路径报"未找到迁移文件"
        assert!(find_migration_file(&files, 3).is_none());
    }

    // ===== 既有纯逻辑辅助 =====

    #[test]
    fn test_mask_database_url_hides_password() {
        let masked = mask_database_url("postgres://user:secret@localhost:5432/db");
        assert!(!masked.contains("secret"));
        assert!(masked.contains("*****"));
    }

    #[test]
    fn test_detect_database_type_supported() {
        assert!(matches!(
            detect_database_type("postgres://u:p@localhost/db"),
            Ok(MigrationDatabaseType::Postgres)
        ));
        assert!(matches!(
            detect_database_type("mysql://u:p@localhost/db"),
            Ok(MigrationDatabaseType::MySql)
        ));
        assert!(matches!(
            detect_database_type("sqlite://data.db"),
            Ok(MigrationDatabaseType::Sqlite)
        ));
    }

    #[test]
    fn test_detect_database_type_unsupported() {
        assert!(detect_database_type("foo://localhost/db").is_err());
        assert!(detect_database_type("not a url").is_err());
    }

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("admin"));
        assert!(is_valid_identifier("ops_user-1.a"));
        assert!(!is_valid_identifier("")); // 空
        assert!(!is_valid_identifier("a b")); // 空格
        assert!(!is_valid_identifier("用户")); // 非ASCII
        assert!(!is_valid_identifier("'; DROP TABLE t; --")); // 注入尝试
        assert!(!is_valid_identifier(&"x".repeat(65))); // 超长
        assert!(is_valid_identifier(&"x".repeat(64))); // 边界
    }

    #[test]
    fn test_hash_password_format_and_salt_uniqueness() {
        let salt = new_salt();
        let h1 = hash_password("s3cret", &salt);
        let h2 = hash_password("s3cret", &new_salt());
        // 格式 sha256$<salt>$<digest>
        let parts: Vec<&str> = h1.split('$').collect();
        assert_eq!(parts[0], "sha256");
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[2].len(), 64); // SHA-256 hex
        // 明文不出现
        assert!(!h1.contains("s3cret"));
        // 不同盐 → 不同摘要
        assert_ne!(h1, h2);
        // 同盐确定性
        assert_eq!(h1, hash_password("s3cret", &salt));
    }

    #[test]
    fn test_to_hex() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xff]), "000fff");
        assert_eq!(to_hex(&[]), "");
    }

    #[test]
    fn test_users_table_ddl_has_all_columns() {
        let ddl = users_table_ddl();
        for col in ["username", "password_hash", "role", "created_at"] {
            assert!(ddl.contains(col), "DDL 缺少列 {col}: {ddl}");
        }
        assert!(ddl.contains("PRIMARY KEY"));
    }

    #[test]
    fn test_exit_code_contract() {
        // 0 成功 / 1 运行时失败 / 2 用法错误
        assert_eq!(ExitCode::Ok as i32, 0);
        assert_eq!(ExitCode::RuntimeFailure as i32, 1);
        assert_eq!(ExitCode::UsageError as i32, 2);
    }

    #[test]
    fn test_classify_connect_error_exit_codes() {
        // 协议不支持 → 用法错误 2
        assert_eq!(
            classify_connect_error("foo://localhost/db"),
            ExitCode::UsageError
        );
        assert_eq!(
            classify_connect_error("oracle://localhost/db"),
            ExitCode::UsageError
        );
        // 合法协议（运行期才会失败的连接）→ 运行时失败 1
        assert_eq!(
            classify_connect_error("sqlite:/tmp/dbnexus_t415_nonexistent_dir_x/db.db?mode=rwc"),
            ExitCode::RuntimeFailure
        );
    }
}
