// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置管理示例
//!
//! 展示如何使用基于 confers 库的 dbnexus 配置管理功能：
//! - 使用 DbConfigBuilder 创建配置
//! - 从配置文件加载配置
//! - 使用环境变量配置
//! - 多源配置合并
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example config --features sqlite
//! ```

use dbnexus::{DbPool, config::DbConfigBuilder};

#[cfg(feature = "confers")]
use dbnexus::config::DbConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DB Nexus 基于 Confers 的配置管理示例 ===\n");

    #[cfg(feature = "confers")]
    {
        // 示例 1: 使用 confers 构建器创建配置
        println!("1. 使用 confers 配置构建器:");
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

        // 示例 2: 从环境变量加载配置
        println!("\n2. 从环境变量加载配置:");
        std::env::set_var("DATABASE_URL", "sqlite:file::memory:?cache=shared");
        std::env::set_var("DB_MAX_CONNECTIONS", "15");
        std::env::set_var("DB_ADMIN_ROLE", "administrator");
        
        let config = DbConfig::from_env()?;
        println!("   URL: {}", config.url_sanitized());
        println!("   Max connections: {}", config.max_connections());
        println!("   Admin role: {}", config.admin_role());

        // 示例 3: 多源配置合并
        println!("\n3. 多源配置合并 (环境变量 + 默认值):");
        std::env::set_var("DB_MAX_CONNECTIONS", "25");
        // DB_MIN_CONNECTIONS 未设置，将使用默认值
        
        let config = DbConfig::builder()
            .env()
            .load()?;
        println!("   Max connections: {}", config.max_connections());
        println!("   Min connections: {}", config.min_connections()); // 应该是默认值 5

        // 示例 4: 使用 try_from_config 初始化连接池
        println!("\n4. 使用 try_from_config 初始化连接池:");
        let config = DbConfigBuilder::new()
            .url("sqlite:file::memory:?cache=shared")
            .max_connections(10)
            .build()
            .unwrap();

        let pool = DbPool::try_from_config(config).await?;
        println!("   连接池创建成功!");
        println!("   池状态: {:?}", pool.status());

        // 示例 5: 使用 try_from 同步初始化
        println!("\n5. 使用 try_from 同步初始化:");
        let config = DbConfigBuilder::new()
            .url("sqlite:file::memory:?cache=shared")
            .max_connections(5)
            .build()
            .unwrap();

        let pool = DbPool::try_from(&config)?;
        println!("   连接池同步创建成功!");
        println!("   池状态: {:?}", pool.status());
    }

    #[cfg(not(feature = "confers"))]
    {
        println!("请启用 confers 特性来运行此示例");
        println!("运行命令: cargo run --example config --features \"sqlite,confers\"");
    }

    println!("\n=== 所有示例完成 ===");
    Ok(())
}
