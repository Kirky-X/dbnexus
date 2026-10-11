// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! SQLite 数据库集成示例
//!
//! 演示 SQLite 的两种运行模式及基本 DDL/DML 操作：
//! - 内存模式（`sqlite::memory:`）：进程内临时数据库，退出即销毁
//! - 文件模式（`sqlite:file.db`）：持久化到文件系统
//! - 执行 CREATE TABLE / INSERT / SELECT
//! - 对比两种模式的区别
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example database_sqlite --features "sqlite"
//! ```

use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🗄️  DBNexus SQLite integration example");
    println!("========================================\n");

    // ============================================
    // 内存模式
    // ============================================
    println!("─── In-memory mode (sqlite::memory:) ───\n");
    let memory_config = DbConfig {
        url: "sqlite::memory:".to_string(),
        admin_role: "admin".to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let memory_pool = DbPool::with_config(memory_config).await?;
    println!("✓ in-memory pool created");

    let session = memory_pool.get_session("admin").await?;
    println!("✓ Session acquired (role: {})", session.role());

    // DDL: 建表
    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL
            )",
        )
        .await?;
    println!("✓ users table created");

    // DML: 插入
    let r1 = session
        .execute_raw("INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@example.com')")
        .await?;
    let r2 = session
        .execute_raw("INSERT INTO users (id, name, email) VALUES (2, 'Bob', 'bob@example.com')")
        .await?;
    println!(
        "✓ inserted {} records (rows_affected: {}, {})",
        2,
        r1.rows_affected(),
        r2.rows_affected()
    );

    drop(session);
    println!("ℹ️  Session released, connection returned to the pool");

    // 内存模式特性说明
    println!("\n📌 In-memory mode features:");
    println!("  - data lives in process memory and is destroyed on exit");
    println!("  - for testing, temporary computation, and prototyping");
    println!(
        "  - each connection gets an independent in-memory database (unless file::memory:?cache=shared)"
    );

    // ============================================
    // 文件模式
    // ============================================
    println!("\n─── File mode (sqlite://example.db) ───\n");

    let db_path = "dbnexus_example.db";
    // 清理旧文件以确保示例干净
    let _ = std::fs::remove_file(db_path);
    // 预创建空文件：sqlx 默认 create_if_missing=false，需先存在文件才能连接
    // （与 tests/common/mod.rs 的做法一致）
    std::fs::File::create(db_path)?;

    let file_config = DbConfig {
        url: format!("sqlite://{}", db_path),
        admin_role: "admin".to_string(),
        pool_config: dbnexus::foundation::PoolConfig {
            max_connections: 5,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let file_pool = DbPool::with_config(file_config).await?;
    println!("✓ file pool created (file: {})", db_path);

    let session = file_pool.get_session("admin").await?;

    // DDL: 建表
    session
        .execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS products (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                price REAL NOT NULL
            )",
        )
        .await?;
    println!("✓ products table created");

    // DML: 插入
    session
        .execute_raw("INSERT INTO products (id, name, price) VALUES (1, 'Widget', 9.99)")
        .await?;
    session
        .execute_raw("INSERT INTO products (id, name, price) VALUES (2, 'Gadget', 19.99)")
        .await?;
    session
        .execute_raw("INSERT INTO products (id, name, price) VALUES (3, 'Gizmo', 29.99)")
        .await?;
    println!("✓ inserted 3 product records");

    drop(session);
    println!("ℹ️  Session released");

    // 文件模式特性说明
    println!("\n📌 File mode features:");
    println!("  - data persists to a disk file");
    println!("  - for production environments needing data persistence");
    println!("  - supports multi-process access (requires WAL mode)");

    let metadata = std::fs::metadata(db_path)?;
    println!(
        "\n✓ Database file created: {} ({} bytes)",
        db_path,
        metadata.len()
    );

    drop(file_pool);
    let _ = std::fs::remove_file(db_path);
    println!("ℹ️  Example database file cleaned up");

    // ============================================
    // 两种模式对比
    // ============================================
    println!("\n─── Mode comparison ───\n");
    println!("┌────────────┬───────────────┬──────────────────┐");
    println!("│ dimension    │ in-memory mode│ file mode       │");
    println!("├────────────┼───────────────┼──────────────────┤");
    println!("│ URL format   │ sqlite::memory: │ sqlite:path.db │");
    println!("│ persistence  │ no            │ yes              │");
    println!("│ performance  │ highest       │ high (disk IO bound) │");
    println!("│ use cases    │ testing/prototyping │ production      │");
    println!("│ multi-proc   │ needs cache=shared │ supported (WAL mode) │");
    println!("└────────────┴───────────────┴──────────────────┘");

    println!("\n========================================");
    println!("✨ SQLite integration example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - sqlite::memory: creates an in-memory database, destroyed on exit");
    println!("  - sqlite:path.db creates a file database, data persisted");
    println!("  - session.execute_raw_ddl(sql) executes DDL (admin only)");
    println!("  - session.execute_raw(sql) executes DML (INSERT/UPDATE/DELETE/SELECT)");
    println!("  - DbPool::with_config(config) is the unified entry point for creating a pool");

    Ok(())
}
