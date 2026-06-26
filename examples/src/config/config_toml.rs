// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

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
//! cargo run --example config_toml --features "config-toml"
//! ```

use dbnexus::{DbConfig, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("📝 DBNexus TOML 配置示例");
    println!("========================================\n");

    // ============================================
    // 1. 编写 TOML 配置字符串
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

    println!("TOML 配置内容:");
    println!("{}", toml_content);

    // ============================================
    // 2. 解析 TOML 配置
    // ============================================
    let config: DbConfig = toml::from_str(toml_content)?;
    println!("✓ TOML 配置解析成功");

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
    println!("  - database_type     : {}", config.database_type());

    println!("\n💾 缓存配置:");
    println!("  - policy_cache_capacity : {}", config.cache_config.policy_cache_capacity);
    println!("  - default_ttl (s)       : {}", config.cache_config.default_ttl);

    // ============================================
    // 4. 创建连接池
    // ============================================
    let pool = DbPool::with_config(config).await?;
    println!("\n✓ 连接池创建成功");
    println!("  - 实际 URL      : {}", pool.config().url);
    println!("  - 实际管理员角色: {}", pool.config().admin_role);

    let status = pool.status();
    println!("\n📊 连接池状态:");
    println!("  - 总连接数: {}", status.total);
    println!("  - 活跃连接: {}", status.active);
    println!("  - 空闲连接: {}", status.idle);

    println!("\n========================================");
    println!("✨ TOML 配置示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - DbConfig 实现 serde::Deserialize，可直接从 TOML 反序列化");
    println!("  - toml::from_str 解析 TOML 字符串为 DbConfig");
    println!("  - TOML 的 [cache_config] 段对应 DbConfig.cache_config 字段");

    Ok(())
}
