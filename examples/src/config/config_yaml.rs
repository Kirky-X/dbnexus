// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! YAML 配置文件示例
//!
//! 演示如何通过 YAML 字符串创建 [`DbConfig`] 并构建连接池：
//! - 编写 YAML 配置字符串
//! - 使用 `serde-saphyr` 反序列化为 `DbConfig`
//! - 用配置创建 `DbPool`
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example config_yaml --features "yaml"
//! ```

use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("📄 DBNexus YAML configuration example");
    println!("========================================\n");

    // 编写 YAML 配置字符串
    // 生产中通常从文件读取，这里为了演示使用内联字符串。
    // DbConfig 的所有字段都实现了 serde，可直接反序列化。
    let yaml_content = r#"
url: "sqlite::memory:"
max_connections: 15
min_connections: 3
idle_timeout: 120
acquire_timeout: 8000
admin_role: "dbadmin"
auto_migrate: false
migration_timeout: 30

cache_config:
  policy_cache_capacity: 8192
  sql_parse_cache_capacity: 2000
  query_cache_capacity: 20000
  default_ttl: 600
"#;

    println!("YAML configuration content:");
    println!("{}", yaml_content);

    // 解析 YAML 配置
    let config: DbConfig = serde_saphyr::from_str(yaml_content)?;
    println!("✓ YAML configuration parsed");

    // 打印配置信息
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

    // 创建连接池
    let pool = DbPool::with_config(config).await?;
    println!("\n✓ Pool created");
    println!("  - actual URL       : {}", pool.config().url);
    println!(
        "  - actual max connections: {}",
        pool.config().pool_config.max_connections
    );

    let status = pool.status();
    println!("\n📊 Pool status:");
    println!("  - total connections: {}", status.total);
    println!("  - active connections: {}", status.active);
    println!("  - idle connections: {}", status.idle);

    println!("\n========================================");
    println!("✨ YAML configuration example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!(
        "  - DbConfig implements serde::Deserialize and can be deserialized from YAML directly"
    );
    println!("  - serde_saphyr::from_str parses a YAML string into DbConfig");
    println!("  - DbPool::with_config creates a pool from DbConfig");

    Ok(())
}
