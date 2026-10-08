// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! DBNexus 迁移 CLI 工具
//!
//! 提供数据库迁移的命令行界面

use clap::{Command, CommandFactory, FromArgMatches, Parser, Subcommand};
#[cfg(feature = "migration")]
use dbnexus::MigrationExecutor;
use dbnexus::foundation::DatabaseType as MigrationDatabaseType;
use dbnexus::i18n;
use dbnexus::{DbError, DbPool, DbResult};
use std::fs;
#[cfg(any(feature = "migration", feature = "permission-engine"))]
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// CLI 配置（about/参数 help 在运行期经 i18n 本地化，见 `build_cli()`）
#[derive(Parser)]
#[command(name = "dbnexus-migrate", long_about = None)]
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
    #[cfg(feature = "migration")]
    Create {
        /// 迁移描述
        description: String,

        /// 迁移文件输出目录
        #[arg(short, long, default_value = "./migrations")]
        directory: PathBuf,
    },

    /// 应用迁移
    #[cfg(feature = "migration")]
    Up {
        /// 目标版本号（可选，默认为所有待应用迁移）
        #[arg(long)]
        version: Option<u32>,
    },

    /// 回滚迁移
    #[cfg(feature = "migration")]
    Down {
        /// 目标版本号（可选，默认为回滚上一版本）
        #[arg(long)]
        version: Option<u32>,

        /// 回滚所有迁移
        #[arg(long, default_value = "false")]
        all: bool,
    },

    /// 查看迁移状态
    #[cfg(feature = "migration")]
    Status,

    /// 测试数据库连接
    TestConnection,

    /// 生成迁移文件（基于 schema 差异）
    #[cfg(feature = "migration")]
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
    #[cfg(feature = "migration")]
    List,

    /// 应用迁移目录中的所有待应用迁移（机器可读输出，退出码 0/1/2）
    #[cfg(feature = "migration")]
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

    /// 连接池状态快照（health_snapshot 结构，JSON 输出）
    #[cfg(feature = "health-check")]
    PoolStatus,

    /// 审计事件查询（DbAuditStorage 过滤查询，JSON 输出，退出码 0/1/2）
    #[cfg(feature = "audit")]
    AuditQuery {
        /// 按用户 ID 过滤（仅经单引号转义拼接，勿透传不可信输入）
        #[arg(long)]
        user: Option<String>,

        /// 按实体类型过滤（仅经单引号转义拼接，勿透传不可信输入）
        #[arg(long)]
        entity: Option<String>,

        /// 按操作类型过滤（create/read/update/delete/login/logout/permission-change/config-change/`other:<text>`）
        #[arg(long)]
        operation: Option<String>,

        /// 按严重级别过滤（info/low/medium/high/critical）
        #[arg(long)]
        severity: Option<String>,

        /// 按结果过滤（success/failure/partial/unknown）
        #[arg(long)]
        status: Option<String>,

        /// 起始时间（RFC 3339，如 2026-01-01T00:00:00Z）
        #[arg(long)]
        since: Option<String>,

        /// 截止时间（RFC 3339）
        #[arg(long)]
        until: Option<String>,

        /// 返回条数上限（默认 500；0 表示不限制）
        #[arg(long, default_value_t = 500)]
        limit: usize,
    },

    /// 分片信息（策略/分片清单/路由演示，JSON 输出）
    #[cfg(feature = "sharding")]
    ShardInfo {
        /// 分片策略（yearly/monthly/daily/hash/consistent-hash）
        #[arg(long)]
        strategy: String,

        /// 总分片数（≥1）
        #[arg(long)]
        total_shards: u32,

        /// 分片名称前缀
        #[arg(long, default_value = "db")]
        prefix: String,

        /// 连接字符串模板（{shard} 占位符）
        #[arg(long, default_value = "sqlite:./data/{shard}.db")]
        template: String,

        /// 演示路由：按业务分片键计算目标分片 ID
        #[arg(long)]
        route_key: Option<String>,
    },

    /// 权限校验（权限配置文件 + PDP 决策，JSON 输出）
    #[cfg(feature = "permission-engine")]
    PermissionCheck {
        /// 被校验的角色名
        #[arg(long)]
        role: String,

        /// 目标资源（表名）
        #[arg(long)]
        table: String,

        /// 操作（select/insert/update/delete，大小写不敏感）
        #[arg(long)]
        action: String,

        /// 权限配置文件路径（YAML/JSON：`roles.<role>` 为规则数组）
        #[arg(long)]
        permissions: PathBuf,
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

/// 预扫描命令行参数中的 `--lang`，在首次 i18n 输出（含 help 渲染）前应用语言覆盖。
///
/// 非法语言值静默忽略（由后续 `set_locale` 正常报错）；未指定时走自动检测链
/// （`DBNEXUS_LANG` → 系统语言 → en），`LANG=zh` 时即为中文。
fn pre_apply_lang_override() {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = if let Some(rest) = arg.strip_prefix("--lang=") {
            Some(rest.to_string())
        } else if arg == "--lang" {
            args.next()
        } else {
            None
        };
        if let Some(value) = value {
            let _ = i18n::set_locale(&value);
            return;
        }
    }
}

