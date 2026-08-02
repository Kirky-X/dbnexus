// Copyright (c) 2026 Kirky.X
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
    println!("🗄️  DBNexus SQLite 集成示例");
    println!("========================================\n");

    // ============================================
    // 1. 内存模式
    // ============================================
    println!("─── 内存模式 (sqlite::memory:) ───\n");
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
    println!("✓ 内存连接池创建成功");

    let session = memory_pool.get_session("admin").await?;
    println!("✓ Session 获取成功 (角色: {})", session.role());

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
    println!("✓ users 表创建成功");

    // DML: 插入
    let r1 = session
        .execute_raw("INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@example.com')")
        .await?;
    let r2 = session
        .execute_raw("INSERT INTO users (id, name, email) VALUES (2, 'Bob', 'bob@example.com')")
        .await?;
    println!(
        "✓ 插入 {} 条记录 (rows_affected: {}, {})",
        2,
        r1.rows_affected(),
        r2.rows_affected()
    );

    drop(session);
    println!("ℹ️  Session 释放，连接归还池中");

    // 内存模式特性说明
    println!("\n📌 内存模式特性:");
    println!("  - 数据存储在进程内存中，退出即销毁");
    println!("  - 适用于测试、临时计算、原型开发");
    println!("  - 每个连接独立的内存库（除非用 file::memory:?cache=shared）");

    // ============================================
    // 2. 文件模式
    // ============================================
    println!("\n─── 文件模式 (sqlite://example.db) ───\n");

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
    println!("✓ 文件连接池创建成功 (文件: {})", db_path);

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
    println!("✓ products 表创建成功");

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
    println!("✓ 插入 3 条产品记录");

    drop(session);
    println!("ℹ️  Session 释放");

    // 文件模式特性说明
    println!("\n📌 文件模式特性:");
    println!("  - 数据持久化到磁盘文件");
    println!("  - 适用于生产环境、需要数据持久化的场景");
    println!("  - 支持多进程访问（需配置 WAL 模式）");

    // 验证文件已创建
    let metadata = std::fs::metadata(db_path)?;
    println!("\n✓ 数据库文件已创建: {} ({} 字节)", db_path, metadata.len());

    // 清理示例文件
    drop(file_pool);
    let _ = std::fs::remove_file(db_path);
    println!("ℹ️  已清理示例数据库文件");

    // ============================================
    // 3. 两种模式对比
    // ============================================
    println!("\n─── 模式对比 ───\n");
    println!("┌────────────┬───────────────┬──────────────────┐");
    println!("│ 维度       │ 内存模式      │ 文件模式          │");
    println!("├────────────┼───────────────┼──────────────────┤");
    println!("│ URL 格式   │ sqlite::memory: │ sqlite:path.db │");
    println!("│ 持久化     │ 否            │ 是               │");
    println!("│ 性能       │ 最高          │ 高（受磁盘IO限制）│");
    println!("│ 适用场景   │ 测试/原型     │ 生产环境          │");
    println!("│ 多进程共享 │ 需 cache=shared │ 支持（WAL 模式）│");
    println!("└────────────┴───────────────┴──────────────────┘");

    println!("\n========================================");
    println!("✨ SQLite 集成示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - sqlite::memory: 创建内存数据库，退出即销毁");
    println!("  - sqlite:path.db 创建文件数据库，数据持久化");
    println!("  - session.execute_raw_ddl(sql) 执行 DDL（仅 admin）");
    println!("  - session.execute_raw(sql) 执行 DML（INSERT/UPDATE/DELETE/SELECT）");
    println!("  - DbPool::with_config(config) 统一入口创建连接池");

    Ok(())
}
