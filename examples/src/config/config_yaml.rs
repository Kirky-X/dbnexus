// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! YAML 配置文件示例
//!
//! 演示如何通过 YAML 字符串创建 [`DbConfig`] 并构建连接池：
//! - 编写 YAML 配置字符串
//! - 使用 `serde_yaml_ng` 反序列化为 `DbConfig`
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
    println!("📄 DBNexus YAML 配置示例");
    println!("========================================\n");

    // ============================================
    // 1. 编写 YAML 配置字符串
    // ============================================
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

    println!("YAML 配置内容:");
    println!("{}", yaml_content);

    // ============================================
    // 2. 解析 YAML 配置
    // ============================================
    let config: DbConfig = serde_yaml_ng::from_str(yaml_content)?;
    println!("✓ YAML 配置解析成功");

    // ============================================
    // 3. 打印配置信息
    // ============================================
    println!("\n📋 解析后的 DbConfig:");
    println!("  - url              : {}", config.url);
    println!("  - max_connections   : {}", config.max_connections);
    println!("  - min_connections   : {}", config.min_connections);
    println!("  - idle_timeout (s)  : {}", config.idle_timeout);
    println!("  - acquire_timeout(ms): {}", config.acquire_timeout);
    println!("  - admin_role        : {}", config.admin_role);
    println!("  - database_type     : {}", config.database_type().unwrap());

    // ============================================
    // 4. 创建连接池
    // ============================================
    let pool = DbPool::with_config(config).await?;
    println!("\n✓ 连接池创建成功");
    println!("  - 实际 URL      : {}", pool.config().url);
    println!("  - 实际最大连接数: {}", pool.config().max_connections);

    let status = pool.status();
    println!("\n📊 连接池状态:");
    println!("  - 总连接数: {}", status.total);
    println!("  - 活跃连接: {}", status.active);
    println!("  - 空闲连接: {}", status.idle);

    println!("\n========================================");
    println!("✨ YAML 配置示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - DbConfig 实现 serde::Deserialize，可直接从 YAML 反序列化");
    println!("  - serde_yaml_ng::from_str 解析 YAML 字符串为 DbConfig");
    println!("  - DbPool::with_config 使用 DbConfig 创建连接池");

    Ok(())
}
