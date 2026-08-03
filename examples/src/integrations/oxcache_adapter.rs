// Copyright (c) 2026 Kirky.X
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

use dbnexus::foundation::DbError;
use dbnexus::DbCacheProvider;
use dbnexus::OxcacheDbCacheAdapter;
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

/// 打印操作结果
#[allow(dead_code)]
fn print_result<T: std::fmt::Debug>(op: &str, result: &Result<T, DbError>) {
    match result {
        Ok(val) => println!("  ✓ {} → {:?}", op, val),
        Err(e) => println!("  ✗ {} → 错误: {}", op, e),
    }
}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔗 DBNexus Oxcache 适配器示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建适配器
    // ============================================
    println!("--- 1. 创建 OxcacheDbCacheAdapter ---\n");

    let adapter = make_adapter(100);
    println!("  ✓ 适配器创建成功（MokaMemoryBackend, capacity=100）");
    println!("  类型: OxcacheDbCacheAdapter");
    println!("  实现: DbCacheProvider trait (get/set/delete)\n");

    // ============================================
    // 2. 基本 get/set/delete 操作
    // ============================================
    println!("--- 2. 基本 CRUD 操作 ---\n");

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
    println!("  ✓ get(\"user:999\") → {:?} (不存在返回 None)", val);

    // delete
    adapter.delete("user:1").await?;
    println!("  ✓ delete(\"user:1\")");

    let val = adapter.get("user:1").await?;
    println!("  ✓ get(\"user:1\") → {:?} (删除后返回 None)", val);

    // delete 幂等性
    adapter.delete("user:1").await?;
    println!("  ✓ delete(\"user:1\") 再次执行（幂等，不报错）\n");

    // ============================================
    // 3. TTL（过期时间）支持
    // ============================================
    println!("--- 3. TTL 支持 ---\n");

    // 设置带 TTL 的缓存
    adapter
        .set("session:abc", b"token_data".to_vec(), Some(Duration::from_secs(60)))
        .await?;
    println!("  ✓ set(\"session:abc\", ttl=60s)");

    let val = adapter.get("session:abc").await?;
    println!(
        "  ✓ 立即 get → {:?} (值存在)",
        val.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    // 设置短 TTL 的缓存
    adapter
        .set("temp:data", b"ephemeral".to_vec(), Some(Duration::from_millis(1)))
        .await?;
    println!("  ✓ set(\"temp:data\", ttl=1ms)");

    // 等待过期
    tokio::time::sleep(Duration::from_millis(50)).await;
    let val = adapter.get("temp:data").await?;
    println!("  ✓ 50ms 后 get → {:?} (已过期)", val);

    // ============================================
    // 4. 通过 trait 对象使用（dyn dispatch）
    // ============================================
    println!("\n--- 4. Trait 对象（dyn dispatch）---\n");

    let adapter2 = make_adapter(50);
    let provider: Arc<dyn DbCacheProvider + Send + Sync> = Arc::new(adapter2);
    println!("  ✓ 创建 Arc<dyn DbCacheProvider + Send + Sync>");

    provider.set("dyn_key", b"dyn_value".to_vec(), None).await?;
    println!("  ✓ 通过 trait 对象 set(\"dyn_key\")");

    let val = provider.get("dyn_key").await?;
    println!(
        "  ✓ 通过 trait 对象 get(\"dyn_key\") → {:?}",
        val.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    provider.delete("dyn_key").await?;
    println!("  ✓ 通过 trait 对象 delete(\"dyn_key\")\n");

    // ============================================
    // 5. 批量操作模拟
    // ============================================
    println!("--- 5. 批量操作 ---\n");

    let adapter3 = make_adapter(200);
    let keys = ["cache:a", "cache:b", "cache:c", "cache:d", "cache:e"];

    // 批量写入
    for (i, key) in keys.iter().enumerate() {
        let value = format!("value_{}", i);
        adapter3.set(key, value.into_bytes(), None).await?;
    }
    println!("  ✓ 批量写入 {} 个 key", keys.len());

    // 批量读取
    let mut found = 0;
    for key in &keys {
        if adapter3.get(key).await?.is_some() {
            found += 1;
        }
    }
    println!("  ✓ 批量读取: {}/{} 命中", found, keys.len());

    // 批量删除
    for key in &keys {
        adapter3.delete(key).await?;
    }
    println!("  ✓ 批量删除 {} 个 key", keys.len());

    // 验证删除
    let mut remaining = 0;
    for key in &keys {
        if adapter3.get(key).await?.is_some() {
            remaining += 1;
        }
    }
    println!("  ✓ 验证: 剩余 {} 个 key（应为 0）\n", remaining);

    // ============================================
    // 6. 错误处理
    // ============================================
    println!("--- 6. 错误处理 ---\n");

    // DbError::Cache 变体
    let err = DbError::Cache("cache get failed: simulated error".to_string());
    println!("  DbError::Cache 错误: {}", err);
    println!("  错误消息: {}", err.message());

    // 验证错误类型
    match err {
        DbError::Cache(msg) => println!("  ✓ 错误变体匹配: Cache({})", msg),
        _ => println!("  ✗ 意外错误类型"),
    }

    println!("\n========================================");
    println!("✨ Oxcache 适配器示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - OxcacheDbCacheAdapter::new(cache)       包装 oxcache 后端");
    println!("  - DbCacheProvider trait                   统一缓存接口 (get/set/delete)");
    println!("  - MokaMemoryBackend                       内存 LRU 缓存后端");
    println!("  - TTL (Duration)                          过期时间支持");
    println!("  - Arc<dyn DbCacheProvider + Send + Sync>  trait 对象（DI 注入）");
    println!("  - DbError::Cache                          统一错误类型");

    Ok(())
}