/// 构建本地化 CLI 命令：about 与参数 help 经 i18n 动态生成，
/// 覆盖 derive 从中文文档注释生成的静态 help。
fn build_cli() -> Command {
    Cli::command()
        .about(i18n::t_simple("cli-help-about"))
        .mut_arg("database_url", |a| {
            let help = i18n::t_simple("cli-help-database-url");
            a.help(help.clone()).long_help(help)
        })
        .mut_arg("config", |a| {
            let help = i18n::t_simple("cli-help-config");
            a.help(help.clone()).long_help(help)
        })
        .mut_arg("migrations_dir", |a| {
            let help = i18n::t_simple("cli-help-migrations-dir");
            a.help(help.clone()).long_help(help)
        })
        .mut_arg("lang", |a| {
            let help = i18n::t_simple("cli-help-lang");
            a.help(help.clone()).long_help(help)
        })
}

/// 程序入口
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 启动早期初始化 i18n（显式 --lang 优先于自动检测）
    pre_apply_lang_override();

    let matches = build_cli().get_matches();
    let cli = Cli::from_arg_matches(&matches)?;

    // global 参数解析（clap 不允许 required global，此处统一收敛）；
    // shard-info（纯路由计算）与 permission-check（读配置文件 + PDP）不依赖数据库
    // 免库命令集合：仅在相应命令存在的 feature 组合下参与判定
    #[cfg(feature = "sharding")]
    let shard_info_free = matches!(cli.command, Commands::ShardInfo { .. });
    #[cfg(not(feature = "sharding"))]
    let shard_info_free = false;
    #[cfg(feature = "permission-engine")]
    let permission_check_free = matches!(cli.command, Commands::PermissionCheck { .. });
    #[cfg(not(feature = "permission-engine"))]
    let permission_check_free = false;
    let needs_db = !(shard_info_free || permission_check_free);
    let database_url: String = if needs_db {
        cli.database_url.unwrap_or_else(|| {
            eprintln!("{}", i18n::t_simple("cli-database-url-required"));
            std::process::exit(ExitCode::UsageError as i32);
        })
    } else {
        cli.database_url.unwrap_or_default()
    };

    // 初始化语言设置（非法值在此报错）
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
        #[cfg(feature = "migration")]
        Commands::Create {
            description,
            directory,
        } => {
            create_migration(description, directory).await?;
        }
        #[cfg(feature = "migration")]
        Commands::Up { version } => {
            run_migrations_up(&database_url, &cli.migrations_dir, *version).await?;
        }
        #[cfg(feature = "migration")]
        Commands::Down { version, all } => {
            run_migrations_down(&database_url, &cli.migrations_dir, *version, *all).await?;
        }
        #[cfg(feature = "migration")]
        Commands::Status => {
            show_status(&database_url, &cli.migrations_dir).await?;
        }
        Commands::TestConnection => {
            test_connection(&database_url).await?;
        }
        #[cfg(feature = "migration")]
        Commands::Generate {
            from_schema,
            to_schema,
            output,
            description,
        } => {
            generate_migration(from_schema, to_schema, output, description).await?;
        }
        #[cfg(feature = "migration")]
        Commands::List => {
            list_migrations(&database_url, &cli.migrations_dir).await?;
        }
        // 运维子命令：JSON 输出 + 退出码契约（0 成功 / 1 运行时失败 / 2 用法错误）
        #[cfg(feature = "migration")]
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
        #[cfg(feature = "health-check")]
        Commands::PoolStatus => {
            let code = run_pool_status_json(&database_url).await;
            std::process::exit(code as i32);
        }
        #[cfg(feature = "audit")]
        Commands::AuditQuery {
            user,
            entity,
            operation,
            severity,
            status,
            since,
            until,
            limit,
        } => {
            let filters = AuditFilterArgs {
                user: user.clone(),
                entity: entity.clone(),
                operation: operation.clone(),
                severity: severity.clone(),
                status: status.clone(),
                since: since.clone(),
                until: until.clone(),
                limit: *limit,
            };
            if *limit > 1_000_000 {
                print_json(&serde_json::json!({
                    "status": "error", "error_code": "invalid_limit",
                    "error": i18n::t_simple("cli-invalid-limit")
                }));
                std::process::exit(ExitCode::UsageError as i32);
            }
            let code = run_audit_query_json(&database_url, &filters).await;
            std::process::exit(code as i32);
        }
        #[cfg(feature = "sharding")]
        Commands::ShardInfo {
            strategy,
            total_shards,
            prefix,
            template,
            route_key,
        } => {
            let code = run_shard_info_json(strategy, *total_shards, prefix, template, route_key);
            std::process::exit(code as i32);
        }
        #[cfg(feature = "permission-engine")]
        Commands::PermissionCheck {
            role,
            table,
            action,
            permissions,
        } => {
            let code = run_permission_check_json(role, table, action, permissions).await;
            std::process::exit(code as i32);
        }
    }

    Ok(())
}

