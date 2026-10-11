// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Oxcache 适配器示例
//!
//! 演示 [`OxcacheDbCacheAdapter`] 将 oxcache 缓存后端适配为
//! DBNexus 的 [`DbCacheProvider`] trait：
//! - 使用 MokaMemoryBackend 创建适配器
//! - 通过 DbCacheProvider trait 执行 get/set/delete
//! - TTL（过期时间）支持
//! - 通过 trait 对象（`Arc<dyn DbCacheProvider>`）使用
//! - 错误处理（DbError::Cache）
//!
//! OxcacheDbCacheAdapter 是 oxcache 与 DBNexus 之间的桥接层，
//! 使 DBNexus 内部可以通过统一的 DbCacheProvider 接口使用 oxcache 缓存。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example oxcache_adapter --features "oxcache-integration"
//! ```

use std::sync::Arc;
use std::time::Duration;

use dbnexus::DbCacheProvider;
use dbnexus::OxcacheDbCacheAdapter;
use dbnexus::foundation::DbError;
use oxcache::backend::{CacheBackend, MokaMemoryBackend};

// ============================================
// 辅助函数
// ============================================

/// 创建基于 MokaMemoryBackend 的 OxcacheDbCacheAdapter
fn make_adapter(capacity: u64) -> OxcacheDbCacheAdapter {
    let backend = MokaMemoryBackend::builder().capacity(capacity).build();
    let cache: Arc<dyn CacheBackend + Send + Sync> = Arc::new(backend);
    OxcacheDbCacheAdapter::new(cache)
}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔗 DBNexus Oxcache adapter example");
    println!("========================================\n");

    // ============================================
    // 创建适配器
    // ============================================
    println!("--- 1. create the OxcacheDbCacheAdapter ---\n");

    let adapter = make_adapter(100);
    println!("  ✓ adapter created (MokaMemoryBackend, capacity=100)");
    println!("  type: OxcacheDbCacheAdapter");
    println!("  implementation: DbCacheProvider trait (get/set/delete)\n");

    // ============================================
    // 基本 get/set/delete 操作
    // ============================================
    println!("--- 2. basic CRUD operations ---\n");

    // set
    adapter.set("user:1", b"Alice".to_vec(), None).await?;
    println!("  ✓ set(\"user:1\", b\"Alice\")");

    adapter.set("user:2", b"Bob".to_vec(), None).await?;
    println!("  ✓ set(\"user:2\", b\"Bob\")");

    // get
    let val = adapter.get("user:1").await?;
    println!(
        "  ✓ get(\"user:1\") → {:?}",
        val.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    let val = adapter.get("user:2").await?;
    println!(
        "  ✓ get(\"user:2\") → {:?}",
        val.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    // get 不存在的 key
    let val = adapter.get("user:999").await?;
    println!(
        "  ✓ get(\"user:999\") → {:?} (returns None when missing)",
        val
    );

    // delete
    adapter.delete("user:1").await?;
    println!("  ✓ delete(\"user:1\")");

    let val = adapter.get("user:1").await?;
    println!(
        "  ✓ get(\"user:1\") → {:?} (returns None after delete)",
        val
    );

    // delete 幂等性
    adapter.delete("user:1").await?;
    println!("  ✓ delete(\"user:1\") run again (idempotent, no error)\n");

    // ============================================
    // TTL（过期时间）支持
    // ============================================
    println!("--- 3. TTL support ---\n");

    adapter
        .set(
            "session:abc",
            b"token_data".to_vec(),
            Some(Duration::from_secs(60)),
        )
        .await?;
    println!("  ✓ set(\"session:abc\", ttl=60s)");

    let val = adapter.get("session:abc").await?;
    println!(
        "  ✓ immediate get → {:?} (value present)",
        val.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    adapter
        .set(
            "temp:data",
            b"ephemeral".to_vec(),
            Some(Duration::from_millis(1)),
        )
        .await?;
    println!("  ✓ set(\"temp:data\", ttl=1ms)");

    // 等待过期
    tokio::time::sleep(Duration::from_millis(50)).await;
    let val = adapter.get("temp:data").await?;
    println!("  ✓ get after 50ms → {:?} (expired)", val);

    // ============================================
    // 通过 trait 对象使用（dyn dispatch）
    // ============================================
    println!("\n--- 4. trait objects (dyn dispatch) ---\n");

    let adapter2 = make_adapter(50);
    let provider: Arc<dyn DbCacheProvider + Send + Sync> = Arc::new(adapter2);
    println!("  ✓ created Arc<dyn DbCacheProvider + Send + Sync>");

    provider.set("dyn_key", b"dyn_value".to_vec(), None).await?;
    println!("  ✓ set(\"dyn_key\") through the trait object");

    let val = provider.get("dyn_key").await?;
    println!(
        "  ✓ get(\"dyn_key\") through the trait object → {:?}",
        val.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    provider.delete("dyn_key").await?;
    println!("  ✓ delete(\"dyn_key\") through the trait object\n");

    // ============================================
    // 批量操作模拟
    // ============================================
    println!("--- 5. batch operations ---\n");

    let adapter3 = make_adapter(200);
    let keys = ["cache:a", "cache:b", "cache:c", "cache:d", "cache:e"];

    // 批量写入
    for (i, key) in keys.iter().enumerate() {
        let value = format!("value_{}", i);
        adapter3.set(key, value.into_bytes(), None).await?;
    }
    println!("  ✓ wrote {} keys in batch", keys.len());

    // 批量读取
    let mut found = 0;
    for key in &keys {
        if adapter3.get(key).await?.is_some() {
            found += 1;
        }
    }
    println!("  ✓ batch read: {}/{} hits", found, keys.len());

    // 批量删除
    for key in &keys {
        adapter3.delete(key).await?;
    }
    println!("  ✓ deleted {} keys in batch", keys.len());

    let mut remaining = 0;
    for key in &keys {
        if adapter3.get(key).await?.is_some() {
            remaining += 1;
        }
    }
    println!("  ✓ verified: {} keys remain (should be 0)\n", remaining);

    // ============================================
    // 错误处理
    // ============================================
    println!("--- 6. error handling ---\n");

    // DbError::Cache 变体
    let err = DbError::Cache("cache get failed: simulated error".to_string());
    println!("  DbError::Cache error: {}", err);
    println!("  error message: {}", err.message());

    match err {
        DbError::Cache(msg) => println!("  ✓ error variant matched: Cache({})", msg),
        _ => println!("  ✗ unexpected error type"),
    }

    println!("\n========================================");
    println!("✨ Oxcache adapter example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - OxcacheDbCacheAdapter::new(cache)       wraps an oxcache backend");
    println!(
        "  - DbCacheProvider trait                   the unified cache interface (get/set/delete)"
    );
    println!("  - MokaMemoryBackend                       in-memory LRU cache backend");
    println!("  - TTL (Duration)                          expiration support");
    println!("  - Arc<dyn DbCacheProvider + Send + Sync>  trait object (DI injection)");
    println!("  - DbError::Cache                          the unified error type");

    Ok(())
}
