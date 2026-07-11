// Copyright (c) 2026 Kirky.X
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
    println!("🐘 DBNexus PostgreSQL 集成示例");
    println!("========================================\n");

    // ============================================
    // 1. 配置连接字符串
    // ============================================
    // 标准 PostgreSQL 连接字符串格式：
    //   postgres://user:password@host:port/database
    //   postgresql://user:password@host:port/database
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/dbnexus_example".to_string());

    println!("连接字符串: {}", db_url);

    let config = DbConfig {
        url: db_url.clone(),
        admin_role: "admin".to_string(),
        max_connections: 10,
        min_connections: 2,
        idle_timeout: 300,
        acquire_timeout: 5000,
        ..Default::default()
    };

    // ============================================
    // 2. 尝试创建连接池
    // ============================================
    println!("\n尝试连接 PostgreSQL...");
    let pool = match DbPool::with_config(config).await {
        Ok(pool) => {
            println!("✓ PostgreSQL 连接成功！");
            pool
        }
        Err(e) => {
            println!("✗ 无法连接 PostgreSQL");
            println!("\n⚠️  连接失败，可能是以下原因：");
            println!("  - PostgreSQL 服务未启动");
            println!("  - 连接字符串中的用户名/密码不正确");
            println!("  - 数据库不存在");
            println!("  - 网络不可达");
            println!("\n📋 错误详情: {:?}", e);
            println!("\n💡 解决方案：");
            println!("  1. 使用 Docker 启动 PostgreSQL：");
            println!("     docker run -d --name pg-dbnexus \\");
            println!("       -e POSTGRES_USER=postgres \\");
            println!("       -e POSTGRES_PASSWORD=postgres \\");
            println!("       -e POSTGRES_DB=dbnexus_example \\");
            println!("       -p 5432:5432 postgres:16");
            println!("  2. 或设置 DATABASE_URL 环境变量指向可用实例");
            println!("\nℹ️  示例已优雅处理连接失败，未 panic。");
            return Ok(());
        }
    };

    // ============================================
    // 3. 基本操作
    // ============================================
    let session = pool.get_session("admin").await?;
    println!("\n✓ Session 获取成功 (角色: {})", session.role());

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
    println!("✓ users 表创建/确认成功");

    // DML: 插入（使用 ON CONFLICT 避免重复插入报错）
    let insert_result = session
        .execute_raw(
            "INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')
             ON CONFLICT (email) DO NOTHING",
        )
        .await?;
    println!("✓ 插入操作完成 (rows_affected: {})", insert_result.rows_affected());

    drop(session);
    println!("ℹ️  Session 释放");

    // ============================================
    // 4. 连接池状态
    // ============================================
    let status = pool.status();
    println!("\n📊 连接池状态:");
    println!("  - 总连接数: {}", status.total);
    println!("  - 活跃连接: {}", status.active);
    println!("  - 空闲连接: {}", status.idle);

    println!("\n========================================");
    println!("✨ PostgreSQL 集成示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - PostgreSQL 连接字符串: postgres://user:pass@host:port/db");
    println!("  - DbPool::with_config 统一入口，与 SQLite 用法一致");
    println!("  - 无 PG 环境时通过 match 优雅处理，不 panic");
    println!("  - SERIAL 类型是 PostgreSQL 的自增主键");

    Ok(())
}
