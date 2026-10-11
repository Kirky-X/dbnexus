// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据库迁移示例
//!
//! 演示如何使用 [`MigrationExecutor`] 执行数据库迁移：
//! - 定义 `Migration`（含 `TableChange::CreateTable`）
//! - 创建 `MigrationExecutor` 并应用迁移
//! - 查看迁移历史记录
//! - 手动回滚（执行反向 SQL）
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example migration --features "sqlite,migration"
//! ```

use dbnexus::sea_orm::ConnectionTrait;
use dbnexus::{
    Column, ColumnType, DbConfig, DbPool, Migration, MigrationExecutor, Table, TableChange,
};

/// 从连接池获取一个独立的 sea-orm 连接用于迁移执行器
async fn connect_for_migration(
    url: &str,
) -> Result<sea_orm::DatabaseConnection, Box<dyn std::error::Error>> {
    let conn = sea_orm::Database::connect(url).await?;
    Ok(conn)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔀 DBNexus database migration example");
    println!("========================================\n");

    // 使用 file::memory:?cache=shared 让多个连接共享同一个内存数据库
    let db_url = "sqlite:file:migration_demo?mode=memory&cache=shared";

    // ============================================
    // 创建连接池和 Session
    // ============================================
    let config = DbConfig {
        url: db_url.to_string(),
        admin_role: "admin".to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    println!("✓ Pool created");

    // ============================================
    // 创建迁移执行器
    // ============================================
    // MigrationExecutor 需要一个 sea_orm::DatabaseConnection
    let conn = connect_for_migration(db_url).await?;
    let mut executor = MigrationExecutor::new(conn, dbnexus::foundation::DatabaseType::Sqlite);
    println!("✓ migration executor created");

    // ============================================
    // 定义迁移：创建 users 表
    // ============================================
    let mut migration_v1 = Migration::new(1, "create_users_table".to_string());

    let users_table = Table {
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
                column_type: ColumnType::String(Some(100)),
                is_primary_key: false,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: false,
                comment: None,
            },
            Column {
                name: "email".to_string(),
                column_type: ColumnType::String(Some(200)),
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
    migration_v1.add_table_change(TableChange::CreateTable(users_table));

    println!(
        "\n📋 Migration definition: v{} - {}",
        migration_v1.version, migration_v1.description
    );
    println!(
        "  - table change count: {}",
        migration_v1.table_changes.len()
    );

    // ============================================
    // 应用迁移
    // ============================================
    println!("\nApplying migration v{}...", migration_v1.version);
    executor.apply_migration(&migration_v1).await?;
    println!("✓ migration v{} applied", migration_v1.version);

    // ============================================
    // 定义并应用第二个迁移：创建 posts 表
    // ============================================
    let mut migration_v2 = Migration::new(2, "create_posts_table".to_string());

    let posts_table = Table {
        name: "posts".to_string(),
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
                name: "title".to_string(),
                column_type: ColumnType::Text,
                is_primary_key: false,
                is_nullable: false,
                has_default: false,
                default_value: None,
                is_auto_increment: false,
                comment: None,
            },
            Column {
                name: "user_id".to_string(),
                column_type: ColumnType::Integer,
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
    migration_v2.add_table_change(TableChange::CreateTable(posts_table));

    println!("\nApplying migration v{}...", migration_v2.version);
    executor.apply_migration(&migration_v2).await?;
    println!("✓ migration v{} applied", migration_v2.version);

    // ============================================
    // 查看迁移历史
    // ============================================
    println!("\n📊 Migration history:");
    let history = executor.history();
    println!(
        "  - applied migration count: {}",
        history.applied_migrations.len()
    );

    for record in &history.applied_migrations {
        println!(
            "  - v{}: {} (applied at: {}, file: {})",
            record.version, record.description, record.applied_at, record.file_path
        );
    }

    println!(
        "\n  applied version list: {:?}",
        executor.get_all_versions()
    );
    if let Some(latest) = executor.get_latest_migration() {
        println!(
            "  latest migration: v{} - {}",
            latest.version, latest.description
        );
    }

    // ============================================
    // 验证表已创建
    // ============================================
    let session = pool.get_session("admin").await?;
    let result = session
        .connection()?
        .execute_unprepared("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .await?;
    println!(
        "\n✓ verify table creation: rows_affected = {}",
        result.rows_affected()
    );

    // 插入数据验证表结构正确
    session
        .execute_raw("INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@example.com')")
        .await?;
    session
        .execute_raw("INSERT INTO posts (id, title, user_id) VALUES (1, 'Hello World', 1)")
        .await?;
    println!("✓ data inserted successfully (users + posts tables are both writable)");

    // ============================================
    // 手动回滚演示
    // ============================================
    // MigrationExecutor 没有内置的 rollback API，
    // 回滚需要手动执行反向 SQL（DROP TABLE）。
    // 注意：DdlGuard 出于安全考虑会阻止 DROP TABLE 操作。
    // 生产环境中应通过 MigrationFile 的 DOWN SQL 进行受控回滚，
    // 或在需要 DROP TABLE 时直接使用 sea_orm 连接绕过 Session 层。
    println!("\n🔄 Manual rollback demo:");
    println!("  trying to roll back v2: delete the posts table...");
    match session.execute_raw_ddl("DROP TABLE IF EXISTS posts").await {
        Ok(_) => println!("  ✓ posts table deleted"),
        Err(e) => {
            println!(
                "  ⚠️  DdlGuard blocked the DROP TABLE operation (safety feature): {}",
                e
            );
            println!(
                "     production rollbacks should go through the DOWN SQL of a MigrationFile."
            );
        }
    }

    // 重新加载历史确认状态
    executor.load_history().await?;
    println!(
        "\n📊 Migration history after rollback (records remain in the database, only the table was dropped): {} records",
        executor.history().applied_migrations.len()
    );

    println!("\n========================================");
    println!("✨ Database migration example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - Migration::new(version, description) create a migration definition");
    println!("  - migration.add_table_change(TableChange::CreateTable(table)) add a table change");
    println!("  - MigrationExecutor::new(conn, db_type) create an executor");
    println!("  - executor.apply_migration(&migration) apply a migration (transactional)");
    println!("  - executor.history() view the applied migration records");
    println!("  - rollback requires running the reverse SQL manually (e.g. DROP TABLE)");

    Ok(())
}
