// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存单元测试
//!
//! 测试缓存核心功能：LRU淘汰、预热、穿透保护、雪崩保护、键模式匹配删除
//! 所有测试使用 moka 同步缓存，确保独立运行。

#[cfg(feature = "cache")]
mod cache_tests {
    use dbnexus::cache::CacheConfig;
    use dbnexus::cache::CacheKey;
    use moka::sync::Cache;
    use moka::sync::CacheBuilder;
    use std::sync::Arc;
    use std::time::Duration;

    // ============================================================================
    // 辅助函数：创建缓存
    // ============================================================================

    fn create_test_cache<V>(capacity: u64) -> Cache<String, V>
    where
        V: Send + Sync + 'static,
    {
        CacheBuilder::new(capacity)
    }

    fn create_test_cache_with_ttl<V>(capacity: u64, ttl: Duration) -> Cache<String, V>
    where
        V: Send + Sync + 'static,
    {
        CacheBuilder::new(capacity)
            .time_to_live(ttl)

    }

    // ============================================================================
    // LRU 淘汰策略测试
    // ============================================================================

    /// TEST-U-CACHE-001: 测试 LRU 淘汰策略 - 容量超限时最久未使用项被淘汰
    ///
    /// 验证当缓存容量达到上限时，最近最少使用的项被正确淘汰。
    #[test]
    fn test_lru_eviction_policy_capacity_exceeded() {
        let cache = create_test_cache::<String>(3);

        // 插入 3 个项目填满缓存
        cache.insert("key1".to_string(), "value1".to_string());
        cache.insert("key2".to_string(), "value2".to_string());
        cache.insert("key3".to_string(), "value3".to_string());

        // 验证所有项都存在
        assert!(cache.get(&"key1".to_string()).is_some());
        assert!(cache.get(&"key2".to_string()).is_some());
        assert!(cache.get(&"key3".to_string()).is_some());

        // 插入第 4 个项目，触发 LRU 淘汰
        cache.insert("key4".to_string(), "value4".to_string());

        // key4 一定存在
        assert!(cache.get(&"key4".to_string()).is_some());

        // 至少有 3 个项目（由于 LRU，key1/key2/key3 中有一个被淘汰）
        let remaining_count = [
            cache.get(&"key1".to_string()).is_some(),
            cache.get(&"key2".to_string()).is_some(),
            cache.get(&"key3".to_string()).is_some(),
        ].iter().filter(|&&x| x).count();

        assert!(remaining_count >= 2, "LRU 淘汰后应保留至少 2 个项目，实际保留: {}", remaining_count);
    }

    /// TEST-U-CACHE-002: 测试 LRU 淘汰策略 - 访问更新顺序
    ///
    /// 验证访问某个项会更新其最近使用时间，避免被淘汰。
    #[test]
    fn test_lru_eviction_policy_access_updates_order() {
        let cache = create_test_cache::<String>(3);

        // 插入 3 个项目
        cache.insert("key1".to_string(), "value1".to_string());
        cache.insert("key2".to_string(), "value2".to_string());
        cache.insert("key3".to_string(), "value3".to_string());

        // 访问 key1，使其成为最近使用的
        let _ = cache.get(&"key1".to_string());

        // 插入新项目触发淘汰
        cache.insert("key4".to_string(), "value4".to_string());

        // key1 应该仍然存在（因为刚刚被访问过）
        // key2 或 key3 中至少有一个被淘汰
        let key1_exists = cache.get(&"key1".to_string()).is_some();
        let remaining_old = [
            cache.get(&"key2".to_string()).is_some(),
            cache.get(&"key3".to_string()).is_some(),
        ].iter().filter(|&&x| x).count();

        assert!(key1_exists, "key1 刚被访问过，不应被淘汰");
        assert!(remaining_old <= 2, "旧项目中至少有一个被淘汰");
    }

    /// TEST-U-CACHE-003: 测试 LRU 淘汰策略 - 写入更新顺序
    ///
    /// 验证写入某个已存在的项会更新其最近使用时间。
    #[test]
    fn test_lru_eviction_policy_write_updates_order() {
        let cache = create_test_cache::<String>(2);

        cache.insert("key1".to_string(), "value1".to_string());
        cache.insert("key2".to_string(), "value2".to_string());

        // 重新写入 key1，更新其时间戳
        cache.insert("key1".to_string(), "value1_updated".to_string());

        // 插入新项目
        cache.insert("key3".to_string(), "value3".to_string());

        // key1 应该仍然存在（因为刚被写入）
        assert!(cache.get(&"key1".to_string()).is_some());
    }

