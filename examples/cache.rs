//! 缓存使用示例
//!
//! 展示如何使用 dbnexus 的缓存功能：
//! - 创建和管理缓存管理器
//! - 使用 LRU 缓存策略
//! - 配置 TTL 过期时间
//! - 防止缓存穿透和击穿
//! - 使用 #[db_cache] 宏自动缓存
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example cache --features sqlite,cache
//! ```

use dbnexus::cache::{
    CacheConfig, CacheEntry, CacheKey, CacheManager, CacheStats, CacheStrategy,
};
use dbnexus::{DbPool, DbEntity, db_crud, db_cache};
use std::time::Duration;
use std::thread;

/// 定义 User Entity（带缓存支持）
///
/// #[db_cache] 宏自动为查询操作添加缓存
#[derive(DbEntity)]
#[db_entity]
#[table_name = "users")]
#[db_crud]
#[db_cache(ttl = 300, max_capacity = 1000)]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    email: String,
    role: String,
}

/// 定义 Product Entity（手动缓存管理）
#[derive(DbEntity)]
#[db_entity]
#[table_name = "products")]
#[db_crud]
struct Product {
    #[primary_key]
    id: i64,
    name: String,
    price: f64,
    category: String,
}

/// 自定义缓存策略：基于角色的缓存
struct RoleBasedCacheStrategy;

