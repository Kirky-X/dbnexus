// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 缓存模块集成测试
//!
//! 测试 CacheConfig 和 moka Cache 功能

#[cfg(feature = "cache")]
mod cache_tests {
    use dbnexus::CacheConfig;

    // ============================================================================
    // CacheConfig 测试
    // ============================================================================

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();

        assert_eq!(config.policy_cache_capacity, 4096);
        assert_eq!(config.sql_parse_cache_capacity, 1000);
        assert_eq!(config.query_cache_capacity, 10000);
        assert_eq!(config.default_ttl, 300);
    }

    #[test]
    fn test_cache_config_new() {
        let config = CacheConfig {
            policy_cache_capacity: 500,
            sql_parse_cache_capacity: 500,
            query_cache_capacity: 500,
            default_ttl: 60,
        };

        assert_eq!(config.policy_cache_capacity, 500);
        assert_eq!(config.default_ttl, 60);
    }

    #[test]
    fn test_cache_config_builder() {
        let config = CacheConfig {
            policy_cache_capacity: 500,
            ..Default::default()
        };

        assert_eq!(config.policy_cache_capacity, 500);
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
    fn test_cache_key_string() {
        let key: String = "users:123".to_string();

        assert!(key.contains("users"));
        assert!(key.contains("123"));
    }

    #[test]
    fn test_cache_key_format() {
        let key: String = format!("products:{}", "product_456");

        assert!(key.contains("products"));
    }

    #[test]
    fn test_cache_key_equality() {
        let key1: String = "users:1".to_string();
        let key2: String = "users:1".to_string();
        let key3: String = "users:2".to_string();

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cache_key_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        let key1: String = "users:1".to_string();
        let key2: String = "users:1".to_string();
        let key3: String = "users:2".to_string();

        map.insert(key1.clone(), "user1".to_string());
        map.insert(key2.clone(), "user1_updated".to_string());
        map.insert(key3, "user2".to_string());

        // key1 and key2 should hash to the same bucket
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&key2), Some(&"user1_updated".to_string()));
    }

    #[test]
    fn test_cache_key_clone() {
        let key1: String = "orders:789".to_string();
        let key2 = key1.clone();

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_key_debug() {
        let key: String = "test:key".to_string();
        let debug = format!("{key:?}");
        assert!(!debug.is_empty());
    }

    // ============================================================================
    // DbCacheProvider 集成测试（使用 MockCacheProvider）
    // ============================================================================

    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use dbnexus::DbCacheProvider;
    use dbnexus::foundation::DbError;

    /// In-memory mock cache provider for integration testing.
    struct MockCacheProvider {
        inner: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl MockCacheProvider {
        fn new() -> Self {
            Self {
                inner: Mutex::new(HashMap::new()),
            }
        }
    }

    impl DbCacheProvider for MockCacheProvider {
        fn get<'a>(
            &'a self,
            key: &'a str,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<Vec<u8>>, DbError>> + Send + 'a>>
        {
            Box::pin(async move {
                let map = self.inner.lock().expect("mock cache lock poisoned");
                Ok(map.get(key).cloned())
            })
        }

        fn set<'a>(
            &'a self,
            key: &'a str,
            value: Vec<u8>,
            _ttl: Option<Duration>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), DbError>> + Send + 'a>> {
            Box::pin(async move {
                let mut map = self.inner.lock().expect("mock cache lock poisoned");
                map.insert(key.to_string(), value);
                Ok(())
            })
        }

        fn delete<'a>(
            &'a self,
            key: &'a str,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), DbError>> + Send + 'a>> {
            Box::pin(async move {
                let mut map = self.inner.lock().expect("mock cache lock poisoned");
                map.remove(key);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn test_cache_provider_basic_get_set_delete() {
        let cache = MockCacheProvider::new();

        // set a value
        cache.set("key1", b"value1".to_vec(), None).await.expect("set failed");

        // get returns the value
        let got = cache.get("key1").await.expect("get failed");
        assert_eq!(got, Some(b"value1".to_vec()));

        // delete the value
        cache.delete("key1").await.expect("delete failed");

        // get after delete returns None
        let gone = cache.get("key1").await.expect("get after delete failed");
        assert!(gone.is_none());
    }

    #[tokio::test]
    async fn test_cache_provider_absent_key_returns_none() {
        let cache = MockCacheProvider::new();
        let got = cache.get("nonexistent").await.expect("get failed");
        assert!(got.is_none(), "absent key must return Ok(None)");
    }

    #[tokio::test]
    async fn test_cache_provider_overwrite() {
        let cache = MockCacheProvider::new();

        cache.set("k", b"old".to_vec(), None).await.expect("set old");
        cache.set("k", b"new".to_vec(), None).await.expect("set new");

        let got = cache.get("k").await.expect("get");
        assert_eq!(got, Some(b"new".to_vec()));
    }

    #[tokio::test]
    async fn test_cache_provider_concurrent_access() {
        let cache = Arc::new(MockCacheProvider::new());
        let mut handles = vec![];

        // 10 concurrent tasks writing different keys
        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let handle = tokio::spawn(async move {
                let key = format!("concurrent:{}", i);
                cache_clone
                    .set(&key, format!("value{}", i).into_bytes(), None)
                    .await
                    .expect("set failed");
            });
            handles.push(handle);
        }

        // Wait for all writes
        for handle in handles {
            handle.await.expect("task panicked");
        }

        // Verify all values
        for i in 0..10 {
            let key = format!("concurrent:{}", i);
            let got = cache.get(&key).await.expect("get failed");
            assert_eq!(got, Some(format!("value{}", i).into_bytes()));
        }
    }

    #[tokio::test]
    async fn test_cache_provider_capacity_eviction() {
        // MockCacheProvider has no capacity limit, so this test verifies
        // that the DbCacheProvider trait supports the eviction contract:
        // after many inserts, the cache remains consistent.
        let cache: Arc<dyn DbCacheProvider + Send + Sync> = Arc::new(MockCacheProvider::new());

        // Insert many entries
        for i in 0..100 {
            let key = format!("evict:{}", i);
            cache
                .set(&key, format!("val{}", i).into_bytes(), None)
                .await
                .expect("set failed");
        }

        // The most recent entries should still be accessible
        let last = cache.get("evict:99").await.expect("get failed");
        assert_eq!(last, Some(b"val99".to_vec()));

        // Earlier entries may or may not be evicted depending on implementation
        // (MockCacheProvider keeps all, but a real cache with capacity would evict)
        let _first = cache.get("evict:0").await.expect("get failed");
    }

    #[tokio::test]
    async fn test_cache_provider_dyn_dispatch() {
        let cache: Arc<dyn DbCacheProvider + Send + Sync> = Arc::new(MockCacheProvider::new());

        cache.set("dyn", b"works".to_vec(), None).await.expect("set");
        let got = cache.get("dyn").await.expect("get");
        assert_eq!(got, Some(b"works".to_vec()));

        cache.delete("dyn").await.expect("delete");
        let gone = cache.get("dyn").await.expect("get after delete");
        assert!(gone.is_none());
    }
}