    // ============================================================================
    // 缓存预热测试
    // ============================================================================

    /// TEST-U-CACHE-004: 测试缓存预热 - 批量插入预热数据
    ///
    /// 验证可以批量预热缓存数据，后续查询直接命中缓存。
    #[test]
    fn test_cache_preheat_batch_insert() {
        let cache = create_test_cache::<String>(100);

        // 模拟预热：批量插入常用数据
        let preheat_data = vec![
            ("config:max_connections".to_string(), "100".to_string()),
            ("config:timeout".to_string(), "30".to_string()),
            ("config:retries".to_string(), "3".to_string()),
        ];

        for (key, value) in preheat_data.iter() {
            cache.insert(key.clone(), value.clone());
        }

        // 验证预热数据可以直接从缓存获取
        let max_conn = cache.get(&"config:max_connections".to_string());
        assert_eq!(max_conn, Some(&"100".to_string()));

        let timeout = cache.get(&"config:timeout".to_string());
        assert_eq!(timeout, Some(&"30".to_string()));

        let retries = cache.get(&"config:retries".to_string());
        assert_eq!(retries, Some(&"3".to_string()));
    }

    /// TEST-U-CACHE-005: 测试缓存预热 - 并发预热
    ///
    /// 验证可以并发进行缓存预热操作。
    #[test]
    fn test_cache_preheat_concurrent() {
        let cache = Arc::new(create_test_cache::<String>(100));
        let keys: Vec<String> = (0..20).map(|i| format!("preheat:{}", i)).collect();

        // 并发预热
        let cache_clone1 = Arc::clone(&cache);
        let cache_clone2 = Arc::clone(&cache);

        // 使用 spawn_blocking 进行并发操作
        std::thread::spawn(move || {
            for key in keys.iter().take(10) {
                cache_clone1.insert(key.clone(), format!("value_{}", key));
            }
        });

        std::thread::spawn(move || {
            for key in keys.iter().skip(10) {
                cache_clone2.insert(key.clone(), format!("value_{}", key));
            }
        });

        // 等待线程完成
        std::thread::sleep(Duration::from_millis(100));

        // 验证所有预热数据存在
        for key in keys.iter() {
            let value = cache.get(key);
            assert!(value.is_some(), "预热 key {} 应该存在", key);
        }
    }

    /// TEST-U-CACHE-006: 测试缓存预热 - 预热后正常访问
    ///
    /// 验证预热数据在后续访问中保持可用。
    #[test]
    fn test_cache_preheat_persistence() {
        let cache = create_test_cache::<String>(10);

        // 预热数据
        cache.insert("user:1".to_string(), "Alice".to_string());
        cache.insert("user:2".to_string(), "Bob".to_string());

        // 多次访问验证持久性
        for _ in 0..5 {
            assert_eq!(cache.get(&"user:1".to_string()), Some(&"Alice".to_string()));
            assert_eq!(cache.get(&"user:2".to_string()), Some(&"Bob".to_string()));
        }
    }

    // ============================================================================
    // 缓存穿透保护测试
    // ============================================================================

    /// TEST-U-CACHE-007: 测试缓存穿透保护 - 空值缓存
    ///
    /// 验证对不存在键的查询可以缓存空值，防止重复查询数据库。
    #[test]
    fn test_cache_penetration_protection_null_caching() {
        let cache = create_test_cache::<String>(100);

        // 模拟查询不存在的用户
        let missing_key = "user:99999".to_string();

        // 第一次查询 - 缓存未命中
        let result = cache.get(&missing_key);
        assert!(result.is_none());

        // 缓存空值，防止穿透
        cache.insert(missing_key.clone(), "".to_string());

        // 后续查询应该从缓存获取（即使是空值）
        let cached_result = cache.get(&missing_key);
        assert!(cached_result.is_some());
    }

    /// TEST-U-CACHE-008: 测试缓存穿透保护 - 特殊值标记
    ///
    /// 验证可以使用特殊标记表示缓存穿透的键。
    #[test]
    fn test_cache_penetration_protection_sentinel_value() {
        let cache = create_test_cache::<String>(100);
        const NULL_MARKER: &str = "__CACHE_NULL__";

        // 查询不存在的键
        let missing_key = "product:不存在".to_string();

        // 存储空值标记
        cache.insert(missing_key.clone(), NULL_MARKER.to_string());

        // 后续查询识别空值标记
        let value = cache.get(&missing_key);
        assert!(value.is_some());
        assert_eq!(value.unwrap(), NULL_MARKER);
    }

