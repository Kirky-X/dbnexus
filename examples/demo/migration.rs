// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 数据库迁移示例
//!
//! 展示如何使用 dbnexus 的迁移功能：
//! - 迁移类型定义
//! - Schema 差异检测概念
//! - 列变更操作类型
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example migration --features "sqlite,migration"
//! ```

use dbnexus::migration::ColumnChangeType;
use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 DBNexus 数据库迁移示例\n");
    println!("========================================");

    // 1. 初始化数据库连接池
    println!("\n1️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // 2. 创建测试表
    println!("\n2️⃣ 创建测试表");
    println!("------------------------------------------");
    let session = pool.get_session("admin").await?;

    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .await?;
    println!("✓ users 表创建成功");

    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS orders (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                amount REAL NOT NULL,
                status TEXT DEFAULT 'pending'
            )",
        )
        .await?;
    println!("✓ orders 表创建成功");

    // 3. 列变更类型示例
    println!("\n3️⃣ 列变更类型");
    println!("------------------------------------------");
    println!("  支持的列变更类型:");
    println!("    - {:?}: 重命名列", ColumnChangeType::RenameColumn);
    println!("    - {:?}: 修改列类型", ColumnChangeType::ModifyColumn);
    println!("    - {:?}: 可空性变更", ColumnChangeType::NullabilityChanged);
    println!("    - {:?}: 默认值变更", ColumnChangeType::DefaultChanged);

    // 4. 表变更类型示例
    println!("\n4️⃣ 表变更类型");
    println!("------------------------------------------");
    println!("  支持的表变更类型:");
    println!("    - CreateTable: 创建新表");
    println!("    - DropTable: 删除表");
    println!("    - AlterTable: 修改表结构");

    // 5. Schema 差异检测
    println!("\n5️⃣ Schema 差异检测");
    println!("------------------------------------------");
    println!("  差异检测用于比较期望 Schema 和实际 Schema");
    println!("  支持检测:");
    println!("    - 新增表、删除表、修改表结构");
    println!("    - 新增列、删除列、修改列类型");
    println!("    - 新增索引、删除索引");
    println!("  支持生成:");
    println!("    - 迁移 SQL");
    println!("    - 回滚 SQL");

    // 6. 迁移执行器
    println!("\n6️⃣ 迁移执行器");
    println!("------------------------------------------");
    println!("  MigrationExecutor 提供:");
    println!("    - 迁移历史记录管理");
    println!("    - 自动创建迁移历史表");
    println!("    - SQL 生成和执行");

    // 7. 连接池状态
    println!("\n7️⃣ 连接池状态");
    println!("------------------------------------------");
    let pool_status = pool.status();
    println!("  总连接数: {}", pool_status.total);
    println!("  活跃连接数: {}", pool_status.active);
    println!("  空闲连接数: {}", pool_status.idle);

    println!("\n=== 所有迁移示例完成 ===");
    Ok(())
}