#[async_trait::async_trait]
impl CacheStrategy for RoleBasedCacheStrategy {
    fn name(&self) -> &'static str {
        "role_based"
    }

    fn ttl(&self) -> Duration {
        Duration::from_secs(600) // 10 分钟
    }

    fn on_hit(&self, _key: &CacheKey, _entry: &CacheEntry<User>) {
        // 缓存命中时的处理
    }

    fn on_miss(&self, _key: &CacheKey) {
        // 缓存未命中时的处理
    }

    fn should_cache(&self, _key: &CacheKey, _value: &User) -> bool {
        true
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("💾 DBNexus 缓存使用示例\n");
    println!("========================================");

    // 1. 创建缓存配置
    println!("\n1️⃣ 创建缓存配置");
    println!("------------------------------------------");
    let cache_config = CacheConfig::builder()
        .max_capacity(10000)
        .default_ttl(Duration::from_secs(300)) // 5 分钟
        .cleanup_interval(Duration::from_secs(60)) // 1 分钟清理一次
        .enable_stats(true)
        .build()?;
    println!("✓ 缓存配置创建成功");
    println!("  - 最大容量: {}", cache_config.max_capacity);
    println!("  - 默认 TTL: {} 秒", cache_config.default_ttl.as_secs());
    println!("  - 清理间隔: {} 秒", cache_config.cleanup_interval.as_secs());

    // 2. 创建缓存管理器
    println!("\n2️⃣ 创建缓存管理器");
    println!("------------------------------------------");
    let cache = CacheManager::new(cache_config);
    println!("✓ 缓存管理器创建成功");

    // 3. 初始化数据库连接池
    println!("\n3️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let pool = DbPool::new("sqlite::memory:").await?;
    println!("✓ 连接池创建成功");

    // 4. 创建测试数据
    println!("\n4️⃣ 创建测试数据");
    println!("------------------------------------------");
    let mut session = pool.get_session("admin").await?;

    // 创建 100 个用户用于测试
    for i in 1..=100 {
        User::insert(
            &mut session,
            User {
                id: i,
                name: format!("User {}", i),
                email: format!("user{}@example.com", i),
                role: if i <= 10 { "admin".to_string() } else { "user".to_string() },
            },
        )
        .await?;
    }
    println!("  ✓ 创建 100 个测试用户");

    // 创建产品数据
    for i in 1..=50 {
        Product::insert(
            &mut session,
            Product {
                id: i,
                name: format!("Product {}", i),
                price: i as f64 * 10.0,
                category: format!("Category{}", (i - 1) / 10 + 1),
            },
        )
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
    cache.set(&cache_key, user.clone(), Duration::from_secs(300)).await;
    println!("  ✓ 设置缓存: {}", cache_key.key);

    // 获取缓存
    let cached = cache.get::<User>(&cache_key).await;
    match cached {
        Some(cached_user) => {
            println!("  ✓ 缓存命中: {}", cached_user.name);
        }
        None => {
            println!("  ✗ 缓存未命中");
        }
    }

    // 获取缓存统计
    let stats = cache.get_stats().await;
    println!("  📊 缓存统计:");
    println!("    - 命中次数: {}", stats.hits);
    println!("    - 未命中次数: {}", stats.misses);
    println!("    - 命中率: {:.2}%", stats.hit_rate() * 100.0);

    // 6. 演示缓存穿透防护
    println!("\n6️⃣ 缓存穿透防护");
    println!("------------------------------------------");

    // 尝试获取不存在的键（多次尝试）
    for i in 0..5 {
        let missing_key = CacheKey::new("users", "99999");
        let _ = cache.get::<User>(&missing_key).await;
    }
    println!("  ✓ 多次尝试获取不存在的键（防止缓存穿透）");

    // 检查缓存统计
    let stats = cache.get_stats().await;
    println!("  📊 缓存统计（穿透防护后）:");
    println!("    - 总访问次数: {}", stats.hits + stats.misses);
    println!("    - 命中率: {:.2}%", stats.hit_rate() * 100.0);

    // 7. 演示缓存击穿防护
    println!("\n7️⃣ 缓存击穿防护");
    println!("------------------------------------------");

    // 使用互斥锁保护热点数据的缓存重建
    let hot_key = CacheKey::new("users", "1");
    let cache = Arc::new(cache);

    // 并发请求同一热点数据
    let mut handles = Vec::new();
    for i in 0..10 {
        let cache = cache.clone();
        let hot_key = hot_key.clone();
        let handle = tokio::spawn(async move {
            // 使用 get_or_insert_with 防止缓存击穿
            let user = cache
                .get_or_insert_with(&hot_key, || async {
                    // 模拟数据库查询
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    User {
                        id: 1,
                        name: format!("User {}", i),
                        email: format!("user{}@example.com", i),
                        role: "admin".to_string(),
                    }
                })
                .await;
            user
        });
        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;
    println!("  ✓ 并发请求热点数据（防止缓存击穿）");
    println!("  📊 成功获取: {} 个结果", results.len());

    // 8. 演示 TTL 过期
    println!("\n8️⃣ TTL 过期演示");
    println!("------------------------------------------");

    // 设置一个短期过期的缓存
    let short_ttl_key = CacheKey::new("products", "1");
    let product = Product {
        id: 1,
        name: "Limited Product".to_string(),
        price: 99.99,
        category: "special".to_string(),
    };
    cache
        .set(&short_ttl_key, product, Duration::from_secs(2))
        .await;
    println!("  ✓ 设置短期缓存（2秒 TTL）");

    // 立即获取（应该命中）
    let _ = cache.get::<Product>(&short_ttl_key).await;
    println!("  ✓ 立即获取：缓存命中");

    // 等待过期
    println!("  ⏳ 等待缓存过期（2秒）...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 获取（应该未命中）
    let result = cache.get::<Product>(&short_ttl_key).await;
    match result {
        Some(_) => println!("  ✗ 缓存未过期"),
        None => println!("  ✓ 缓存已过期（未命中）"),
    }

    // 9. 演示缓存失效
    println!("\n9️⃣ 缓存失效操作");
    println!("------------------------------------------");

    // 设置缓存
    let key = CacheKey::new("users", "2");
    let user = User {
        id: 2,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
        role: "user".to_string(),
    };
    cache.set(&key, user.clone(), Duration::from_secs(300)).await;
    println!("  ✓ 设置缓存: {}", key.key);

    // 使缓存失效
    cache.invalidate(&key).await;
    println!("  ✓ 使缓存失效");

    // 验证失效
    let result = cache.get::<User>(&key).await;
    match result {
        Some(_) => println!("  ✗ 缓存未失效"),
        None => println!("  ✓ 缓存已失效"),
    }

    // 10. 批量缓存操作
    println!("\n🔟 批量缓存操作");
    println!("------------------------------------------");

    // 批量设置
    let mut batch_keys = Vec::new();
    for i in 3..=7 {
        let key = CacheKey::new("users", &i.to_string());
        let user = User {
            id: i,
            name: format!("User {}", i),
            email: format!("user{}@example.com", i),
            role: "user".to_string(),
        };
        cache.set(&key, user, Duration::from_secs(300)).await;
        batch_keys.push(key);
        println!("  ✓ 批量设置缓存: {}", key.key);
    }

    // 批量获取
    let results = cache.get_many::<User>(&batch_keys).await;
    println!("  ✓ 批量获取缓存: {} / {} 命中", results.len(), batch_keys.len());

    // 批量失效
    cache.invalidate_many(&batch_keys).await;
    println!("  ✓ 批量失效缓存");

    // 11. 清理缓存
    println!("\n1️⃣1️⃣ 清理缓存");
    println!("------------------------------------------");

    // 清理所有过期条目
    let cleaned = cache.cleanup().await;
    println!("  ✓ 清理过期条目: {} 个", cleaned);

    // 清空所有缓存
    cache.clear().await;
    println!("  ✓ 清空所有缓存");

    // 最终统计
    let stats = cache.get_stats().await;
    println!("\n📊 最终缓存统计:");
    println!("  - 命中次数: {}", stats.hits);
    println!("  - 未命中次数: {}", stats.misses);
    println!("  - 命中率: {:.2}%", stats.hit_rate() * 100.0);

    println!("\n========================================");
    println!("✨ 缓存使用示例运行完成！");

    Ok(())
}
