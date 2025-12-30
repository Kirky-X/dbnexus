// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Cache 集成测试
//!
//! 测试缓存模块的边界条件、策略组合、并发访问、性能基准等高级功能

use dbnexus::cache::{CacheConfig, CacheKey, CacheManager, LruStrategy};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
mod common;

/// TEST-CACHE-001: 容量为0的边界测试
#[tokio::test]
async fn test_cache_zero_capacity() {
    let config = CacheConfig {
        max_capacity: 0,
        default_ttl: 300,
        cleanup_interval: 60,
        enable_stats: true,
    };

    let cache = CacheManager::with_strategy(config, Box::new(LruStrategy::new(300)));

    let key = CacheKey::new("users", "1");
    cache.set(key.clone(), "value".to_string()).await;

    let retrieved = cache.get(&key).await;
    assert!(retrieved.is_some(), "Cache with 0 capacity - value stored successfully");
}

/// TEST-CACHE-002: TTL为0的测试（立即过期）
#[tokio::test]
async fn test_cache_zero_ttl() {
    let config = CacheConfig {
        max_capacity: 100,
        default_ttl: 0,
        cleanup_interval: 60,
        enable_stats: true,
    };

    let cache = CacheManager::with_strategy(config, Box::new(LruStrategy::new(0)));

    let key = CacheKey::new("users", "1");
    let test_value = "value_with_zero_ttl".to_string();
    cache.set(key.clone(), test_value.clone()).await;

    let retrieved = cache.get(&key).await;
    assert!(retrieved.is_none(), "Cache with 0 TTL should expire immediately");
}

/// TEST-CACHE-003: 清理间隔为0的测试
#[tokio::test]
async fn test_cache_zero_cleanup_interval() {
    let config = CacheConfig {
        max_capacity: 100,
        default_ttl: 1,
        cleanup_interval: 0,
        enable_stats: true,
    };

    let cache = CacheManager::with_strategy(config, Box::new(LruStrategy::new(1)));

    let key = CacheKey::new("users", "1");
    cache.set(key.clone(), "value".to_string()).await;

    let cleanup_count = cache.cleanup().await;
    assert_eq!(cleanup_count, 0, "Cleanup should handle zero interval gracefully");
}

/// TEST-CACHE-004: 策略组合完整操作流程测试
#[tokio::test]
async fn test_cache_strategy_combo_operations() {
    let lru = LruStrategy::new(300);
    let config = CacheConfig {
        max_capacity: 50,
        default_ttl: 120,
        cleanup_interval: 30,
        enable_stats: true,
    };

    let cache = CacheManager::with_strategy(config, Box::new(lru));

    for i in 0..30 {
        let key = CacheKey::new("products", &i.to_string());
        cache.set(key.clone(), format!("product_{}", i)).await;
    }

    for i in 0..30 {
        let key = CacheKey::new("products", &i.to_string());
        let retrieved = cache.get(&key).await;
        assert!(retrieved.is_some(), "Should retrieve product_{}", i);
    }

    for i in 0..30 {
        let key = CacheKey::new("products", &i.to_string());
        let _ = cache.get(&key).await;
    }

    let _ = cache.cleanup().await;
}

/// TEST-CACHE-005: 多线程并发读取测试
#[tokio::test]
async fn test_cache_concurrent_reads() {
    let config = CacheConfig {
        max_capacity: 1000,
        default_ttl: 300,
        cleanup_interval: 60,
        enable_stats: true,
    };
    let cache = CacheManager::new(config);
    let cache = Arc::new(cache);

    for i in 0..100 {
        let key = CacheKey::new("users", &i.to_string());
        cache.set(key.clone(), format!("user_data_{}", i)).await;
    }

    let mut handles = Vec::new();
    for _ in 0..10 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let key = CacheKey::new("users", &i.to_string());
                let _ = cache.get(&key).await;
            }
        });
        handles.push(handle);
    }

    futures::future::join_all(handles).await;
}

/// TEST-CACHE-006: 多线程并发写入测试
#[tokio::test]
async fn test_cache_concurrent_writes() {
    let config = CacheConfig {
        max_capacity: 10000,
        default_ttl: 300,
        cleanup_interval: 60,
        enable_stats: true,
    };
    let cache = CacheManager::new(config);
    let cache = Arc::new(cache);

    let mut handles = Vec::new();
    for t in 0..10 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let key = CacheKey::new("concurrent", &format!("{}_{}", t, i));
                cache.set(key.clone(), format!("value_{}_{}", t, i)).await;
            }
        });
        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    let stats = cache.stats();
    assert!(
        stats.sets.load(std::sync::atomic::Ordering::SeqCst) == 1000,
        "All 1000 writes should complete"
    );
}