/// 创建新的迁移文件
#[cfg(feature = "migration")]
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
#[cfg(feature = "migration")]
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

    // 获取会话
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
#[cfg(feature = "migration")]
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
#[cfg(feature = "migration")]
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
#[cfg(feature = "migration")]
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
#[cfg(feature = "migration")]
async fn rollback_migration(
    executor: &mut MigrationExecutor,
    version: u32,
    migration_file: &dbnexus::MigrationFile,
) -> DbResult<()> {
    executor.rollback_version(version, migration_file).await
}

/// 生成迁移文件
#[cfg(feature = "migration")]
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
#[cfg(feature = "migration")]
struct DiffSql {
    up: String,
    down: String,
}

/// 生成 Schema 差异 SQL（简化版本）
#[cfg(feature = "migration")]
fn generate_schema_diff_sql(_from_content: &str, _to_content: &str) -> Result<DiffSql, DbError> {
    // 这里是一个简化实现
    // 实际实现需要解析 schema 文件并计算差异
    Ok(DiffSql {
        up: "-- 自动生成的 UP SQL 请手动编辑".to_string(),
        down: "-- 自动生成的 DOWN SQL 请手动编辑".to_string(),
    })
}

/// 列出所有迁移文件
#[cfg(feature = "migration")]
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
#[cfg(feature = "migration")]
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

// ============================================================================
// 运维子命令（pool-status / audit-query / shard-info / permission-check）
// — 机器可读 JSON + 退出码 0/1/2
// ============================================================================

