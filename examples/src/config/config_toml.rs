// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! TOML 配置文件示例
//!
//! 演示如何通过 TOML 字符串创建 [`DbConfig`] 并构建连接池：
//! - 编写 TOML 配置字符串
//! - 使用 `toml` crate 反序列化为 `DbConfig`
//! - 用配置创建 `DbPool`
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example config_toml
//! ```

use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("📝 DBNexus TOML configuration example");
    println!("========================================\n");

    // ============================================
    // 编写 TOML 配置字符串
    // ============================================
    // 生产中通常从 .toml 文件读取，这里为了演示使用内联字符串。
    let toml_content = r#"
url = "sqlite::memory:"
max_connections = 20
min_connections = 5
idle_timeout = 300
acquire_timeout = 5000
admin_role = "toml_admin"
auto_migrate = false
migration_timeout = 60

[cache_config]
policy_cache_capacity = 4096
sql_parse_cache_capacity = 1000
query_cache_capacity = 10000
default_ttl = 300
"#;

    println!("TOML configuration content:");
    println!("{}", toml_content);

    // ============================================
    // 解析 TOML 配置
    // ============================================
    let config: DbConfig = toml::from_str(toml_content)?;
    println!("✓ TOML configuration parsed");

    // ============================================
    // 打印配置信息
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
        "  - default_ttl (s)       : {}",
        config.cache_config.default_ttl
    );

    // ============================================
    // 创建连接池
    // ============================================
    let pool = DbPool::with_config(config).await?;
    println!("\n✓ Pool created");
    println!("  - actual URL       : {}", pool.config().url);
    println!("  - actual admin role: {}", pool.config().admin_role);

    let status = pool.status();
    println!("\n📊 Pool status:");
    println!("  - total connections: {}", status.total);
    println!("  - active connections: {}", status.active);
    println!("  - idle connections: {}", status.idle);

    println!("\n========================================");
    println!("✨ TOML configuration example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!(
        "  - DbConfig implements serde::Deserialize and can be deserialized from TOML directly"
    );
    println!("  - toml::from_str parses a TOML string into DbConfig");
    println!("  - the TOML [cache_config] section maps to the DbConfig.cache_config field");

    Ok(())
}