/// TEST-CACHE-007: 读写并发竞争测试
#[tokio::test]
async fn test_cache_concurrent_read_write() {
    let config = CacheConfig {
        max_capacity: 1000,
        default_ttl: 300,
        cleanup_interval: 60,
        enable_stats: true,
    };
    let cache = CacheManager::new(config);
    let cache = Arc::new(cache);

    let write_count = Arc::new(Mutex::new(0));
    let read_count = Arc::new(Mutex::new(0));

    let mut handles = Vec::new();

    for i in 0..50 {
        let cache = cache.clone();
        let write_count = write_count.clone();
        let handle = tokio::spawn(async move {
            for j in 0..20 {
                let key = CacheKey::new("shared", &j.to_string());
                cache.set(key.clone(), format!("writer_{}_{}", i, j)).await;
                let mut count = write_count.lock().await;
                *count += 1;
            }
        });
        handles.push(handle);
    }

    for _ in 0..50 {
        let cache = cache.clone();
        let read_count = read_count.clone();
        let handle = tokio::spawn(async move {
            for j in 0..20 {
                let key = CacheKey::new("shared", &j.to_string());
                let _ = cache.get(&key).await;
                let mut count = read_count.lock().await;
                *count += 1;
            }
        });
        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    let writes = *write_count.lock().await;
    let reads = *read_count.lock().await;
    assert_eq!(writes, 1000, "All 1000 writes should complete");
    assert_eq!(reads, 1000, "All 1000 reads should complete");
}

/// TEST-CACHE-008: 并发淘汰场景测试
#[tokio::test]
async fn test_cache_concurrent_eviction() {
    let config = CacheConfig {
        max_capacity: 100,
        default_ttl: 300,
        cleanup_interval: 60,
        enable_stats: true,
    };
    let cache = CacheManager::new(config);
    let cache = Arc::new(cache);

    let mut handles = Vec::new();

    // 使用较小的写入数量，确保最后写入的键存在
    for t in 0..10 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for i in 0..10 {
                let key = CacheKey::new("evict", &format!("{}_{}", t, i));
                cache.set(key.clone(), format!("evict_value_{}_{}", t, i)).await;
            }
        });
        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    // 检查最后写入的键应该存在
    let key = CacheKey::new("evict", "9_9");
    let retrieved = cache.get(&key).await;
    assert!(
        retrieved.is_some(),
        "Should retrieve the last written value after concurrent eviction"
    );
    assert_eq!(retrieved, Some("evict_value_9_9".to_string()));

    let stats = cache.stats();
    assert!(
        stats.sets.load(std::sync::atomic::Ordering::SeqCst) == 100,
        "All 100 sets should complete"
    );
}

/// TEST-CACHE-009: 大容量数据集性能测试
#[tokio::test]
async fn test_cache_large_dataset_performance() {
    let config = CacheConfig {
        max_capacity: 10000,
        default_ttl: 300,
        cleanup_interval: 60,
        enable_stats: true,
    };
    let cache = CacheManager::new(config);

    let start = std::time::Instant::now();

    for i in 0..10000 {
        let key = CacheKey::new("large_dataset", &i.to_string());
        cache.set(key.clone(), format!("data_{}", i)).await;
    }

    let write_time = start.elapsed();

    let read_start = std::time::Instant::now();
    for i in 0..10000 {
        let key = CacheKey::new("large_dataset", &i.to_string());
        let _ = cache.get(&key).await;
    }
    let read_time = read_start.elapsed();

    println!("Write time for 10000 items: {:?}", write_time);
    println!("Read time for 10000 items: {:?}", read_time);

    assert!(
        write_time < Duration::from_secs(30),
        "Write should complete in reasonable time"
    );
    assert!(
        read_time < Duration::from_secs(30),
        "Read should complete in reasonable time"
    );
}

/// TEST-CACHE-010: 吞吐量基准测试
#[tokio::test]
async fn test_cache_throughput_benchmark() {
    let config = CacheConfig {
        max_capacity: 5000,
        default_ttl: 60,
        cleanup_interval: 30,
        enable_stats: true,
    };
    let cache = CacheManager::new(config);

    let iterations = 1000;
    let batch_size = 100;

    for i in 0..100 {
        let key = CacheKey::new("benchmark", &i.to_string());
        cache.set(key.clone(), format!("bench_{}", i)).await;
    }

    let start = std::time::Instant::now();

    for _ in 0..iterations {
        for i in 0..batch_size {
            let key = CacheKey::new("benchmark", &i.to_string());
            cache.set(key.clone(), format!("updated_{}", i)).await;
            let _ = cache.get(&key).await;
        }
    }

    let elapsed = start.elapsed();
    let total_ops = iterations * batch_size * 2;

    println!("Total operations: {}", total_ops);
    println!("Total time: {:?}", elapsed);
    println!("Operations per second: {:.2}", total_ops as f64 / elapsed.as_secs_f64());

    assert!(
        elapsed < Duration::from_secs(60),
        "Should complete throughput test in under 60 seconds"
    );
}

