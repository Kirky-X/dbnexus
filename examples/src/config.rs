// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置管理示例
//!
//! 展示如何使用 dbnexus 的配置管理功能：
//! - 使用 DbConfigBuilder 创建配置
//! - 使用配置结构体创建配置
//! - 从配置文件加载配置
//! - 使用环境变量配置
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example config --features sqlite
//! ```

use dbnexus::{DbPool, config::DbConfigBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DB Nexus 配置管理示例 ===\n");

    // 示例 1: 使用构建器创建配置
    println!("1. 使用配置构建器:");
    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .max_connections(20)
        .min_connections(5)
        .idle_timeout(300)
        .acquire_timeout(5000)
        .admin_role("admin")
        .build()
        .expect("Failed to build config");

    println!("   URL: {}", config.url_sanitized());
    println!("   Max connections: {}", config.max_connections());
    println!("   Min connections: {}", config.min_connections());

    // 示例 2: 使用构建器创建配置
    println!("\n2. 使用配置构建器:");
    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .max_connections(10)
        .min_connections(2)
        .idle_timeout(300)
        .acquire_timeout(5000)
        .admin_role("admin")
        .build()
        .expect("Failed to build config");

    println!("   URL: {}", config.url_sanitized());
    println!("   Max connections: {}", config.max_connections());

    // 示例 3: 使用 try_from_config 初始化连接池
    println!("\n3. 使用 try_from_config 初始化连接池:");
    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .max_connections(10)
        .build()
        .unwrap();

    let pool = DbPool::try_from_config(config).await?;
    println!("   连接池创建成功!");
    println!("   池状态: {:?}", pool.status());

    // 示例 4: 使用 try_from 同步初始化
    println!("\n4. 使用 try_from 同步初始化:");
    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .max_connections(5)
        .build()
        .unwrap();

    let pool = DbPool::try_from(&config)?;
    println!("   连接池同步创建成功!");
    println!("   池状态: {:?}", pool.status());

    println!("\n=== 所有示例完成 ===");
    Ok(())
}
