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
    migration::{Column, ColumnType, ForeignKey, Index, Migration, MigrationExecutor, Table, TableChange},
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🗄️  DBNexus 数据库迁移示例\n");
    println!("========================================");

    // 1. 初始化数据库连接池
    println!("\n1️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let pool = DbPool::new("sqlite::memory:").await?;
    println!("✓ 连接池创建成功");

    // 2. 创建迁移执行器
    println!("\n2️⃣ 创建迁移执行器");
    println!("------------------------------------------");
    let connection = pool.get_connection().await?;
    let mut executor = MigrationExecutor::new(connection, DatabaseType::Sqlite);
    println!("✓ 迁移执行器创建成功");

    // 3. 加载迁移历史
    println!("\n3️⃣ 加载迁移历史");
    println!("------------------------------------------");
    executor.load_history().await?;
    println!("✓ 迁移历史加载完成");
    println!("  📋 已应用的迁移数: {}", executor.history().migrations().len());

    // 4. 创建第一个迁移：创建 users 表
    println!("\n4️⃣ 创建迁移 V1：创建 users 表");
    println!("------------------------------------------");

    let migration_v1 = Migration {
        version: 1,
        description: "Create users table".to_string(),
        timestamp: Some(time::OffsetDateTime::now_utc()),
        changes: vec![TableChange::CreateTable(Table {
            name: "users".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    column_type: ColumnType::Integer,
                    nullable: false,
                    primary_key: true,
                    auto_increment: true,
                    default_value: None,
                    unique: false,
                },
                Column {
                    name: "name".to_string(),
                    column_type: ColumnType::Text,
                    nullable: false,
                    primary_key: false,
                    auto_increment: false,
                    default_value: None,
                    unique: false,
                },
                Column {
                    name: "email".to_string(),
                    column_type: ColumnType::Text,
                    nullable: false,
                    primary_key: false,
                    auto_increment: false,
                    default_value: None,
                    unique: true,
                },
                Column {
                    name: "created_at".to_string(),
                    column_type: ColumnType::Timestamp,
                    nullable: false,
                    primary_key: false,
                    auto_increment: false,
                    default_value: Some("CURRENT_TIMESTAMP".to_string()),
                    unique: false,
                },
            ],
            indexes: vec![],
            foreign_keys: vec![],
        })],
    };

    // 应用迁移 V1
    executor.apply_migration(&migration_v1).await?;
    println!("  ✓ 迁移 V1 应用成功");
    println!("  📝 描述: {}", migration_v1.description);

    // 5. 创建第二个迁移：创建 posts 表和添加外键
    println!("\n5️⃣ 创建迁移 V2：创建 posts 表");
    println!("------------------------------------------");

    let migration_v2 = Migration {
        version: 2,
        description: "Create posts table with foreign key".to_string(),
        timestamp: Some(time::OffsetDateTime::now_utc()),
        changes: vec![TableChange::CreateTable(Table {
            name: "posts".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    column_type: ColumnType::Integer,
                    nullable: false,
                    primary_key: true,
                    auto_increment: true,
                    default_value: None,
                    unique: false,
                },
                Column {
                    name: "title".to_string(),
                    column_type: ColumnType::Text,
                    nullable: false,
                    primary_key: false,
                    auto_increment: false,
                    default_value: None,
                    unique: false,
                },
                Column {
                    name: "content".to_string(),
                    column_type: ColumnType::Text,
                    nullable: true,
                    primary_key: false,
                    auto_increment: false,
                    default_value: None,
                    unique: false,
                },
                Column {
                    name: "user_id".to_string(),
                    column_type: ColumnType::Integer,
                    nullable: false,
                    primary_key: false,
                    auto_increment: false,
                    default_value: None,
                    unique: false,
                },
                Column {
                    name: "created_at".to_string(),
                    column_type: ColumnType::Timestamp,
                    nullable: false,
                    primary_key: false,
                    auto_increment: false,
                    default_value: Some("CURRENT_TIMESTAMP".to_string()),
                    unique: false,
                },
            ],
            indexes: vec![Index {
                name: "idx_posts_user_id".to_string(),
                columns: vec!["user_id".to_string()],
                unique: false,
            }],
            foreign_keys: vec![ForeignKey {
                name: "fk_posts_user_id".to_string(),
                from_table: "posts".to_string(),
                from_column: "user_id".to_string(),
                to_table: "users".to_string(),
                to_column: "id".to_string(),
                on_delete: "CASCADE".to_string(),
                on_update: "CASCADE".to_string(),
            }],
        })],
    };

    // 应用迁移 V2
    executor.apply_migration(&migration_v2).await?;
    println!("  ✓ 迁移 V2 应用成功");
    println!("  📝 描述: {}", migration_v2.description);

    // 6. 创建第三个迁移：修改 users 表，添加新列
    println!("\n6️⃣ 创建迁移 V3：修改 users 表");
    println!("------------------------------------------");

    let migration_v3 = Migration {
        version: 3,
        description: "Add status column to users table".to_string(),
        timestamp: Some(time::OffsetDateTime::now_utc()),
        changes: vec![TableChange::AlterTable {
            table_name: "users".to_string(),
            column_changes: vec![],
            added_columns: vec![Column {
                name: "status".to_string(),
                column_type: ColumnType::Text,
                nullable: false,
                primary_key: false,
                auto_increment: false,
                default_value: Some("'active'".to_string()),
                unique: false,
            }],
            removed_columns: vec![],
            added_indexes: vec![Index {
                name: "idx_users_status".to_string(),
                columns: vec!["status".to_string()],
                unique: false,
            }],
            removed_indexes: vec![],
            added_foreign_keys: vec![],
            removed_foreign_keys: vec![],
        }],
    };

    // 应用迁移 V3
    executor.apply_migration(&migration_v3).await?;
    println!("  ✓ 迁移 V3 应用成功");
    println!("  📝 描述: {}", migration_v3.description);

    // 7. 创建第四个迁移：添加索引
    println!("\n7️⃣ 创建迁移 V4：添加索引");
    println!("------------------------------------------");

    let migration_v4 = Migration {
        version: 4,
        description: "Add index on posts created_at".to_string(),
        timestamp: Some(time::OffsetDateTime::now_utc()),
        changes: vec![TableChange::AlterTable {
            table_name: "posts".to_string(),
            column_changes: vec![],
            added_columns: vec![],
            removed_columns: vec![],
            added_indexes: vec![Index {
                name: "idx_posts_created_at".to_string(),
                columns: vec!["created_at".to_string()],
                unique: false,
            }],
            removed_indexes: vec![],
            added_foreign_keys: vec![],
            removed_foreign_keys: vec![],
        }],
    };

    // 应用迁移 V4
    executor.apply_migration(&migration_v4).await?;
    println!("  ✓ 迁移 V4 应用成功");
    println!("  📝 描述: {}", migration_v4.description);

    // 8. 重新加载迁移历史
    println!("\n8️⃣ 重新加载迁移历史");
    println!("------------------------------------------");
    executor.load_history().await?;
    println!("✓ 迁移历史重新加载完成");

    // 9. 显示迁移历史
    println!("\n9️⃣ 显示迁移历史");
    println!("------------------------------------------");

    let migrations = executor.history().migrations();
    println!("  📋 迁移历史 ({} 个迁移):", migrations.len());
    for (i, migration) in migrations.iter().enumerate() {
        println!("    {}. V{} - {}", i + 1, migration.version, migration.description);
        println!("       应用时间: {}", migration.applied_at);
        println!("       文件路径: {}", migration.file_path);
    }

    // 10. 验证数据库结构
    println!("\n🔟 验证数据库结构");
    println!("------------------------------------------");

    // 查询 users 表结构
    let users_schema = executor
        .connection()
        .query_all(
            &sea_orm::sea_query::Query::select()
                .from(sea_orm::sea_query::Alias::new("sqlite_master"))
                .columns(vec![
                    sea_orm::sea_query::Alias::new("name"),
                    sea_orm::sea_query::Alias::new("sql"),
                ])
                .and_where(sea_orm::sea_query::Expr::col(sea_orm::sea_query::Alias::new("type")).eq("table"))
                .and_where(sea_orm::sea_query::Expr::col(sea_orm::sea_query::Alias::new("name")).eq("users"))
                .to_string(sea_orm::DbBackend::Sqlite),
        )
        .await;

    match users_schema {
        Ok(rows) => {
            if !rows.is_empty() {
                println!("  ✓ users 表存在");
            }
        }
        Err(e) => {
            println!("  ✗ 查询 users 表失败: {}", e);
        }
    }

    // 查询 posts 表结构
    let posts_schema = executor
        .connection()
        .query_all(
            &sea_orm::sea_query::Query::select()
                .from(sea_orm::sea_query::Alias::new("sqlite_master"))
                .columns(vec![
                    sea_orm::sea_query::Alias::new("name"),
                    sea_orm::sea_query::Alias::new("sql"),
                ])
                .and_where(sea_orm::sea_query::Expr::col(sea_orm::sea_query::Alias::new("type")).eq("table"))
                .and_where(sea_orm::sea_query::Expr::col(sea_orm::sea_query::Alias::new("name")).eq("posts"))
                .to_string(sea_orm::DbBackend::Sqlite),
        )
        .await;

    match posts_schema {
        Ok(rows) => {
            if !rows.is_empty() {
                println!("  ✓ posts 表存在");
            }
        }
        Err(e) => {
            println!("  ✗ 查询 posts 表失败: {}", e);
        }
    }

    // 11. 演示迁移文件路径
    println!("\n1️⃣1️⃣ 迁移文件组织");
    println!("------------------------------------------");

    println!("  📁 推荐的迁移文件目录结构:");
    println!("     migrations/");
    println!("     ├── 001_create_users_table.sql");
    println!("     ├── 002_create_posts_table.sql");
    println!("     ├── 003_add_status_to_users.sql");
    println!("     └── 004_add_index_on_posts_created_at.sql");

    println!("\n  📄 迁移文件命名规则:");
    println!("     - 使用 3 位数字前缀（001, 002, ...）");
    println!("     - 使用下划线分隔单词");
    println!("     - 使用描述性名称");
    println!("     - 使用 .sql 扩展名");

    // 12. 演示迁移回滚（仅演示概念）
    println!("\n1️⃣2️⃣ 迁移回滚（概念演示）");
    println!("------------------------------------------");

    println!("  💡 迁移回滚通常需要:");
    println!("     1. 为每个迁移创建对应的 down migration");
    println!("     2. 按相反的顺序应用 down migration");
    println!("     3. 更新迁移历史记录");

    println!("\n  📝 示例 down migration:");
    println!("     -- 004_add_index_on_posts.sql");
    println!("     DROP INDEX IF EXISTS idx_posts_created_at;");

    println!("\n========================================");
    println!("✨ 数据库迁移示例运行完成！");

    println!("\n💡 提示:");
    println!("  - 在生产环境中，迁移文件应该进行版本控制");
    println!("  - 每个迁移应该是可逆的（提供 up 和 down 脚本）");
    println!("  - 在应用迁移前，先在测试环境验证");
    println!("  - 使用事务确保迁移的原子性");
    println!("  - 考虑使用自动迁移功能（auto-migrate feature）");

    Ok(())
}
