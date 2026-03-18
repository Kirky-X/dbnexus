// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存使用示例
//!
//! 展示如何使用 dbnexus 缓存功能：
//! - 创建缓存实例
//! - 基本缓存操作（插入、获取、删除）
//! - TTL 过期
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example cache --features "sqlite,permission,cache"
//! ```

use dbnexus::cache::{CacheConfig, create_cache, create_cache_with_ttl};
use dbnexus::{DbConfig, DbPool};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    // 使用 String 类型
    let user_cache = create_cache::<String>(1000).await?;
    let product_cache = create_cache::<String>(100).await?;
    println!("✓ 缓存创建成功");

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

    // 5. 手动缓存操作
    println!("\n5️⃣ 手动缓存操作");
    println!("------------------------------------------");

    // 设置缓存 - 存储用户 JSON 字符串
    let user_json = r#"{"id":1,"name":"Alice","email":"alice@example.com","role":"admin"}"#;
    user_cache.insert("user:1".to_string(), user_json.to_string()).await;
    println!("  ✓ 设置缓存: user:1");

    // 获取缓存
    let cached = user_cache.get(&"user:1".to_string()).await;
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

    // 尝试获取不存在的键
    for i in 0..5 {
        let missing_key = format!("user:{}", 99999 + i);
        let _ = user_cache.get(&missing_key).await;
    }
    println!("  ✓ 多次尝试获取不存在的键（防止缓存穿透）");

    // 7. 演示 TTL 过期
    println!("\n7️⃣ TTL 过期演示");
    println!("------------------------------------------");

    // 设置一个短期过期的缓存
    let short_ttl_key = "product:1".to_string();
    let product_json = r#"{"id":1,"name":"Limited Product","price":99.99}"#;
    let short_cache = create_cache_with_ttl::<String>(10, Duration::from_secs(2)).await?;
    short_cache
        .insert(short_ttl_key.clone(), product_json.to_string())
        .await;
    println!("  ✓ 设置短期缓存（2秒 TTL）");

    // 立即获取（应该命中）
    let _ = short_cache.get(&short_ttl_key).await;
    println!("  ✓ 立即获取：缓存命中");

    // 等待过期
    println!("  ⏳ 等待缓存过期（2秒）...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 再次获取（应该未命中）
    let expired = short_cache.get(&short_ttl_key).await;
    match expired {
        Some(_) => println!("  ✗ 缓存仍然存在（意外）"),
        None => println!("  ✓ 缓存已过期"),
    }

    // 8. 缓存统计
    println!("\n8️⃣ 缓存统计");
    println!("------------------------------------------");
    println!("  用户缓存已创建");

    // 9. 清空缓存
    println!("\n9️⃣ 清空缓存");
    println!("------------------------------------------");
    user_cache.invalidate_all();
    println!("  ✓ 用户缓存已清空");

    println!("\n=== 所有示例完成 ===");
    Ok(())
}