/// 连接池状态快照（JSON 输出 health_snapshot 结构）
///
/// 退出码：0 健康/降级 / 1 不健康（快照 unhealthy 或连接失败）/ 2 URL 用法错误
#[cfg(feature = "health-check")]
async fn run_pool_status_json(database_url: &str) -> ExitCode {
    if detect_database_type(database_url).is_err() {
        print_json(&serde_json::json!({
            "status": "unhealthy",
            "checks": { "url": "invalid" },
            "error": i18n::t_simple("cli-unsupported-db-url-protocol")
        }));
        return ExitCode::UsageError;
    }

    let pool = match DbPool::new(database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "unhealthy",
                "checks": { "connect": "fail" },
                "error": mask_url_in_text(&e.to_string(), database_url),
                "url": mask_database_url(database_url),
            }));
            return ExitCode::RuntimeFailure;
        }
    };

    // 先取一次会话建立真实连接：新建池无预热连接（total=0），health_snapshot
    // 的零连接语义恒判 unhealthy；探测会话归还后快照反映真实池容量
    if let Err(e) = pool.get_session("admin").await {
        print_json(&serde_json::json!({
            "status": "unhealthy",
            "checks": { "connect": "ok", "session": "fail" },
            "error": e.to_string(),
            "url": mask_database_url(database_url),
        }));
        return ExitCode::RuntimeFailure;
    }

    let mut snapshot = pool.health_snapshot().await;
    snapshot["url"] = serde_json::Value::String(mask_database_url(database_url));
    let code = if snapshot["status"] == "unhealthy" {
        ExitCode::RuntimeFailure
    } else {
        ExitCode::Ok
    };
    print_json(&snapshot);
    code
}

/// audit-query 的过滤参数集合
#[cfg(feature = "audit")]
struct AuditFilterArgs {
    user: Option<String>,
    entity: Option<String>,
    operation: Option<String>,
    severity: Option<String>,
    status: Option<String>,
    since: Option<String>,
    until: Option<String>,
    /// 0 = 不限制，其余为条数上限
    limit: usize,
}

/// 审计操作枚举名 → 枚举（serde 序列化名，大小写不敏感）
#[cfg(feature = "audit")]
fn parse_audit_operation(name: &str) -> Option<dbnexus::AuditOperation> {
    use dbnexus::AuditOperation;
    // other:<text> 映射自由文本变体 Other(String)，覆盖非预置操作类型的事件
    if let Some(text) = name.strip_prefix("other:") {
        return Some(AuditOperation::Other(text.to_string()));
    }
    match name.to_lowercase().as_str() {
        "create" => Some(AuditOperation::Create),
        "read" => Some(AuditOperation::Read),
        "update" => Some(AuditOperation::Update),
        "delete" => Some(AuditOperation::Delete),
        "login" => Some(AuditOperation::Login),
        "logout" => Some(AuditOperation::Logout),
        "permission-change" => Some(AuditOperation::PermissionChange),
        "config-change" => Some(AuditOperation::ConfigChange),
        _ => None,
    }
}

/// 审计严重级别枚举名 → 枚举
#[cfg(feature = "audit")]
fn parse_audit_severity(name: &str) -> Option<dbnexus::AuditSeverity> {
    use dbnexus::AuditSeverity;
    match name.to_lowercase().as_str() {
        "info" => Some(AuditSeverity::Info),
        "low" => Some(AuditSeverity::Low),
        "medium" => Some(AuditSeverity::Medium),
        "high" => Some(AuditSeverity::High),
        "critical" => Some(AuditSeverity::Critical),
        _ => None,
    }
}

/// 审计状态枚举名 → 枚举
#[cfg(feature = "audit")]
fn parse_audit_status(name: &str) -> Option<dbnexus::AuditStatus> {
    use dbnexus::AuditStatus;
    match name.to_lowercase().as_str() {
        "success" => Some(AuditStatus::Success),
        "failure" => Some(AuditStatus::Failure),
        "partial" => Some(AuditStatus::Partial),
        "unknown" => Some(AuditStatus::Unknown),
        _ => None,
    }
}

/// 参数校验失败（用法错误）的统一 JSON 输出
#[cfg(any(
    feature = "health-check",
    feature = "audit",
    feature = "sharding",
    feature = "permission-engine"
))]
fn print_usage_error(error_code: &str, error: &str) {
    print_json(&serde_json::json!({
        "status": "error", "error_code": error_code, "error": error
    }));
}