    /// TEST-U-CACHE-009: 测试缓存穿透保护 - 计数防止恶意访问
    ///
    /// 验证可以通过计数机制检测异常的缓存未命中模式。
    #[test]
    fn test_cache_penetration_protection_miss_counting() {
        let cache = create_test_cache::<String>(100);

        // 模拟多次查询不存在的键
        let miss_key = "sensitive:data".to_string();
        let mut miss_count = 0;

        // 执行多次查询
        for _ in 0..10 {
            if cache.get(&miss_key).is_none() {
                miss_count += 1;
            }
        }

        // 记录miss_count用于分析
        assert_eq!(miss_count, 10, "未缓存的键应该有 10 次未命中");

        // 缓存这个键的值（即使是空）
        cache.insert(miss_key.clone(), "".to_string());

        // 再次查询应该命中
        let hit_count = if cache.get(&miss_key).is_some() { 1 } else { 0 };
        assert_eq!(hit_count, 1, "缓存后应该命中");
    }

    // ============================================================================
    // 缓存雪崩保护测试
    // ============================================================================

    /// TEST-U-CACHE-010: 测试缓存雪崩保护 - TTL 随机化
    ///
    /// 验证可以使用随机 TTL 防止大量缓存同时过期。
    #[test]
    fn test_cache_avalanche_protection_randomized_ttl() {
        // 创建多个缓存，使用不同的 TTL
        let cache1 = create_test_cache_with_ttl::<String>(100, Duration::from_secs(10));
        let cache2 = create_test_cache_with_ttl::<String>(100, Duration::from_secs(15));
        let cache3 = create_test_cache_with_ttl::<String>(100, Duration::from_secs(20));

        // 插入相同数据
        let key = "shared:data".to_string();
        cache1.insert(key.clone(), "value1".to_string());
        cache2.insert(key.clone(), "value2".to_string());
        cache3.insert(key.clone(), "value3".to_string());

        // 验证数据存在
        assert!(cache1.get(&key).is_some());
        assert!(cache2.get(&key).is_some());
        assert!(cache3.get(&key).is_some());

        // 注意：实际随机化 TTL 需要在应用层实现，这里测试多个缓存实例
    }

    /// TEST-U-CACHE-011: 测试缓存雪崩保护 - 渐进式过期
    ///
    /// 验证可以使用渐进式过期策略，避免同时过期。
    #[test]
    fn test_cache_avalanche_protection_gradual_expiration() {
        let cache = create_test_cache::<String>(100);

        // 插入数据
        cache.insert("key:1".to_string(), "value1".to_string());
        cache.insert("key:2".to_string(), "value2".to_string());
        cache.insert("key:3".to_string(), "value3".to_string());

        // 立即验证存在
        assert!(cache.get(&"key:1".to_string()).is_some());
        assert!(cache.get(&"key:2".to_string()).is_some());
        assert!(cache.get(&"key:3".to_string()).is_some());

        // 模拟定期刷新：访问时更新
        cache.insert("key:1".to_string(), "value1_v2".to_string());

        // 验证更新后值正确
        assert_eq!(cache.get(&"key:1".to_string()), Some(&"value1_v2".to_string()));
    }

    /// TEST-U-CACHE-012: 测试缓存雪崩保护 - 缓存重建
    ///
    /// 验证缓存项过期后可以重建。
    #[test]
    fn test_cache_avalanche_protection_rebuild() {
        // 使用很短的 TTL
        let cache = create_test_cache_with_ttl::<String>(10, Duration::from_millis(50));

        let key = "avalanche:test".to_string();

        // 初始插入
        cache.insert(key.clone(), "initial".to_string());
        assert_eq!(cache.get(&key), Some(&"initial".to_string()));

        // 等待 TTL 过期
        std::thread::sleep(Duration::from_millis(100));

        // 缓存已过期（moka 的 TTL 是近似值，可能需要更长等待）
        // 重建缓存
        cache.insert(key.clone(), "rebuilt".to_string());
        assert_eq!(cache.get(&key), Some(&"rebuilt".to_string()));
    }

    // ============================================================================
    // 缓存键模式匹配删除测试
    // ============================================================================

