// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存使用示例
//!
//! 展示如何使用 dbnexus 的缓存功能：
//! - 创建和管理缓存管理器
//! - 使用 LRU 缓存策略
//! - 配置 TTL 过期时间
//! - 防止缓存穿透和击穿
//! - 手动管理缓存
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example cache --features sqlite,cache
//! ```

use dbnexus::{DbConfig, DbPool};
use dbnexus::cache::{CacheConfig, CacheKey, CacheManager};
use std::time::Duration;

/// 定义 User 结构体（用于演示缓存）
#[derive(Debug, Clone, PartialEq)]
struct User {
    id: i64,
    name: String,
    email: String,
    role: String,
}

/// 定义 Product 结构体（用于演示不同查询类型的缓存）
#[derive(Debug, Clone, PartialEq)]
struct Product {
    id: i64,
    name: String,
    price: f64,
    category: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("💾 DBNexus 缓存使用示例\n");
    println!("========================================");

    // 1. 创建缓存配置
    println!("\n1️⃣ 创建缓存配置");
    println!("------------------------------------------");
    let cache_config = CacheConfig::default();
    println!("✓ 缓存配置创建成功");
    println!("  - 最大容量: {}", cache_config.max_capacity);
    println!("  - 默认 TTL: {} 秒", cache_config.default_ttl);
    println!("  - 清理间隔: {} 秒", cache_config.cleanup_interval);

    // 2. 创建缓存管理器
    println!("\n2️⃣ 创建缓存管理器");
    println!("------------------------------------------");
    let user_cache: CacheManager<User> = CacheManager::new(cache_config.clone());
    let product_cache: CacheManager<Product> = CacheManager::new(cache_config);
    println!("✓ 缓存管理器创建成功");

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
    let mut session = pool.get_session("admin").await?;

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

    // 创建 100 个用户用于测试
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

    // 创建产品数据
    let product_session = pool.get_session("admin").await?;
    for i in 1..=50 {
        let category = format!("Category{}", (i - 1) / 10 + 1);
        product_session
            .execute_raw(&format!(
                "INSERT INTO products (id, name, price, category) VALUES ({}, 'Product {}', {}, '{}')",
                i,
                i,
                i as f64 * 10.0,
                category
            ))
            .await?;
    }
    println!("  ✓ 创建 50 个测试产品");

    // 5. 手动缓存操作
    println!("\n5️⃣ 手动缓存操作");
    println!("------------------------------------------");

    // 设置缓存
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        role: "admin".to_string(),
    };
    let cache_key = CacheKey::new("users", "1");
    user_cache
        .set_with_ttl(cache_key.clone(), user.clone(), Duration::from_secs(300))
        .await;
    println!("  ✓ 设置缓存: users:1");

    // 获取缓存
    let cached = user_cache.get(&cache_key).await;
    match cached {
        Some(cached_user) => {
            println!("  ✓ 缓存命中: {}", cached_user.name);
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
        let missing_key = CacheKey::new("users", "99999");
        let _ = user_cache.get(&missing_key).await;
    }
    println!("  ✓ 多次尝试获取不存在的键（防止缓存穿透）");

    // 7. 演示 TTL 过期
    println!("\n7️⃣ TTL 过期演示");
    println!("------------------------------------------");

    // 设置一个短期过期的缓存
    let short_ttl_key = CacheKey::new("products", "1");
    let product = Product {
        id: 1,
        name: "Limited Product".to_string(),
        price: 99.99,
        category: "special".to_string(),
    };
    product_cache
        .set_with_ttl(short_ttl_key.clone(), product, Duration::from_secs(2))
        .await;
    println!("  ✓ 设置短期缓存（2秒 TTL）");

    // 立即获取（应该命中）
    let _ = product_cache.get(&short_ttl_key).await;
    println!("  ✓ 立即获取：缓存命中");

    // 等待过期
    println!("  ⏳ 等待缓存过期（2秒）...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 获取（应该未命中）
    let result = product_cache.get(&short_ttl_key).await;
    match result {
        Some(_) => println!("  ✗ 缓存未过期"),
        None => println!("  ✓ 缓存已过期（未命中）"),
    }

    // 8. 演示缓存删除
    println!("\n8️⃣ 缓存删除操作");
    println!("------------------------------------------");

    // 设置缓存
    let key = CacheKey::new("users", "2");
    let user = User {
        id: 2,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
        role: "user".to_string(),
    };
    user_cache
        .set_with_ttl(key.clone(), user.clone(), Duration::from_secs(300))
        .await;
    println!("  ✓ 设置缓存: users:2");

    // 删除缓存
    user_cache.delete(&key).await;
    println!("  ✓ 删除缓存");

    // 验证删除
    let result = user_cache.get(&key).await;
    match result {
        Some(_) => println!("  ✗ 缓存未删除"),
        None => println!("  ✓ 缓存已删除"),
    }

    // 9. 批量缓存操作
    println!("\n9️⃣ 批量缓存操作");
    println!("------------------------------------------");

    // 批量设置
    let mut batch_keys = Vec::new();
    let mut batch_items = Vec::new();
    for i in 3..=7 {
        let key = CacheKey::new("users", &i.to_string());
        let user = User {
            id: i,
            name: format!("User {}", i),
            email: format!("user{}@example.com", i),
            role: "user".to_string(),
        };
        batch_keys.push(key.clone());
        batch_items.push((key, user));
        println!("  ✓ 批量设置缓存: users:{}", i);
    }

    user_cache.batch_set(batch_items).await;

    // 批量获取
    let results = user_cache.batch_get(&batch_keys).await;
    println!(
        "  ✓ 批量获取缓存: {} / {} 命中",
        results.iter().filter(|r| r.is_some()).count(),
        batch_keys.len()
    );

    // 批量删除
    let deleted = user_cache.batch_delete(&batch_keys).await;
    println!("  ✓ 批量删除缓存: {} 个", deleted);

    // 10. 清理缓存
    println!("\n🔟 清理缓存");
    println!("------------------------------------------");

    // 清理所有过期条目
    let cleaned = user_cache.cleanup().await;
    println!("  ✓ 清理过期条目: {} 个", cleaned);

    // 11. 获取缓存统计
    println!("\n1️⃣1️⃣ 获取缓存统计");
    println!("------------------------------------------");

    let stats = user_cache.stats();
    println!("  📊 缓存统计:");
    println!("    - 命中率: {:.2}%", stats.hit_rate() * 100.0);

    println!("\n========================================");
    println!("✨ 缓存使用示例运行完成！");

    Ok(())
}
