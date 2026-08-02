// Copyright (c) 2026 Kirky.X
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
    println!("🐬 DBNexus MySQL 集成示例");
    println!("========================================\n");

    // ============================================
    // 1. 配置连接字符串
    // ============================================
    // 标准 MySQL 连接字符串格式：
    //   mysql://user:password@host:port/database
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://root:root@localhost:3306/dbnexus_example".to_string());

    println!("连接字符串: {}", db_url);

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
    // 2. 尝试创建连接池
    // ============================================
    println!("\n尝试连接 MySQL...");
    let pool = match DbPool::with_config(config).await {
        Ok(pool) => {
            println!("✓ MySQL 连接成功！");
            pool
        }
        Err(e) => {
            println!("✗ 无法连接 MySQL");
            println!("\n⚠️  连接失败，可能是以下原因：");
            println!("  - MySQL 服务未启动");
            println!("  - 连接字符串中的用户名/密码不正确");
            println!("  - 数据库不存在");
            println!("  - 网络不可达");
            println!("\n📋 错误详情: {:?}", e);
            println!("\n💡 解决方案：");
            println!("  1. 使用 Docker 启动 MySQL：");
            println!("     docker run -d --name mysql-dbnexus \\");
            println!("       -e MYSQL_ROOT_PASSWORD=root \\");
            println!("       -e MYSQL_DATABASE=dbnexus_example \\");
            println!("       -p 3306:3306 mysql:8");
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
                id INT AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(200) NOT NULL,
                UNIQUE KEY uk_email (email)
            )",
        )
        .await?;
    println!("✓ users 表创建/确认成功");

    // DML: 插入（使用 INSERT IGNORE 避免重复插入报错）
    let insert_result = session
        .execute_raw("INSERT IGNORE INTO users (name, email) VALUES ('Alice', 'alice@example.com')")
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
    println!("✨ MySQL 集成示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - MySQL 连接字符串: mysql://user:pass@host:port/db");
    println!("  - DbPool::with_config 统一入口，与 SQLite/PostgreSQL 用法一致");
    println!("  - 无 MySQL 环境时通过 match 优雅处理，不 panic");
    println!("  - AUTO_INCREMENT 是 MySQL 的自增主键语法");

    Ok(())
}
