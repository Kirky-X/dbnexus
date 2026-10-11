// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 环境变量配置示例
//!
//! 演示如何通过环境变量创建 [`DbConfig`]：
//! - 设置 `DATABASE_URL`、`DB_MAX_CONNECTIONS` 等环境变量
//! - 调用 `DbConfig::from_env()` 加载配置
//! - 打印解析后的配置信息
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example config_env --features "config-env"
//! ```

use dbnexus::DbConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("⚙️  DBNexus environment variable configuration example");
    println!("========================================\n");

    // ============================================
    // 1. 设置环境变量
    // ============================================
    // 生产中这些变量通常由容器/CI 注入，这里为了演示直接设置。
    // 注意：Rust 2024 edition 中 set_var 是 unsafe 的（可能引发数据竞争）。
    unsafe {
        std::env::set_var("DATABASE_URL", "sqlite::memory:");
        std::env::set_var("DB_MAX_CONNECTIONS", "10");
        std::env::set_var("DB_MIN_CONNECTIONS", "2");
        std::env::set_var("DB_IDLE_TIMEOUT", "600");
        std::env::set_var("DB_ACQUIRE_TIMEOUT", "3000");
        std::env::set_var("DB_ADMIN_ROLE", "superadmin");
        std::env::set_var("DB_AUTO_MIGRATE", "false");
    }

    println!("Environment variables set:");
    println!(
        "  - DATABASE_URL       = {}",
        std::env::var("DATABASE_URL")?
    );
    println!(
        "  - DB_MAX_CONNECTIONS = {}",
        std::env::var("DB_MAX_CONNECTIONS")?
    );
    println!(
        "  - DB_MIN_CONNECTIONS = {}",
        std::env::var("DB_MIN_CONNECTIONS")?
    );
    println!(
        "  - DB_IDLE_TIMEOUT    = {}",
        std::env::var("DB_IDLE_TIMEOUT")?
    );
    println!(
        "  - DB_ACQUIRE_TIMEOUT = {}",
        std::env::var("DB_ACQUIRE_TIMEOUT")?
    );
    println!(
        "  - DB_ADMIN_ROLE      = {}",
        std::env::var("DB_ADMIN_ROLE")?
    );

    // ============================================
    // 2. 从环境变量加载配置
    // ============================================
    let config = DbConfig::from_env()?;
    println!("\n✓ configuration loaded");

    // ============================================
    // 3. 打印配置信息
    // ============================================
    println!("\n📋 Parsed DbConfig:");
    println!("  - url              : {}", config.url);
    println!(
        "  - max_connections   : {}",
        config.pool_config.max_connections
    );
    println!(
        "  - min_connections   : {}",
        config.pool_config.min_connections
    );
    println!(
        "  - idle_timeout (s)  : {}",
        config.pool_config.idle_timeout
    );
    println!(
        "  - acquire_timeout(ms): {}",
        config.pool_config.acquire_timeout
    );
    println!("  - admin_role        : {}", config.admin_role);
    println!("  - auto_migrate      : {}", config.auto_migrate);
    println!(
        "  - database_type     : {}",
        config.database_type().unwrap()
    );

    println!("\n💾 Cache configuration:");
    println!(
        "  - policy_cache_capacity : {}",
        config.cache_config.policy_cache_capacity
    );
    println!(
        "  - sql_parse_cache_capacity: {}",
        config.cache_config.sql_parse_cache_capacity
    );
    println!(
        "  - query_cache_capacity  : {}",
        config.cache_config.query_cache_capacity
    );
    println!(
        "  - default_ttl (s)       : {}",
        config.cache_config.default_ttl
    );

    println!("\n========================================");
    println!("✨ Environment variable configuration example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - DbConfig::from_env() reads configuration from environment variables");
    println!("  - DATABASE_URL is required; a missing value returns ConfigError::MissingUrl");
    println!("  - other variables are optional and fall back to defaults when unset");

    Ok(())
}
