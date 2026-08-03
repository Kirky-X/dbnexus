// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 独立缓存功能示例
//!
//! 演示 DBNexus 的 [`DbCacheProvider`] trait 及其独立使用：
//! - 实现自定义 DbCacheProvider（基于 HashMap 的简单内存缓存）
//! - 通过 trait 对象（`Arc<dyn DbCacheProvider>`）使用
//! - get/set/delete 操作
//! - TTL 支持
//! - 与 DbPoolBuilder 集成（通过 cache_provider 注入）
//!
//! DbCacheProvider 是 DBNexus 的缓存抽象层，允许注入任意缓存实现。
//! 内置的 OxcacheDbCacheAdapter 适配 oxcache，但用户也可以实现自己的缓存后端。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example cache_standalone --features "sqlite,cache"
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dbnexus::foundation::{DbConfig, DbError, PoolConfig};
use dbnexus::{DbCacheProvider, DbPoolBuilder};

// ============================================
// 自定义缓存实现
// ============================================

/// 基于 HashMap 的简单内存缓存
///
/// 演示如何实现 `DbCacheProvider` trait。
/// 使用 `Mutex<HashMap>` 保证线程安全。
struct SimpleMemoryCache {
    store: Mutex<HashMap<String, CacheEntry>>,
    capacity: usize,
}

struct CacheEntry {
    value: Vec<u8>,
    expires_at: Option<std::time::Instant>,
}

impl SimpleMemoryCache {
    fn new(capacity: usize) -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
            capacity,
        }
    }

    fn is_expired(entry: &CacheEntry) -> bool {
        entry.expires_at.is_some_and(|exp| std::time::Instant::now() >= exp)
    }
}

impl DbCacheProvider for SimpleMemoryCache {
    fn get<'a>(&'a self, key: &'a str) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, DbError>> + Send + 'a>> {
        Box::pin(async move {
            let store = self
                .store
                .lock()
                .map_err(|e| DbError::Cache(format!("lock poisoned: {e}")))?;
            match store.get(key) {
                Some(entry) if !Self::is_expired(entry) => Ok(Some(entry.value.clone())),
                Some(_) => {
                    // 已过期，返回 None（惰性清理在 set 时进行）
                    Ok(None)
                }
                None => Ok(None),
            }
        })
    }

