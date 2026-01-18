// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 数据库迁移示例
//!
//! 展示如何使用 dbnexus 的数据库迁移功能：
//! - 创建迁移文件
//! - 应用数据库迁移
//! - 回滚迁移
//! - 查看迁移历史
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example migration --features "sqlite,migration"
//! ```

use dbnexus::{
    DbPool,
    config::DatabaseType,
    migration::{Column, ColumnType, ForeignKey, ForeignKeyAction, Index, Migration, MigrationExecutor, Table, TableChange},
};
use sea_orm::Database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🗄️  DBNexus 数据库迁移示例\n");
    println!("========================================");

    // 1. 初始化数据库连接池
    println!("\n1️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let _pool = DbPool::new("sqlite::memory:").await?;
    println!("✓ 连接池创建成功");

    // 2. 创建迁移执行器
    println!("\n2️⃣ 创建迁移执行器");
    println!("------------------------------------------");
    let connection = Database::connect("sqlite::memory:").await?;
    let mut executor = MigrationExecutor::new(connection, DatabaseType::Sqlite);
    println!("✓ 迁移执行器创建成功");

    // 3. 加载迁移历史
    println!("\n3️⃣ 加载迁移历史");
    println!("------------------------------------------");
    executor.load_history().await?;
    println!("✓ 迁移历史加载完成");
    println!("  📋 已应用的迁移数: {}", executor.history().applied_migrations.len());

    // 4. 创建第一个迁移：创建 users 表
    println!("\n4️⃣ 创建迁移 V1：创建 users 表");
    println!("------------------------------------------");

    let migration_v1 = Migration {
        version: 1,
        description: "Create users table".to_string(),
        table_changes: vec![TableChange::CreateTable(Table {
            name: "users".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    column_type: ColumnType::Integer,
                    is_nullable: false,
                    is_primary_key: true,
                    is_auto_increment: true,
                    has_default: false,
                    default_value: None,
                    comment: None,
                },
                Column {
                    name: "name".to_string(),
                    column_type: ColumnType::Text,
                    is_nullable: false,
                    is_primary_key: false,
                    is_auto_increment: false,
                    has_default: false,
                    default_value: None,
                    comment: None,
                },
                Column {
                    name: "email".to_string(),
                    column_type: ColumnType::Text,
                    is_nullable: false,
                    is_primary_key: false,
                    is_auto_increment: false,
                    has_default: false,
                    default_value: None,
                    comment: None,
                },
                Column {
                    name: "created_at".to_string(),
                    column_type: ColumnType::Timestamp,
                    is_nullable: false,
                    is_primary_key: false,
                    is_auto_increment: false,
                    has_default: true,
                    default_value: Some("CURRENT_TIMESTAMP".to_string()),
                    comment: None,
                },
            ],
            primary_key_columns: vec![],
            indexes: vec![
                Index {
                    name: "idx_users_email".to_string(),
                    table_name: "users".to_string(),
                    columns: vec!["email".to_string()],
                    is_unique: true,
                    is_constraint: true,
                },
            ],
            foreign_keys: vec![],
            comment: Some("用户表".to_string()),
        })],
        sql: None,
        timestamp: Some(time::OffsetDateTime::now_utc()),
    };

    println!("  ✓ 迁移 V1 创建成功");
    println!("    - 版本: {}", migration_v1.version);
    println!("    - 描述: {}", migration_v1.description);

    // 5. 创建第二个迁移：创建 posts 表
    println!("\n5️⃣ 创建迁移 V2：创建 posts 表");
    println!("------------------------------------------");

    let migration_v2 = Migration {
        version: 2,
        description: "Create posts table".to_string(),
        table_changes: vec![TableChange::CreateTable(Table {
            name: "posts".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    column_type: ColumnType::Integer,
                    is_nullable: false,
                    is_primary_key: true,
                    is_auto_increment: true,
                    has_default: false,
                    default_value: None,
                    comment: None,
                },
                Column {
                    name: "title".to_string(),
                    column_type: ColumnType::Text,
                    is_nullable: false,
                    is_primary_key: false,
                    is_auto_increment: false,
                    has_default: false,
                    default_value: None,
                    comment: None,
                },
                Column {
                    name: "content".to_string(),
                    column_type: ColumnType::Text,
                    is_nullable: true,
                    is_primary_key: false,
                    is_auto_increment: false,
                    has_default: false,
                    default_value: None,
                    comment: None,
                },
                Column {
                    name: "user_id".to_string(),
                    column_type: ColumnType::Integer,
                    is_nullable: false,
                    is_primary_key: false,
                    is_auto_increment: false,
                    has_default: false,
                    default_value: None,
                    comment: None,
                },
                Column {
                    name: "created_at".to_string(),
                    column_type: ColumnType::Timestamp,
                    is_nullable: false,
                    is_primary_key: false,
                    is_auto_increment: false,
                    has_default: true,
                    default_value: Some("CURRENT_TIMESTAMP".to_string()),
                    comment: None,
                },
            ],
            primary_key_columns: vec![],
            indexes: vec![
                Index {
                    name: "idx_posts_user_id".to_string(),
                    table_name: "posts".to_string(),
                    columns: vec!["user_id".to_string()],
                    is_unique: false,
                    is_constraint: false,
                },
            ],
            foreign_keys: vec![
                ForeignKey {
                    name: "fk_posts_user_id".to_string(),
                    table_name: "posts".to_string(),
                    column_name: "user_id".to_string(),
                    referenced_table_name: "users".to_string(),
                    referenced_column_name: "id".to_string(),
                    on_delete: Some(ForeignKeyAction::Cascade),
                    on_update: Some(ForeignKeyAction::Cascade),
                },
            ],
            comment: Some("文章表".to_string()),
        })],
        sql: None,
        timestamp: Some(time::OffsetDateTime::now_utc()),
    };

    println!("  ✓ 迁移 V2 创建成功");
    println!("    - 版本: {}", migration_v2.version);
    println!("    - 描述: {}", migration_v2.description);

    // 6. 创建第三个迁移：添加索引
    println!("\n6️⃣ 创建迁移 V3：为 users 表添加索引");
    println!("------------------------------------------");

    let migration_v3 = Migration {
        version: 3,
        description: "Add index to users.name".to_string(),
        table_changes: vec![TableChange::AlterTable {
            table_name: "users".to_string(),
            column_changes: vec![],
            added_columns: vec![],
            removed_columns: vec![],
            added_indexes: vec![
                Index {
                    name: "idx_users_name".to_string(),
                    table_name: "users".to_string(),
                    columns: vec!["name".to_string()],
                    is_unique: false,
                    is_constraint: false,
                },
            ],
            removed_indexes: vec![],
            added_foreign_keys: vec![],
            removed_foreign_keys: vec![],
        }],
        sql: None,
        timestamp: Some(time::OffsetDateTime::now_utc()),
    };

    println!("  ✓ 迁移 V3 创建成功");
    println!("    - 版本: {}", migration_v3.version);
    println!("    - 描述: {}", migration_v3.description);

    // 7. 应用迁移
    println!("\n7️⃣ 应用迁移");
    println!("------------------------------------------");

    let migrations = vec![migration_v1, migration_v2, migration_v3];

    for migration in &migrations {
        match executor.apply_migration(migration).await {
            Ok(_) => {
                println!("  ✓ 迁移 V{} 应用成功", migration.version);
            }
            Err(e) => {
                println!("  ✗ 迁移 V{} 应用失败: {}", migration.version, e);
            }
        }
    }

    // 8. 查看迁移历史
    println!("\n8️⃣ 查看迁移历史");
    println!("------------------------------------------");
    executor.load_history().await?;
    println!("  📋 已应用的迁移数: {}", executor.history().applied_migrations.len());

    for mv in &executor.history().applied_migrations {
        println!("    - V{}: {} (应用于 {})", mv.version, mv.description, mv.applied_at);
    }

    println!("========================================");
    println!("✨ 数据库迁移示例运行完成！");

    println!("\n💡 提示:");
    println!("  - 迁移文件通常存储在 migrations/ 目录");
    println!("  - 使用 DbPool::run_migrations() 可以自动应用迁移");
    println!("  - 迁移支持回滚功能");

    Ok(())
}