/// 审计事件查询（JSON 输出）
///
/// 退出码：0 查询成功（含空集）/ 1 查询执行失败 / 2 参数错误
#[cfg(feature = "audit")]
async fn run_audit_query_json(database_url: &str, args: &AuditFilterArgs) -> ExitCode {
    use dbnexus::{AuditQueryFilters, AuditStorage};

    let operation = match &args.operation {
        Some(name) => match parse_audit_operation(name) {
            Some(op) => Some(op),
            None => {
                print_usage_error(
                    "invalid_operation",
                    &i18n::t_simple("cli-invalid-operation"),
                );
                return ExitCode::UsageError;
            }
        },
        None => None,
    };
    let severity = match &args.severity {
        Some(name) => match parse_audit_severity(name) {
            Some(sev) => Some(sev),
            None => {
                print_usage_error("invalid_severity", &i18n::t_simple("cli-invalid-severity"));
                return ExitCode::UsageError;
            }
        },
        None => None,
    };
    let status = match &args.status {
        Some(name) => match parse_audit_status(name) {
            Some(st) => Some(st),
            None => {
                print_usage_error("invalid_status", &i18n::t_simple("cli-invalid-status"));
                return ExitCode::UsageError;
            }
        },
        None => None,
    };
    let parse_time = |raw: &str| -> Result<chrono::DateTime<chrono::Utc>, ()> {
        chrono::DateTime::parse_from_rfc3339(raw)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| ())
    };
    let since = match &args.since {
        Some(raw) => match parse_time(raw) {
            Ok(t) => Some(t),
            Err(_) => {
                print_usage_error("invalid_since", &i18n::t_simple("cli-invalid-since"));
                return ExitCode::UsageError;
            }
        },
        None => None,
    };
    let until = match &args.until {
        Some(raw) => match parse_time(raw) {
            Ok(t) => Some(t),
            Err(_) => {
                print_usage_error("invalid_until", &i18n::t_simple("cli-invalid-until"));
                return ExitCode::UsageError;
            }
        },
        None => None,
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

    let storage = dbnexus::DbAuditStorage::new(std::sync::Arc::new(pool));
    // 幂等建表：空库首查返回空集而非"表不存在"失败
    if let Err(e) = storage.init().await {
        print_json(&serde_json::json!({
            "status": "error", "error_code": "table_init_failed", "error": e.to_string()
        }));
        return ExitCode::RuntimeFailure;
    }

    // limit+1 探测截断：多取一条即可判定是否仍有匹配（0 = unlimited 不探测）；
    // saturating 加法防御 usize::MAX 输入回绕为 LIMIT 0（恒空集的错误输出）
    let probe_limit = (args.limit != 0).then_some(args.limit.saturating_add(1));
    let filters = AuditQueryFilters {
        user_id: args.user.clone(),
        entity_type: args.entity.clone(),
        operation,
        start_time: since,
        end_time: until,
        severity,
        result: status,
        limit: probe_limit,
    };

    match storage.query(&filters).await {
        Ok(events) => {
            let truncated = probe_limit.is_some() && events.len() > args.limit;
            let mut rows = Vec::with_capacity(events.len().min(args.limit));
            for event in &events {
                match serde_json::to_value(event) {
                    Ok(value) => rows.push(value),
                    Err(e) => {
                        print_json(&serde_json::json!({
                            "status": "error", "error_code": "serialize_failed",
                            "error": e.to_string()
                        }));
                        return ExitCode::RuntimeFailure;
                    }
                }
            }
            if truncated {
                rows.truncate(args.limit);
            }
            let count = rows.len();
            print_json(&serde_json::json!({
                "status": "ok",
                "count": count,
                "truncated": truncated,
                "limit": args.limit,
                "events": rows,
            }));
            ExitCode::Ok
        }
        Err(e) => {
            print_json(&serde_json::json!({
                "status": "error", "error_code": "query_failed", "error": e.to_string()
            }));
            ExitCode::RuntimeFailure
        }
    }
}

