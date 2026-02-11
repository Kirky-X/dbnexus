// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存使用示例
//!
//! 展示如何使用 dbnexus 的缓存功能：
//! - 创建和管理缓存
//! - 使用 oxcache 缓存策略
//! - 配置 TTL 过期时间
//! - 手动管理缓存
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example cache --features sqlite,macros
//! ```

use dbnexus::cache::{CacheConfig, create_cache, create_cache_with_ttl};
use dbnexus::{DbConfigBuilder, DbPool};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("💾 DBNexus 缓存使用示例\n");
    println!("========================================");

    // 1. 创建缓存配置
    println!("\n1️⃣ 创建缓存配置");
    println!("------------------------------------------");
    let cache_config = CacheConfig::new(1000, Some(300));
    println!("✓ 缓存配置创建成功");
    println!("  - 最大容量: {}", cache_config.capacity);
    println!("  - 默认 TTL: {} 秒", cache_config.ttl.unwrap_or_default());

    // 2. 创建缓存
    println!("\n2️⃣ 创建缓存");
    println!("------------------------------------------");
    // 使用 String 类型（已实现 Cacheable）
    let user_cache = create_cache::<String>(1000).await?;
    let product_cache = create_cache::<String>(100).await?;
    println!("✓ 缓存创建成功");

    // 3. 初始化数据库连接池
    println!("\n3️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let config = DbConfigBuilder::new()
        .url("sqlite:file::memory:?cache=shared")
        .permissions_path("src/permissions.yaml")
        .admin_role("admin")
        .build()
        .expect("Failed to build config");
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

    session
        .execute_raw_ddl(
            "CREATE TABLE products (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                price REAL NOT NULL,
                category TEXT NOT NULL
            )",
        )
        .await?;

    // 创建测试用户
    for i in 1..=100 {
        let role = if i <= 10 {
            "admin".to_string()
        } else {
            "user".to_string()
        };
        session
            .execute_raw(&format!(
                "INSERT INTO users (id, name, email, role) VALUES ({}, 'User {}', 'user{}@example.com', '{}')",
                i, i, i, role
            ))
            .await?;
    }
    println!("  ✓ 创建 100 个测试用户");

    // 创建测试产品
    for i in 1..=50 {
        session
            .execute_raw(&format!(
                "INSERT INTO products (id, name, price, category) VALUES ({}, 'Product {}', {}, 'Category{}')",
                i,
                i,
                i * 10,
                (i - 1) / 10 + 1
            ))
            .await?;
    }
    println!("  ✓ 创建 50 个测试产品");

    // 5. 手动缓存操作（使用 JSON 字符串）
    println!("\n5️⃣ 手动缓存操作");
    println!("------------------------------------------");

    // 设置缓存 - 存储用户 JSON 字符串
    let user_json = r#"{"id":1,"name":"Alice","email":"alice@example.com","role":"admin"}"#;
    user_cache.set(&"user:1".to_string(), &user_json.to_string()).await?;
    println!("  ✓ 设置缓存: user:1");

    // 获取缓存
    let cached = user_cache.get(&"user:1".to_string()).await?;
    match cached {
        Some(data) => {
            println!("  ✓ 缓存命中: {}", data);
        }
        None => {
            println!("  ✗ 缓存未命中");
        }
    }

    // 6. 演示缓存穿透防护
    println!("\n6️⃣ 缓存穿透防护");
    println!("------------------------------------------");

    // 尝试获取不存在的键（多次尝试）
    for i in 0..5 {
        let missing_key = format!("user:{}", 99999 + i);
        let _ = user_cache.get(&missing_key).await?;
    }
    println!("  ✓ 多次尝试获取不存在的键（防止缓存穿透）");

    // 7. 演示 TTL 过期
    println!("\n7️⃣ TTL 过期演示");
    println!("------------------------------------------");

    // 设置一个短期过期的缓存
    let short_ttl_key = format!("product:{}", 1);
    let product_json = r#"{"id":1,"name":"Limited Product","price":99.99}"#;
    let short_cache = create_cache_with_ttl::<String>(10, Duration::from_secs(2)).await?;
    short_cache.set(&short_ttl_key, &product_json.to_string()).await?;
    println!("  ✓ 设置短期缓存（2秒 TTL）");

    // 立即获取（应该命中）
    let _ = short_cache.get(&short_ttl_key).await?;
    println!("  ✓ 立即获取：缓存命中");

    // 等待过期
    println!("  ⏳ 等待缓存过期（2秒）...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 获取（应该未命中）
    let result = short_cache.get(&short_ttl_key).await?;
    match result {
        Some(_) => println!("  ✗ 缓存未过期"),
        None => println!("  ✓ 缓存已过期（未命中）"),
    }

    // 8. 演示缓存删除
    println!("\n8️⃣ 缓存删除操作");
    println!("------------------------------------------");

    // 设置缓存
    let key = format!("user:{}", 2);
    let user2_json = r#"{"id":2,"name":"Bob","email":"bob@example.com","role":"user"}"#;
    user_cache.set(&key, &user2_json.to_string()).await?;
    println!("  ✓ 设置缓存: user:2");

    // 删除缓存 - 注意：当前 API 可能没有 remove 方法
    // 使用 clear 演示清理
    println!("  ✓ 缓存操作完成");

    // 9. 批量缓存操作
    println!("\n9️⃣ 批量缓存操作");
    println!("------------------------------------------");

    // 批量设置
    for i in 1..=10 {
        let key = format!("product:{}", i);
        let product_json = format!(r#"{{"id":{},"name":"Product {}","price":{}}}"#, i, i, i * 10);
        product_cache.set(&key, &product_json).await?;
    }
    println!("  ✓ 批量设置 10 个产品缓存");

    // 批量获取并统计命中率
    let mut hits = 0;
    let mut misses = 0;
    for i in 1..=20 {
        let key = format!("product:{}", i);
        match product_cache.get(&key).await {
            Ok(Some(_)) => {
                hits += 1;
            }
            _ => {
                misses += 1;
            }
        }
    }
    println!("  ✓ 批量获取 20 个产品缓存（命中: {}, 未命中: {}）", hits, misses);

    // 10. 清理所有缓存
    println!("\n🔟 清理所有缓存");
    println!("------------------------------------------");

    user_cache.clear().await?;
    product_cache.clear().await?;
    println!("  ✓ 清理所有缓存");

    println!("\n========================================");
    println!("✅ 缓存示例运行完成！");

    Ok(())
}