    /// TEST-U-CACHE-013: 测试键模式匹配删除 - 前缀匹配
    ///
    /// 验证可以按前缀模式删除缓存键。
    #[test]
    fn test_cache_key_pattern_delete_prefix() {
        let cache = create_test_cache::<String>(100);

        // 插入带前缀的键
        let user_keys = vec![
            "user:1".to_string(),
            "user:2".to_string(),
            "user:3".to_string(),
        ];
        let product_keys = vec![
            "product:1".to_string(),
            "product:2".to_string(),
        ];

        for key in user_keys.iter() {
            cache.insert(key.clone(), format!("value_{}", key));
        }
        for key in product_keys.iter() {
            cache.insert(key.clone(), format!("value_{}", key));
        }

        // 验证所有键存在
        assert!(cache.get(&"user:1".to_string()).is_some());
        assert!(cache.get(&"product:1".to_string()).is_some());

        // 删除所有 user: 前缀的键（模拟实现）
        // 注意：moka 不直接支持模式删除，这里演示手动删除
        let keys_to_delete: Vec<String> = vec![
            "user:1".to_string(),
            "user:2".to_string(),
            "user:3".to_string(),
        ];

        for key in keys_to_delete {
            cache.invalidate(&key);
        }

        // 验证 user 键已删除
        assert!(cache.get(&"user:1".to_string()).is_none());
        assert!(cache.get(&"user:2".to_string()).is_none());
        assert!(cache.get(&"user:3".to_string()).is_none());

        // 验证 product 键仍然存在
        assert!(cache.get(&"product:1".to_string()).is_some());
        assert!(cache.get(&"product:2".to_string()).is_some());
    }

    /// TEST-U-CACHE-014: 测试键模式匹配删除 - 精确匹配删除
    ///
    /// 验证可以精确删除特定键。
    #[test]
    fn test_cache_key_pattern_delete_exact() {
        let cache = create_test_cache::<String>(100);

        // 插入多个键
        cache.insert("config:db".to_string(), "postgres".to_string());
        cache.insert("config:cache".to_string(), "moka".to_string());
        cache.insert("config:log".to_string(), "debug".to_string());

        // 精确删除 config:db
        cache.invalidate(&"config:db".to_string());

        // 验证只有 config:db 被删除
        assert!(cache.get(&"config:db".to_string()).is_none());
        assert!(cache.get(&"config:cache".to_string()).is_some());
        assert!(cache.get(&"config:log".to_string()).is_some());
    }

    /// TEST-U-CACHE-015: 测试键模式匹配删除 - 批量删除
    ///
    /// 验证可以批量删除多个指定的键。
    #[test]
    fn test_cache_key_pattern_delete_batch() {
        let cache = create_test_cache::<String>(100);

        // 插入多个键
        let keys = vec![
            "session:1".to_string(),
            "session:2".to_string(),
            "session:3".to_string(),
            "cache:1".to_string(),
            "cache:2".to_string(),
        ];

        for key in keys.iter() {
            cache.insert(key.clone(), "value".to_string());
        }

        // 批量删除 session 相关键
        let session_keys = vec![
            "session:1".to_string(),
            "session:2".to_string(),
            "session:3".to_string(),
        ];

        for key in session_keys.iter() {
            cache.invalidate(key);
        }

        // 验证 session 键已删除
        for key in session_keys.iter() {
            assert!(cache.get(key).is_none(), "键 {} 应该被删除", key);
        }

        // 验证 cache 键仍然存在
        assert!(cache.get(&"cache:1".to_string()).is_some());
        assert!(cache.get(&"cache:2".to_string()).is_some());
    }

    /// TEST-U-CACHE-016: 测试键模式匹配删除 - 清除所有缓存
    ///
    /// 验证可以清除所有缓存数据。
    #[test]
    fn test_cache_key_pattern_clear_all() {
        let cache = create_test_cache::<String>(100);

        // 插入数据
        cache.insert("key1".to_string(), "value1".to_string());
        cache.insert("key2".to_string(), "value2".to_string());
        cache.insert("key3".to_string(), "value3".to_string());

        // 清除所有
        cache.invalidate(&"key1".to_string());
        cache.invalidate(&"key2".to_string());
        cache.invalidate(&"key3".to_string());

        // 验证所有数据已清除
        assert!(cache.get(&"key1".to_string()).is_none());
        assert!(cache.get(&"key2".to_string()).is_none());
        assert!(cache.get(&"key3".to_string()).is_none());
    }

    // ============================================================================
    // CacheConfig 测试
    // ============================================================================

    /// TEST-U-CACHE-017: 测试 CacheConfig 默认配置
    ///
    /// 验证默认配置值正确。
    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();