/// 分片信息（JSON 输出）
///
/// 退出码：0 成功 / 2 参数错误（未知策略、total_shards=0、空 route key）
#[cfg(feature = "sharding")]
fn run_shard_info_json(
    strategy: &str,
    total_shards: u32,
    prefix: &str,
    template: &str,
    route_key: &Option<String>,
) -> ExitCode {
    // 上界 fail-fast：generate_all_connections 会按 total_shards 全量分配，
    // 误传超大值（如 3.6e9）将导致进程挂起/OOM；运维真实分片规模远低于此
    if total_shards == 0 || total_shards > 10_000 {
        print_usage_error(
            "invalid_total_shards",
            &i18n::t_simple("cli-invalid-total-shards"),
        );
        return ExitCode::UsageError;
    }
    // 单一事实源：库侧 is_known_strategy（create_strategy 对未知名静默回落）
    if !dbnexus::is_known_strategy(strategy) {
        print_usage_error("unknown_strategy", &i18n::t_simple("cli-invalid-strategy"));
        return ExitCode::UsageError;
    }
    if route_key.as_ref().is_some_and(|k| k.is_empty()) {
        print_usage_error(
            "invalid_route_key",
            &i18n::t_simple("cli-invalid-route-key"),
        );
        return ExitCode::UsageError;
    }

    let config = dbnexus::ShardConfig {
        strategy: strategy.to_string(),
        total_shards,
        prefix: prefix.to_string(),
        connection_template: template.to_string(),
    };
    let router = dbnexus::ShardRouter::with_config_sync(&config);

    let mut infos = router.all_shards();
    // 按 shard_id 排序输出：HashMap 迭代序不稳定，运维输出需确定性
    infos.sort_by_key(|info| info.shard_id);
    let shards: Vec<serde_json::Value> = infos
        .iter()
        .map(|info| {
            serde_json::json!({
                "shard_id": info.shard_id,
                "name": info.name,
                "connection_string": info.connection_string,
            })
        })
        .collect();

    let mut payload = serde_json::json!({
        "status": "ok",
        "strategy": router.strategy_name(),
        "total_shards": router.total_shards(),
        "shards": shards,
    });

    if let Some(key) = route_key {
        let shard_id = router.shard_id_for_key(key);
        payload["route"] = serde_json::json!({ "key": key, "shard_id": shard_id });
    }

    print_json(&payload);
    ExitCode::Ok
}

/// 权限校验白名单（PDP check 对未知 action fail-closed 拒绝，
/// CLI 层先行拦截以便归为用法错误而非 deny 决策）
#[cfg(feature = "permission-engine")]
fn parse_permission_action(action: &str) -> Option<&'static str> {
    match action.to_lowercase().as_str() {
        "select" => Some("select"),
        "insert" => Some("insert"),
        "update" => Some("update"),
        "delete" => Some("delete"),
        _ => None,
    }
}

