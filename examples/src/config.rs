// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 配置管理示例
//!
//! 展示如何使用 dbnexus 配置管理功能：
//! - 使用 DbConfig 结构体创建配置
//! - 从 confers 配置提供者加载配置
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example config --features sqlite
//! ```

use dbnexus::{DbPool, config::DbConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DB Nexus 配置管理示例 ===\n");

    // 示例 1: 使用结构体字面量创建配置
    println!("1. 使用结构体字面量创建配置:");
    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        max_connections: 20,
        min_connections: 5,
        idle_timeout: 300,
        acquire_timeout: 5000,
        admin_role: "admin".to_string(),
        ..Default::default()
    };

    println!("   URL: {}", config.url);
    println!("   Max connections: {}", config.max_connections);
    println!("   Min connections: {}", config.min_connections);
    println!("   Admin role: {}", config.admin_role);

    // 示例 2: 使用默认值创建配置
    println!("\n2. 使用默认值创建配置:");
    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        ..Default::default()
    };

    println!("   URL: {}", config.url);
    println!("   Max connections: {} (默认)", config.max_connections);
    println!("   Min connections: {} (默认)", config.min_connections);
    println!("   Admin role: {} (默认)", config.admin_role);

    // 示例 3: 使用配置创建连接池
    println!("\n3. 使用配置创建连接池:");
    let pool = DbPool::with_config(config).await?;
    println!("   连接池创建成功!");
    println!("   池状态: {:?}", pool.status());

    // 示例 4: 配置示例说明
    println!("\n4. 配置管理特性:");
    println!("   DBNexus 支持多种配置方式:");
    println!("   - DbConfig 结构体（已在上面演示）");
    println!("   - 环境变量配置 (config-env feature)");
    println!("   - YAML 配置文件 (config-yaml feature)");
    println!("   - TOML 配置文件 (config-toml feature)");

    println!("\n=== 所有示例完成 ===");
    Ok(())
}
