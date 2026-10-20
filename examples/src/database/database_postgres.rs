// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! PostgreSQL 数据库集成示例
//!
//! 演示如何连接 PostgreSQL 并执行基本操作。
//! 如果环境中没有可用的 PostgreSQL，示例会优雅地处理连接失败并退出。
//!
//! # 前置条件
//!
//! 需要 Docker 或本地 PostgreSQL：
//!
//! ```bash
//! docker run -d --name pg-dbnexus \
//!   -e POSTGRES_USER=postgres \
//!   -e POSTGRES_PASSWORD=postgres \
//!   -e POSTGRES_DB=dbnexus_example \
//!   -p 5432:5432 \
//!   postgres:16
//! ```
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example database_postgres --features "postgres"
//! ```
//!
//! 如果无 PostgreSQL 环境，示例不会 panic，只会打印提示信息。

use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🐘 DBNexus PostgreSQL integration example");
    println!("========================================\n");

    // 配置连接字符串
    // 标准 PostgreSQL 连接字符串格式：
    //   postgres://user:password@host:port/database
    //   postgresql://user:password@host:port/database
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/dbnexus_example".to_string()
    });

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

    // 尝试创建连接池
    println!("\nTrying to connect to PostgreSQL...");
    let pool = match DbPool::with_config(config).await {
        Ok(pool) => {
            println!("✓ PostgreSQL connected!");
            pool
        }
        Err(e) => {
            println!("✗ cannot connect to PostgreSQL");
            println!("\n⚠️  Connection failed. Possible reasons:");
            println!("  - the PostgreSQL server is not running");
            println!("  - the username/password in the connection string is incorrect");
            println!("  - the database does not exist");
            println!("  - the network is unreachable");
            println!("\n📋 Error details: {:?}", e);
            println!("\n💡 Solution:");
            println!("  1. start PostgreSQL with Docker:");
            println!("     docker run -d --name pg-dbnexus \\");
            println!("       -e POSTGRES_USER=postgres \\");
            println!("       -e POSTGRES_PASSWORD=postgres \\");
            println!("       -e POSTGRES_DB=dbnexus_example \\");
            println!("       -p 5432:5432 postgres:16");
            println!("  2. or set the DATABASE_URL environment variable to an available instance");
            println!(
                "\nℹ️  The example handled the connection failure gracefully, without panicking."
            );
            return Ok(());
        }
    };

    // 基本操作
    let session = pool.get_session("admin").await?;
    println!("\n✓ Session acquired (role: {})", session.role());

    // DDL: 建表
    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS users (
                id SERIAL PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(200) NOT NULL UNIQUE
            )",
        )
        .await?;
    println!("✓ users table created/confirmed");

    // DML: 插入（使用 ON CONFLICT 避免重复插入报错）
    let insert_result = session
        .execute_raw(
            "INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')
             ON CONFLICT (email) DO NOTHING",
        )
        .await?;
    println!(
        "✓ insert completed (rows_affected: {})",
        insert_result.rows_affected()
    );

    drop(session);
    println!("ℹ️  Session released");

    // 连接池状态
    let status = pool.status();
    println!("\n📊 Pool status:");
    println!("  - total connections: {}", status.total);
    println!("  - active connections: {}", status.active);
    println!("  - idle connections: {}", status.idle);

    println!("\n========================================");
    println!("✨ PostgreSQL integration example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - PostgreSQL connection string: postgres://user:pass@host:port/db");
    println!("  - DbPool::with_config is the unified entry point, same as SQLite");
    println!("  - without a PG environment, handle it gracefully via match, no panic");
    println!("  - SERIAL is PostgreSQL's auto-increment primary key type");

    Ok(())
}