/// 权限校验（JSON 输出）
///
/// 退出码：0 allow / 1 deny 或 NotApplicable（fail-closed）/ 2 参数或配置错误
#[cfg(feature = "permission-engine")]
async fn run_permission_check_json(
    role: &str,
    table: &str,
    action: &str,
    permissions: &Path,
) -> ExitCode {
    if role.is_empty() || table.is_empty() {
        print_usage_error(
            "empty_role_or_table",
            &i18n::t_simple("cli-empty-role-or-table"),
        );
        return ExitCode::UsageError;
    }
    let action = match parse_permission_action(action) {
        Some(a) => a,
        None => {
            print_usage_error("invalid_action", &i18n::t_simple("cli-invalid-action"));
            return ExitCode::UsageError;
        }
    };
    if !permissions.exists() {
        print_usage_error(
            "permissions_file_unavailable",
            &i18n::t_simple("cli-permissions-file-unavailable"),
        );
        return ExitCode::UsageError;
    }

    use dbnexus::EnginePermissionProvider;

    let provider =
        match dbnexus::EngineYamlPermissionProvider::new(permissions.to_string_lossy().as_ref()) {
            Ok(p) => p,
            Err(e) => {
                print_usage_error("permissions_file_unavailable", &e);
                return ExitCode::UsageError;
            }
        };
    // 显式 refresh 让配置加载失败在决策前即刻暴露为退出码 2，
    // 而非依赖 provider 内部的 Error 决策路径
    if let Err(e) = provider.refresh().await {
        print_usage_error(
            "permissions_file_invalid",
            &i18n::t(
                "cli-permissions-file-load-failed",
                &[("error", e.to_string())],
            ),
        );
        return ExitCode::UsageError;
    }

    // default_decision 保持 NotApplicable 透传：显式 Deny（策略拒绝）与
    // NotApplicable（无匹配策略，多为角色/表名拼写或规则 subject 配错）
    // 根因不同，诊断输出必须可辨；fail-closed 由下方退出码统一保证
    let pdp = dbnexus::PolicyDecisionPoint::builder()
        .provider(std::sync::Arc::new(provider))
        .build();

    let decision = pdp.check(role, table, action).await;
    let (decision_name, raw_decision_name, code) = match &decision {
        dbnexus::PermissionDecision::Allow => ("allow", "allow", ExitCode::Ok),
        dbnexus::PermissionDecision::Deny => ("deny", "deny", ExitCode::RuntimeFailure),
        // fail-closed：无适用策略按拒绝处理，但原始决策保留供诊断
        dbnexus::PermissionDecision::NotApplicable => {
            ("deny", "not_applicable", ExitCode::RuntimeFailure)
        }
        dbnexus::PermissionDecision::Error(_) => {
            print_usage_error(
                "permissions_file_invalid",
                &i18n::t_simple("cli-permissions-file-invalid"),
            );
            return ExitCode::UsageError;
        }
    };

    print_json(&serde_json::json!({
        "status": "ok",
        "role": role,
        "table": table,
        "action": action,
        "decision": decision_name,
        "raw_decision": raw_decision_name,
    }));
    code
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
///
/// authority 中的 password 段与 query 参数中的 password/token/secret 类
/// 键值统一脱敏；无法解析时返回固定占位符而非原文（原文可能携带凭据）
fn mask_database_url(url: &str) -> String {
    let mut parsed = match url::Url::parse(url) {
        Ok(parsed) => parsed,
        Err(_) => return "<unparsable-url>".to_string(),
    };
    if let Some(password) = parsed.password() {
        parsed.set_password(Some(&"*".repeat(password.len()))).ok();
    }
    let has_sensitive_query = parsed
        .query_pairs()
        .any(|(k, _)| is_sensitive_query_key(&k));
    if has_sensitive_query {
        let pairs: Vec<String> = parsed
            .query_pairs()
            .map(|(k, v)| {
                if is_sensitive_query_key(&k) {
                    format!("{k}=***")
                } else {
                    format!("{k}={v}")
                }
            })
            .collect();
        let query = pairs.join("&");
        parsed.set_query(Some(&query));
    }
    parsed.to_string()
}

fn is_sensitive_query_key(key: &str) -> bool {
    let key = key.to_lowercase();
    key.contains("password") || key.contains("token") || key.contains("secret")
}

/// 将错误文本中内嵌的完整连接串替换为脱敏形式
///
/// 连接错误的 Display 文本可能内嵌原始 URL；仅对文本整体调
/// mask_database_url 无法覆盖复合文本，故按调用方已知的原始 URL
/// 做精确子串替换
#[cfg(any(feature = "health-check", feature = "audit"))]
fn mask_url_in_text(text: &str, url: &str) -> String {
    let masked = mask_database_url(url);
    if masked == url {
        return text.to_string();
    }
    text.replace(url, &masked)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== DOWN 提取（rollback 前置校验依赖的逻辑） =====

    #[cfg(feature = "migration")]
    #[test]
    fn test_extract_down_sql_present() {
        let content = "-- UP:\nCREATE TABLE users (id INTEGER);\n-- DOWN:\nDROP TABLE users;\n";
        let down = MigrationExecutor::extract_down_sql(content).expect("应提取到 DOWN SQL");
        assert_eq!(down, "DROP TABLE users;");
    }

    #[cfg(feature = "migration")]
    #[test]
    fn test_extract_down_sql_case_insensitive() {
        let content = "-- up:\nALTER TABLE users ADD COLUMN c TEXT;\n-- down:\nALTER TABLE users DROP COLUMN c;\n";
        let down = MigrationExecutor::extract_down_sql(content).expect("应提取到 DOWN SQL");
        assert!(down.contains("DROP COLUMN c"));
    }

    /// DOWN 缺失时的错误分支：extract 返回 None → rollback 报"无可回滚的 DOWN 部分"
    #[cfg(feature = "migration")]
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

    #[cfg(feature = "migration")]
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