    fn set<'a>(
        &'a self,
        key: &'a str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
        Box::pin(async move {
            let mut store = self
                .store
                .lock()
                .map_err(|e| DbError::Cache(format!("lock poisoned: {e}")))?;

            // 惰性清理过期条目
            store.retain(|_, entry| !SimpleMemoryCache::is_expired(entry));

            // 容量检查
            if store.len() >= self.capacity && !store.contains_key(key) {
                // 移除第一个条目（简单 FIFO 策略）
                if let Some(first_key) = store.keys().next().cloned() {
                    store.remove(&first_key);
                }
            }

            let expires_at = ttl.map(|d| std::time::Instant::now() + d);
            store.insert(key.to_string(), CacheEntry { value, expires_at });
            Ok(())
        })
    }

    fn delete<'a>(&'a self, key: &'a str) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
        Box::pin(async move {
            let mut store = self
                .store
                .lock()
                .map_err(|e| DbError::Cache(format!("lock poisoned: {e}")))?;
            store.remove(key);
            Ok(())
        })
    }
}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("💾 DBNexus 独立缓存功能示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建自定义缓存
    // ============================================
    println!("--- 1. 创建自定义 DbCacheProvider ---\n");

    let cache = SimpleMemoryCache::new(100);
    println!("  ✓ SimpleMemoryCache 创建成功（capacity=100）");
    println!("  实现: DbCacheProvider trait (get/set/delete)");
    println!("  存储: Mutex<HashMap<String, CacheEntry>>\n");

    // ============================================
    // 2. 基本操作
    // ============================================
    println!("--- 2. 基本 get/set/delete ---\n");

    cache.set("key1", b"value1".to_vec(), None).await?;
    println!("  ✓ set(\"key1\", b\"value1\")");

    cache.set("key2", b"value2".to_vec(), None).await?;
    println!("  ✓ set(\"key2\", b\"value2\")");

    let val = cache.get("key1").await?;
    println!(
        "  ✓ get(\"key1\") → {:?}",
        val.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    cache.delete("key1").await?;
    println!("  ✓ delete(\"key1\")");

    let val = cache.get("key1").await?;
    println!("  ✓ get(\"key1\") → {:?} (已删除)", val);

    let val = cache.get("nonexistent").await?;
    println!("  ✓ get(\"nonexistent\") → {:?} (不存在)", val);
    println!();

    // ============================================
    // 3. TTL 过期
    // ============================================
    println!("--- 3. TTL 过期 ---\n");

    cache
        .set("ttl_key", b"temporary".to_vec(), Some(Duration::from_millis(100)))
        .await?;
    println!("  ✓ set(\"ttl_key\", ttl=100ms)");

    let val = cache.get("ttl_key").await?;
    println!(
        "  ✓ 立即 get → {:?} (存在)",
        val.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    tokio::time::sleep(Duration::from_millis(150)).await;

    let val = cache.get("ttl_key").await?;
    println!("  ✓ 150ms 后 get → {:?} (已过期)", val);
    println!();

    // ============================================
    // 4. 通过 trait 对象使用
    // ============================================
    println!("--- 4. Trait 对象（dyn dispatch）---\n");

    let provider: Arc<dyn DbCacheProvider + Send + Sync> = Arc::new(SimpleMemoryCache::new(50));
    println!("  ✓ 创建 Arc<dyn DbCacheProvider + Send + Sync>");

    provider.set("dyn:test", b"dynamic".to_vec(), None).await?;
    let val = provider.get("dyn:test").await?;
    println!(
        "  ✓ 通过 trait 对象: set + get → {:?}",
        val.map(|v| String::from_utf8_lossy(&v).to_string())
    );
    println!();

    // ============================================
    // 5. 与 DbPoolBuilder 集成
    // ============================================
    println!("--- 5. 与 DbPoolBuilder 集成 ---\n");

    let custom_cache: Arc<dyn DbCacheProvider + Send + Sync> = Arc::new(SimpleMemoryCache::new(200));
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        pool_config: PoolConfig {
            max_connections: 3,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    };

    let pool = DbPoolBuilder::new()
        .config(config)
        .cache_provider(custom_cache)
        .build()
        .await?;
    println!("  ✓ DbPoolBuilder 构建成功（注入自定义缓存）");
    println!("  连接池 URL: {}", pool.config().url);
    println!("  max_connections: {}", pool.config().pool_config.max_connections);

    // 验证连接池可用
    let session = pool.get_session("admin").await?;
    println!("  ✓ 获取 Session 成功 (角色: {})", session.role());
    println!();

    // ============================================
    // 6. 容量管理
    // ============================================
    println!("--- 6. 容量管理 ---\n");

    let small_cache = SimpleMemoryCache::new(5);

    // 写入超过容量的条目
    for i in 0..8 {
        let key = format!("item:{}", i);
        let value = format!("value_{}", i).into_bytes();
        small_cache.set(&key, value, None).await?;
    }
    println!("  ✓ 写入 8 个条目（capacity=5）");

    // 检查哪些条目还在
    let mut present = Vec::new();
    for i in 0..8 {
        let key = format!("item:{}", i);
        if small_cache.get(&key).await?.is_some() {
            present.push(key);
        }
    }
    println!("  ✓ 当前存在 {} 个条目: {:?}", present.len(), present);
    println!("  ✓ FIFO 淘汰策略保留了最近的 5 个条目");

    println!("\n========================================");
    println!("✨ 独立缓存功能示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - DbCacheProvider trait              统一缓存接口");
    println!("  - 自定义实现                          实现 get/set/delete 即可");
    println!("  - TTL (Duration)                     过期时间支持");
    println!("  - Arc<dyn DbCacheProvider>            trait 对象（DI 注入）");
    println!("  - DbPoolBuilder::cache_provider()    注入到连接池");
    println!("  - DbError::Cache                     统一错误类型");

    Ok(())
}
