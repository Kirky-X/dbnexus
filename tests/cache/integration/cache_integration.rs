// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存模块集成测试
//!
//! 测试 CacheManager, CacheConfig, CacheKey, CacheStats 等缓存功能

#[cfg(feature = "cache")]
mod cache_tests {
    use dbnexus::cache::{CacheConfig, CacheKey, CacheManager, CacheStats};
    use std::sync::Arc;
    use std::time::Duration;

    // ============================================================================
    // CacheConfig 测试
    // ============================================================================

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();

        assert_eq!(config.max_capacity, 10000);
        assert_eq!(config.default_ttl, 300);
        assert_eq!(config.cleanup_interval, 60);
        assert!(config.enable_stats);
    }

    #[test]
    fn test_cache_config_custom() {
        let config = CacheConfig {
            max_capacity: 500,
            default_ttl: 60,
            cleanup_interval: 30,
            enable_stats: false,
        };

        assert_eq!(config.max_capacity, 500);
        assert_eq!(config.default_ttl, 60);
        assert_eq!(config.cleanup_interval, 30);
        assert!(!config.enable_stats);
    }

    #[test]
    fn test_cache_config_debug() {
        let config = CacheConfig::default();
        let debug = format!("{config:?}");
        assert!(!debug.is_empty());
    }

    // ============================================================================
    // CacheKey 测试
    // ============================================================================

    #[test]
    fn test_cache_key_new() {
        let key = CacheKey::new("users", "123");

        assert!(format!("{:?}", key).contains("users"));
        assert!(format!("{:?}", key).contains("123"));
    }

    #[test]
    fn test_cache_key_from_value() {
        let key = CacheKey::from_value("products", &"product_456");

        assert!(format!("{:?}", key).contains("products"));
    }

    #[test]
    fn test_cache_key_equality() {
        let key1 = CacheKey::new("users", "1");
        let key2 = CacheKey::new("users", "1");
        let key3 = CacheKey::new("users", "2");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cache_key_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        let key1 = CacheKey::new("users", "1");
        let key2 = CacheKey::new("users", "1");
        let key3 = CacheKey::new("users", "2");

        map.insert(key1.clone(), "user1".to_string());
        map.insert(key2.clone(), "user1_updated".to_string());
        map.insert(key3, "user2".to_string());

        // key1 and key2 should hash to the same bucket
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&key2), Some(&"user1_updated".to_string()));
    }

    #[test]
    fn test_cache_key_clone() {
        let key1 = CacheKey::new("orders", "789");
        let key2 = key1.clone();

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_key_debug() {
        let key = CacheKey::new("test", "key");
        let debug = format!("{key:?}");
        assert!(!debug.is_empty());
    }

    // ============================================================================
    // CacheStats 测试
    // ============================================================================

    #[test]
    fn test_cache_stats_new() {
        let stats = CacheStats::new();

        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_record_hit() {
        let stats = CacheStats::new();

        stats.record_hit();
        stats.record_hit();

        assert_eq!(stats.hit_rate(), 1.0);
    }

    #[test]
    fn test_cache_stats_record_miss() {
        let stats = CacheStats::new();

        stats.record_hit();
        stats.record_miss();

        assert_eq!(stats.hit_rate(), 0.5);
    }

    #[test]
    fn test_cache_stats_record_set() {
        let stats = CacheStats::new();

        stats.record_set();
        // Verify by checking hit rate (hits=0, misses=1, sets=1)
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_record_delete() {
        let stats = CacheStats::new();

        stats.record_delete();
        // Delete affects internal counter, check basic functionality
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_record_expiration() {
        let stats = CacheStats::new();

        stats.record_expiration();
        // Expiration affects internal counter, check basic functionality
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_hit_rate_empty() {
        let stats = CacheStats::new();
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_hit_rate_all_hits() {
        let stats = CacheStats::new();

        for _ in 0..100 {
            stats.record_hit();
        }

        assert_eq!(stats.hit_rate(), 1.0);
    }

    #[test]
    fn test_cache_stats_hit_rate_all_misses() {
        let stats = CacheStats::new();

        for _ in 0..100 {
            stats.record_miss();
        }

        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_debug() {
        let stats = CacheStats::new();
        let debug = format!("{stats:?}");
        assert!(!debug.is_empty());
    }

    // ============================================================================
    // CacheManager 基本操作测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_manager_basic_operations() {
        let config = CacheConfig {
            max_capacity: 100,
            default_ttl: 60,
            cleanup_interval: 30,
            enable_stats: true,
        };
        let cache: CacheManager<String> = CacheManager::new(config);

        let key = CacheKey::new("users", "1");

        // 初始状态 - 不存在
        assert!(cache.get(&key).await.is_none());

        // 设置值
        cache.set(key.clone(), "Alice".to_string()).await;

        // 获取值
        let value = cache.get(&key).await;
        assert_eq!(value, Some("Alice".to_string()));

        // 再次获取应该命中
        let value2 = cache.get(&key).await;
        assert_eq!(value2, Some("Alice".to_string()));
    }

    #[tokio::test]
    async fn test_cache_manager_set_update() {
        let config = CacheConfig::default();
        let cache: CacheManager<i32> = CacheManager::new(config);

        let key = CacheKey::new("counter", "1");

        // 初始设置
        cache.set(key.clone(), 10).await;

        // 更新值
        cache.set(key.clone(), 20).await;

        // 获取最新值
        let value = cache.get(&key).await;
        assert_eq!(value, Some(20));
    }

    #[tokio::test]
    async fn test_cache_manager_delete() {
        let config = CacheConfig::default();
        let cache: CacheManager<String> = CacheManager::new(config);

        let key = CacheKey::new("data", "key");

        // 设置并验证
        cache.set(key.clone(), "value".to_string()).await;
        assert!(cache.get(&key).await.is_some());

        // 删除
        cache.delete(&key).await;

        // 验证删除
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_manager_clear() {
        let config = CacheConfig::default();
        let mut cache: CacheManager<String> = CacheManager::new(config);

        let key1 = CacheKey::new("test", "1");
        let key2 = CacheKey::new("test", "2");

        cache.set(key1.clone(), "value1".to_string()).await;
        cache.set(key2.clone(), "value2".to_string()).await;

        // 使用 get 来验证存在
        assert!(cache.get(&key1).await.is_some());
        assert!(cache.get(&key2).await.is_some());

        cache.clear().await;

        // 验证清除
        assert!(cache.get(&key1).await.is_none());
        assert!(cache.get(&key2).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_manager_len() {
        let config = CacheConfig::default();
        let cache: CacheManager<String> = CacheManager::new(config);

        assert_eq!(cache.len().await, 0);

        let key1 = CacheKey::new("test", "1");
        cache.set(key1.clone(), "value1".to_string()).await;
        assert_eq!(cache.len().await, 1);

        let key2 = CacheKey::new("test", "2");
        cache.set(key2.clone(), "value2".to_string()).await;
        assert_eq!(cache.len().await, 2);

        cache.delete(&key1).await;
        assert_eq!(cache.len().await, 1);
    }

    // ============================================================================
    // CacheManager TTL 测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_manager_ttl_expiration() {
        let config = CacheConfig {
            max_capacity: 100,
            default_ttl: 1, // 1 second TTL
            cleanup_interval: 60,
            enable_stats: true,
        };
        let cache: CacheManager<String> = CacheManager::new(config);

        let key = CacheKey::new("temp", "data");

        // 设置值
        cache.set(key.clone(), "temporary".to_string()).await;

        // 立即获取应该存在
        assert!(cache.get(&key).await.is_some());

        // 等待过期
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // 过期后应该不存在
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_manager_custom_ttl() {
        let config = CacheConfig {
            max_capacity: 100,
            default_ttl: 60,
            cleanup_interval: 60,
            enable_stats: true,
        };
        let cache: CacheManager<String> = CacheManager::new(config);

        let key = CacheKey::new("custom", "ttl");

        // 设置带自定义 TTL 的值 (500ms)
        cache
            .set_with_ttl(key.clone(), "short-lived".to_string(), Duration::from_millis(500))
            .await;

        // 立即获取应该存在
        assert!(cache.get(&key).await.is_some());

        // 等待过期
        tokio::time::sleep(Duration::from_millis(600)).await;

        // 过期后应该不存在
        assert!(cache.get(&key).await.is_none());
    }

    // ============================================================================
    // CacheManager 容量测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_manager_capacity() {
        let config = CacheConfig {
            max_capacity: 3, // Small capacity for testing
            default_ttl: 60,
            cleanup_interval: 60,
            enable_stats: true,
        };
        let cache: CacheManager<String> = CacheManager::new(config);

        // 添加 3 个条目
        for i in 0..3 {
            let key = CacheKey::new("items", &i.to_string());
            cache.set(key, format!("item_{}", i)).await;
        }

        assert_eq!(cache.len().await, 3);

        // 添加第 4 个条目 - 应该触发 LRU 淘汰
        let key = CacheKey::new("items", "3");
        cache.set(key, "item_3".to_string()).await;

        // 容量应该仍然为 3
        assert_eq!(cache.len().await, 3);

        // 访问最早的条目以更新 LRU 顺序
        let key0 = CacheKey::new("items", "0");
        let _ = cache.get(&key0).await;

        // 添加第 5 个条目
        let key4 = CacheKey::new("items", "4");
        cache.set(key4, "item_4".to_string()).await;

        // 容量应该仍然为 3 (可能淘汰了 key1 或 key2)
        assert_eq!(cache.len().await, 3);
    }

    // ============================================================================
    // CacheManager 统计测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_manager_stats() {
        let config = CacheConfig {
            max_capacity: 100,
            default_ttl: 60,
            cleanup_interval: 60,
            enable_stats: true,
        };
        let cache: CacheManager<String> = CacheManager::new(config);

        let key = CacheKey::new("stats", "test");

        // 初始统计
        let stats = cache.stats();
        assert_eq!(stats.hit_rate(), 0.0);

        // 未命中
        let _ = cache.get(&key).await;
        let stats = cache.stats();
        assert_eq!(stats.hit_rate(), 0.0);

        // 设置并命中
        cache.set(key.clone(), "value".to_string()).await;
        let _ = cache.get(&key).await;
        let stats = cache.stats();
        // Hit rate: 1 hit / (1 miss + 1 hit) = 0.5
        assert!((stats.hit_rate() - 0.5).abs() < 0.01);

        // 再次命中
        let _ = cache.get(&key).await;
        let stats = cache.stats();
        // Hit rate: 2 hits / (1 miss + 2 hits) = 2/3 ≈ 0.667
        assert!((stats.hit_rate() - 0.667).abs() < 0.1);
    }

    // ============================================================================
    // CacheManager 并发测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_manager_concurrent_access() {
        let config = CacheConfig {
            max_capacity: 1000,
            default_ttl: 60,
            cleanup_interval: 60,
            enable_stats: true,
        };
        let cache: Arc<CacheManager<i32>> = Arc::new(CacheManager::new(config));

        // 并发设置
        let mut handles = Vec::new();
        for i in 0..100 {
            let cache = Arc::clone(&cache);
            let key = CacheKey::new("concurrent", &i.to_string());
            handles.push(tokio::spawn(async move {
                cache.set(key, i as i32).await;
            }));
        }
        futures::future::join_all(handles).await;

        // 验证所有值都被设置
        assert_eq!(cache.len().await, 100);

        // 读取验证
        for i in 0..100 {
            let key = CacheKey::new("concurrent", &i.to_string());
            let result = cache.get(&key).await;
            assert_eq!(result, Some(i as i32), "Failed at index {}", i);
        }
    }

    // ============================================================================
    // CacheManager 复杂类型测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_manager_complex_values() {
        #[derive(Debug, Clone, PartialEq)]
        struct User {
            id: u64,
            name: String,
            email: String,
        }

        let config = CacheConfig::default();
        let cache: CacheManager<User> = CacheManager::new(config);

        let key = CacheKey::new("users", "1");
        let user = User {
            id: 1,
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        };

        cache.set(key.clone(), user.clone()).await;

        let cached = cache.get(&key).await;
        assert_eq!(cached, Some(user));
    }

    #[tokio::test]
    async fn test_cache_manager_vec_values() {
        let config = CacheConfig::default();
        let cache: CacheManager<Vec<String>> = CacheManager::new(config);

        let key = CacheKey::new("list", "data");
        let data = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        cache.set(key.clone(), data.clone()).await;

        let cached = cache.get(&key).await;
        assert_eq!(cached, Some(data));
    }

    #[tokio::test]
    async fn test_cache_manager_option_values() {
        let config = CacheConfig::default();
        let cache: CacheManager<Option<String>> = CacheManager::new(config);

        let key1 = CacheKey::new("opt", "some");
        let key2 = CacheKey::new("opt", "none");

        cache.set(key1.clone(), Some("value".to_string())).await;
        cache.set(key2.clone(), None).await;

        assert_eq!(cache.get(&key1).await, Some(Some("value".to_string())));
        assert_eq!(cache.get(&key2).await, Some(None));
    }
}