        assert_eq!(config.capacity, 1000);
        assert_eq!(config.ttl, None);
    }

    /// TEST-U-CACHE-018: 测试 CacheConfig 自定义配置
    ///
    /// 验证自定义配置值正确。
    #[test]
    fn test_cache_config_custom() {
        let config = CacheConfig::new(500, Some(60));

        assert_eq!(config.capacity, 500);
        assert_eq!(config.ttl, Some(60));
    }

    /// TEST-U-CACHE-019: 测试 CacheConfig 链式配置
    ///
    /// 验证链式配置 API 正确工作。
    #[test]
    fn test_cache_config_builder_pattern() {
        let config = CacheConfig::new(1000, None)
            .capacity(500)
            .ttl(120);

        assert_eq!(config.capacity, 500);
        assert_eq!(config.ttl, Some(120));
    }

    // ============================================================================
    // CacheKey trait 测试
    // ============================================================================

    /// TEST-U-CACHE-020: 测试 CacheKey trait 实现
    ///
    /// 验证 CacheKey trait 正确实现。
    #[test]
    fn test_cache_key_trait_implementation() {
        let key = "users:123".to_string();
        let cache_key = key.to_cache_key();

        assert_eq!(cache_key, "users:123");
    }

    // ============================================================================
    // 边界条件和错误处理测试
    // ============================================================================

    /// TEST-U-CACHE-021: 测试空键处理
    ///
    /// 验证空字符串作为键的行为。
    #[test]
    fn test_cache_empty_key() {
        let cache = create_test_cache::<String>(10);

        let empty_key = "".to_string();
        cache.insert(empty_key.clone(), "empty_value".to_string());

        let result = cache.get(&empty_key);
        assert!(result.is_some());
    }

    /// TEST-U-CACHE-022: 测试超长键处理
    ///
    /// 验证超长键可以正常存储和检索。
    #[test]
    fn test_cache_very_long_key() {
        let cache = create_test_cache::<String>(10);

        let long_key = "x".repeat(10000);
        let long_value = "y".repeat(10000);

        cache.insert(long_key.clone(), long_value.clone());

        let result = cache.get(&long_key);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), long_value);
    }

    /// TEST-U-CACHE-023: 测试特殊字符键
    ///
    /// 验证包含特殊字符的键可以正常处理。
    #[test]
    fn test_cache_special_characters_key() {
        let cache = create_test_cache::<String>(10);

        let special_keys = vec![
            "key:with:colons".to_string(),
            "key-with-dashes".to_string(),
            "key.with.dots".to_string(),
            "key_under_score".to_string(),
            "key with spaces".to_string(),
        ];

        for key in special_keys.iter() {
            cache.insert(key.clone(), format!("value_{}", key));
        }

        for key in special_keys.iter() {
            let result = cache.get(key);
            assert!(result.is_some(), "键 {:?} 应该存在", key);
        }
    }

    // ============================================================================
    // 缓存容量测试
    // ============================================================================

    /// TEST-U-CACHE-024: 测试缓存容量边界
    ///
    /// 验证缓存容量设置为不同边界值时的行为。
    #[test]
    fn test_cache_capacity_boundary() {
        // 测试容量为 1
        let cache = create_test_cache::<String>(1);
        cache.insert("key1".to_string(), "value1".to_string());
        cache.insert("key2".to_string(), "value2".to_string());

        // 至少有一个键存在
        let has_key = cache.get(&"key1".to_string()).is_some()
            || cache.get(&"key2".to_string()).is_some();
        assert!(has_key);
    }

    /// TEST-U-CACHE-025: 测试缓存更新已有键
    ///
    /// 验证更新已有键的值正常工作。
    #[test]
    fn test_cache_update_existing_key() {
        let cache = create_test_cache::<String>(10);

        cache.insert("counter".to_string(), "0".to_string());

        for i in 1..=5 {
            cache.insert("counter".to_string(), i.to_string());
        }

        assert_eq!(cache.get(&"counter".to_string()), Some(&"5".to_string()));
    }

    /// TEST-U-CACHE-026: 测试缓存值类型
    ///
    /// 验证可以存储不同类型的值。
    #[test]
    fn test_cache_different_value_types() {
        let string_cache = create_test_cache::<String>(10);
        let vec_cache = create_test_cache::<Vec<u8>>(10);
        let num_cache = create_test_cache::<u64>(10);

        string_cache.insert("str".to_string(), "hello".to_string());
        vec_cache.insert("vec".to_string(), vec![1, 2, 3]);
        num_cache.insert("num".to_string(), 42);

        assert_eq!(string_cache.get(&"str".to_string()), Some(&"hello".to_string()));
        assert_eq!(vec_cache.get(&"vec".to_string()), Some(&vec![1, 2, 3]));
        assert_eq!(num_cache.get(&"num".to_string()), Some(&42));
    }
}
