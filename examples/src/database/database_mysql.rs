// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! MySQL 数据库集成示例
//!
//! 演示如何连接 MySQL 并执行基本操作。
//! 如果环境中没有可用的 MySQL，示例会优雅地处理连接失败并退出。
//!
//! # 前置条件
//!
//! 需要 Docker 或本地 MySQL：
//!
//! ```bash
//! docker run -d --name mysql-dbnexus \
//!   -e MYSQL_ROOT_PASSWORD=root \
//!   -e MYSQL_DATABASE=dbnexus_example \
//!   -p 3306:3306 \
//!   mysql:8
//! ```
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example database_mysql --features "mysql"
//! ```
//!
//! 如果无 MySQL 环境，示例不会 panic，只会打印提示信息。

use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🐬 DBNexus MySQL integration example");
    println!("========================================\n");

    // ============================================
    // 配置连接字符串
    // ============================================
    // 标准 MySQL 连接字符串格式：
    //   mysql://user:password@host:port/database
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://root:root@localhost:3306/dbnexus_example".to_string());

    println!("connection string: {}", db_url);

    let config = DbConfig {
        url: db_url.clone(),
        admin_role: "admin".to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 10,
            min_connections: 2,
            idle_timeout: 300,
            acquire_timeout: 5000,
        },
        ..Default::default()
    };

    // ============================================
    // 尝试创建连接池
    // ============================================
    println!("\nTrying to connect to MySQL...");
    let pool = match DbPool::with_config(config).await {
        Ok(pool) => {
            println!("✓ MySQL connected!");
            pool
        }
        Err(e) => {
            println!("✗ cannot connect to MySQL");
            println!("\n⚠️  Connection failed. Possible reasons:");
            println!("  - the MySQL server is not running");
            println!("  - the username/password in the connection string is incorrect");
            println!("  - the database does not exist");
            println!("  - the network is unreachable");
            println!("\n📋 Error details: {:?}", e);
            println!("\n💡 Solution:");
            println!("  1. start MySQL with Docker:");
            println!("     docker run -d --name mysql-dbnexus \\");
            println!("       -e MYSQL_ROOT_PASSWORD=root \\");
            println!("       -e MYSQL_DATABASE=dbnexus_example \\");
            println!("       -p 3306:3306 mysql:8");
            println!("  2. or set the DATABASE_URL environment variable to an available instance");
            println!(
                "\nℹ️  The example handled the connection failure gracefully, without panicking."
            );
            return Ok(());
        }
    };

    // ============================================
    // 基本操作
    // ============================================
    let session = pool.get_session("admin").await?;
    println!("\n✓ Session acquired (role: {})", session.role());

    // DDL: 建表
    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(200) NOT NULL,
                UNIQUE KEY uk_email (email)
            )",
        )
        .await?;
    println!("✓ users table created/confirmed");

    // DML: 插入（使用 INSERT IGNORE 避免重复插入报错）
    let insert_result = session
        .execute_raw("INSERT IGNORE INTO users (name, email) VALUES ('Alice', 'alice@example.com')")
        .await?;
    println!(
        "✓ insert completed (rows_affected: {})",
        insert_result.rows_affected()
    );

    drop(session);
    println!("ℹ️  Session released");

    // ============================================
    // 连接池状态
    // ============================================
    let status = pool.status();
    println!("\n📊 Pool status:");
    println!("  - total connections: {}", status.total);
    println!("  - active connections: {}", status.active);
    println!("  - idle connections: {}", status.idle);

    println!("\n========================================");
    println!("✨ MySQL integration example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - MySQL connection string: mysql://user:pass@host:port/db");
    println!("  - DbPool::with_config is the unified entry point, same as SQLite/PostgreSQL");
    println!("  - without a MySQL environment, handle it gracefully via match, no panic");
    println!("  - AUTO_INCREMENT is MySQL's auto-increment primary key syntax");

    Ok(())
}