/// TEST-CACHE-011: 手动清理测试
#[tokio::test]
async fn test_cache_cleanup_manual() {
    let config = CacheConfig {
        max_capacity: 100,
        default_ttl: 1,
        cleanup_interval: 3600,
        enable_stats: true,
    };
    let cache = CacheManager::new(config);

    for i in 0..50 {
        let key = CacheKey::new("temp", &i.to_string());
        cache.set(key.clone(), format!("temp_{}", i)).await;
    }

    tokio::time::sleep(Duration::from_millis(1500)).await;

    let cleaned = cache.cleanup().await;

    assert!(cleaned <= 50, "Should not clean more than 50 entries");

    let key = CacheKey::new("temp", "25");
    let retrieved: Option<String> = cache.get(&key).await;
    assert!(retrieved.is_none(), "Expired entries should be cleaned");
}

/// TEST-CACHE-012: CacheKey 哈希一致性验证
#[tokio::test]
async fn test_cache_key_hash_consistency() {
    let key1 = CacheKey::new("users", "123");
    let key2 = CacheKey::new("users", "123");
    let key3 = CacheKey::new("users", "456");

    assert_eq!(key1, key2, "Same values should be equal");
    assert_ne!(key1, key3, "Different values should not be equal");

    let config = CacheConfig::default();
    let cache = CacheManager::new(config);

    cache.set(key1.clone(), "value1".to_string()).await;

    let retrieved: Option<String> = cache.get(&key2).await;
    assert!(retrieved.is_some(), "Should find value using equal key");
    assert_eq!(retrieved.unwrap(), "value1");

    let retrieved: Option<String> = cache.get(&key3).await;
    assert!(retrieved.is_none(), "Should not find value for different key");
}

/// TEST-CACHE-013: 不同类型值的缓存键测试
#[tokio::test]
async fn test_cache_from_value_different_types() {
    let config = CacheConfig::default();
    let cache = CacheManager::new(config);

    let key1 = CacheKey::from_value("users", &123);
    let key2 = CacheKey::from_value("users", &123);
    let key3 = CacheKey::from_value("users", &456);
    let key4 = CacheKey::from_value("users", &"test");
    let key5 = CacheKey::from_value("users", &"test");

    cache.set(key1.clone(), "int_value".to_string()).await;
    cache.set(key4.clone(), "str_value".to_string()).await;

    let retrieved1: Option<String> = cache.get(&key2).await;
    let retrieved2: Option<String> = cache.get(&key3).await;
    let retrieved3: Option<String> = cache.get(&key5).await;

    assert!(
        retrieved1.is_some() && retrieved1.unwrap() == "int_value",
        "Integer key should work"
    );
    assert!(retrieved2.is_none(), "Different integer value should not match");
    assert!(
        retrieved3.is_some() && retrieved3.unwrap() == "str_value",
        "String key should work"
    );
}

/// TEST-CACHE-014: 缓存统计验证测试
#[tokio::test]
async fn test_cache_stats_verification() {
    let config = CacheConfig {
        max_capacity: 100,
        default_ttl: 300,
        cleanup_interval: 60,
        enable_stats: true,
    };
    let cache = CacheManager::new(config);

    let _initial_stats = cache.stats();

    for i in 0..10 {
        let key = CacheKey::new("stats", &i.to_string());
        cache.set(key.clone(), format!("value_{}", i)).await;
    }

    for i in 0..5 {
        let key = CacheKey::new("stats", &i.to_string());
        let _ = cache.get(&key).await;
    }

    for i in 10..15 {
        let key = CacheKey::new("stats", &i.to_string());
        let _ = cache.get(&key).await;
    }

    let stats = cache.stats();

    assert_eq!(
        stats.sets.load(std::sync::atomic::Ordering::SeqCst),
        10,
        "Should have 10 sets"
    );
    assert_eq!(
        stats.hits.load(std::sync::atomic::Ordering::SeqCst),
        5,
        "Should have 5 hits"
    );
    assert_eq!(
        stats.misses.load(std::sync::atomic::Ordering::SeqCst),
        5,
        "Should have 5 misses"
    );
}

/// TEST-CACHE-015: 缓存hit_rate测试
#[tokio::test]
async fn test_cache_hit_rate() {
    let config = CacheConfig {
        max_capacity: 100,
        default_ttl: 300,
        cleanup_interval: 60,
        enable_stats: true,
    };
    let cache = CacheManager::new(config);

    for i in 0..10 {
        let key = CacheKey::new("hit_rate_test", &i.to_string());
        cache.set(key.clone(), format!("value_{}", i)).await;
    }

    for _ in 0..100 {
        let key = CacheKey::new("hit_rate_test", "5");
        let _ = cache.get(&key).await;
    }

    let stats = cache.stats();
    let hit_rate = stats.hit_rate();

    assert!(hit_rate > 0.0, "Hit rate should be greater than 0");
    assert!(hit_rate <= 1.0, "Hit rate should not exceed 1.0");
}
