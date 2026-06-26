// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

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

use dbnexus::{
    Column, ColumnType, DbConfig, DbPool, Migration, MigrationExecutor, Table, TableChange,
};
use sea_orm::ConnectionTrait;

/// 从连接池获取一个独立的 sea-orm 连接用于迁移执行器
async fn connect_for_migration(url: &str) -> Result<sea_orm::DatabaseConnection, Box<dyn std::error::Error>> {
    let conn = sea_orm::Database::connect(url).await?;
    Ok(conn)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔀 DBNexus 数据库迁移示例");
    println!("========================================\n");

    // 使用 file::memory:?cache=shared 让多个连接共享同一个内存数据库
    let db_url = "sqlite:file:migration_demo?mode=memory&cache=shared";

    // ============================================
    // 1. 创建连接池和 Session
    // ============================================
    let config = DbConfig {
        url: db_url.to_string(),
        admin_role: "admin".to_string(),
        max_connections: 5,
        min_connections: 1,
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // ============================================
    // 2. 创建迁移执行器
    // ============================================
    // MigrationExecutor 需要一个 sea_orm::DatabaseConnection
    let conn = connect_for_migration(db_url).await?;
    let mut executor = MigrationExecutor::new(conn, dbnexus::foundation::config::DatabaseType::Sqlite);
    println!("✓ 迁移执行器创建成功");

    // ============================================
    // 3. 定义迁移：创建 users 表
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

    println!("\n📋 迁移定义: v{} - {}", migration_v1.version, migration_v1.description);
    println!("  - 表变更数量: {}", migration_v1.table_changes.len());

    // ============================================
    // 4. 应用迁移
    // ============================================
    println!("\n应用迁移 v{}...", migration_v1.version);
    executor.apply_migration(&migration_v1).await?;
    println!("✓ 迁移 v{} 应用成功", migration_v1.version);

    // ============================================
    // 5. 定义并应用第二个迁移：创建 posts 表
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

    println!("\n应用迁移 v{}...", migration_v2.version);
    executor.apply_migration(&migration_v2).await?;
    println!("✓ 迁移 v{} 应用成功", migration_v2.version);

    // ============================================
    // 6. 查看迁移历史
    // ============================================
    println!("\n📊 迁移历史记录:");
    let history = executor.history();
    println!("  - 已应用迁移数量: {}", history.applied_migrations.len());

    for record in &history.applied_migrations {
        println!(
            "  - v{}: {} (应用时间: {}, 文件: {})",
            record.version, record.description, record.applied_at, record.file_path
        );
    }

    println!("\n  已应用版本列表: {:?}", executor.get_all_versions());
    if let Some(latest) = executor.get_latest_migration() {
        println!("  最新迁移: v{} - {}", latest.version, latest.description);
    }

    // ============================================
    // 7. 验证表已创建
    // ============================================
    let session = pool.get_session("admin").await?;
    let result = session
        .connection()?
        .execute_unprepared("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .await?;
    println!("\n✓ 验证表创建: rows_affected = {}", result.rows_affected());

    // 插入数据验证表结构正确
    session
        .execute_raw("INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@example.com')")
        .await?;
    session
        .execute_raw("INSERT INTO posts (id, title, user_id) VALUES (1, 'Hello World', 1)")
        .await?;
    println!("✓ 数据插入成功（users + posts 表均可写入）");

    // ============================================
    // 8. 手动回滚演示
    // ============================================
    // MigrationExecutor 没有内置的 rollback API，
    // 回滚需要手动执行反向 SQL（DROP TABLE）。
    // 注意：DdlGuard 出于安全考虑会阻止 DROP TABLE 操作。
    // 生产环境中应通过 MigrationFile 的 DOWN SQL 进行受控回滚，
    // 或在需要 DROP TABLE 时直接使用 sea_orm 连接绕过 Session 层。
    println!("\n🔄 手动回滚演示:");
    println!("  尝试回滚 v2: 删除 posts 表...");
    match session.execute_raw_ddl("DROP TABLE IF EXISTS posts").await {
        Ok(_) => println!("  ✓ posts 表已删除"),
        Err(e) => {
            println!("  ⚠️  DdlGuard 阻止了 DROP TABLE 操作（安全特性）: {}", e);
            println!("     生产环境应通过 MigrationFile 的 DOWN SQL 进行受控回滚。");
        }
    }

    // 重新加载历史确认状态
    executor.load_history().await?;
    println!(
        "\n📊 回滚后迁移历史（数据库中的记录仍保留，仅表被删除）: {} 条记录",
        executor.history().applied_migrations.len()
    );

    println!("\n========================================");
    println!("✨ 数据库迁移示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - Migration::new(version, description) 创建迁移定义");
    println!("  - migration.add_table_change(TableChange::CreateTable(table)) 添加表变更");
    println!("  - MigrationExecutor::new(conn, db_type) 创建执行器");
    println!("  - executor.apply_migration(&migration) 应用迁移（事务性）");
    println!("  - executor.history() 查看已应用迁移记录");
    println!("  - 回滚需手动执行反向 SQL（DROP TABLE 等）");

    Ok(())
}
