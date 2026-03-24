// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存使用示例
//!
//! 展示如何使用 dbnexus 缓存功能：
//! - 创建缓存实例 (OxcacheBackend)
//! - 基本缓存操作（插入、获取、删除）
//! - TTL 过期
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example cache --features "sqlite,permission,cache"
//! ```

use dbnexus::cache::{CacheBackend, OxcacheBackend};
use dbnexus::{DbConfig, DbPool};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("💾 DBNexus 缓存使用示例\n");
    println!("========================================");

    // 1. 创建缓存实例
    println!("\n1️⃣ 创建缓存实例");
    println!("------------------------------------------");
    let user_cache = OxcacheBackend::with_capacity(1000).await?;
    println!("✓ 缓存创建成功");
    println!("  - 容量: 1000 条");

    // 2. 创建产品缓存
    println!("\n2️⃣ 创建产品缓存");
    println!("------------------------------------------");
    let _product_cache = OxcacheBackend::with_capacity(100).await?;
    println!("✓ 产品缓存创建成功");

    // 3. 初始化数据库连接池
    println!("\n3️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        permissions_path: Some("examples/demo/permissions.yaml".to_string()),
        admin_role: "admin".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // 4. 创建测试数据
    println!("\n4️⃣ 创建测试数据");
    println!("------------------------------------------");
    let session = pool.get_session("admin").await?;

    // 创建表
    session
        .execute_raw_ddl(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL,
                role TEXT NOT NULL
            )",
        )
        .await?;

    // 创建测试用户
    for i in 1..=10 {
        let role = if i <= 2 { "admin" } else { "user" };
        session
            .execute_raw(&format!(
                "INSERT INTO users (id, name, email, role) VALUES ({}, 'User {}', 'user{}@example.com', '{}')",
                i, i, i, role
            ))
            .await?;
    }
    println!("  ✓ 创建 10 个测试用户");

    // 5. 基本缓存操作
    println!("\n5️⃣ 基本缓存操作");
    println!("------------------------------------------");

    // 设置缓存 - 存储用户 JSON 字符串
    let user_json = r#"{"id":1,"name":"Alice","email":"alice@example.com","role":"admin"}"#;
    user_cache.set("user:1", user_json.to_string(), None).await?;
    println!("  ✓ 设置缓存: user:1");

    // 获取缓存
    let cached = user_cache.get("user:1").await;
    match cached {
        Some(data) => {
            println!("  ✓ 缓存命中: {}", data);
        }
        None => {
            println!("  ✗ 缓存未命中");
        }
    }

    // 6. 演示缓存穿透防护
    println!("\n6️⃣ 演示缓存穿透防护");
    println!("------------------------------------------");

    // 尝试获取不存在的键
    for i in 0..5 {
        let missing_key = format!("user:{}", 99999 + i);
        let _ = user_cache.get(&missing_key).await;
    }
    println!("  ✓ 多次尝试获取不存在的键（防止缓存穿透）");

    // 7. 演示 TTL 过期
    println!("\n7️⃣ TTL 过期演示");
    println!("------------------------------------------");

    // 设置一个带 TTL 的缓存
    let short_ttl_key = "product:1";
    let product_json = r#"{"id":1,"name":"Limited Product","price":99.99}"#;
    // 注意：当前 oxcache TTL 需要缓存实现支持
    user_cache.set(short_ttl_key, product_json.to_string(), Some(Duration::from_secs(2))).await?;
    println!("  ✓ 设置带 TTL 的缓存（2秒）");

    // 立即获取（应该命中）
    let cached = user_cache.get(short_ttl_key).await;
    match cached {
        Some(_) => println!("  ✓ 立即获取：缓存命中"),
        None => println!("  ✗ 立即获取：缓存未命中"),
    }

    // 等待过期
    println!("  ⏳ 等待缓存过期（3秒）...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 再次获取（可能未命中，取决于 TTL 实现）
    let expired = user_cache.get(short_ttl_key).await;
    match expired {
        Some(_) => println!("  ⚠️  缓存仍然存在（TTL 未生效）"),
        None => println!("  ✓ 缓存已过期"),
    }

    // 8. 缓存存在性检查
    println!("\n8️⃣ 缓存存在性检查");
    println!("------------------------------------------");

    let exists = user_cache.exists("user:1").await;
    println!("  user:1 存在: {}", exists);

    let exists = user_cache.exists("user:99999").await;
    println!("  user:99999 存在: {}", exists);

    // 9. 删除缓存
    println!("\n9️⃣ 删除缓存");
    println!("------------------------------------------");

    user_cache.delete("user:1").await?;
    println!("  ✓ 删除缓存: user:1");

    let cached = user_cache.get("user:1").await;
    match cached {
        Some(_) => println!("  ✗ user:1 仍然存在"),
        None => println!("  ✓ user:1 已删除"),
    }

    println!("\n========================================");
    println!("✨ 缓存使用示例运行完成！");
    println!("========================================\n");

    println!("💡 OxcacheBackend API:");
    println!("  - OxcacheBackend::with_capacity(n) - 创建指定容量的缓存");
    println!("  - cache.get(key)                   - 获取缓存值");
    println!("  - cache.set(key, value, ttl)      - 设置缓存值");
    println!("  - cache.delete(key)                - 删除缓存");
    println!("  - cache.exists(key)                - 检查缓存是否存在");

    Ok(())
}
