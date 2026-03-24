// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Mock CacheBackend 实现用于测试
//!
//! 提供一个线程安全的内存缓存模拟实现，可用于单元测试和集成测试

use crate::cache::traits::{CacheBackend, CacheResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Mock CacheBackend 实现
///
/// 用于测试的内存缓存实现，支持基本的 get/set/delete/exists 操作
#[derive(Debug, Clone, Default)]
pub struct MockCacheBackend {
    /// 内部存储
    store: Arc<RwLock<HashMap<String, String>>>,
}

impl MockCacheBackend {
    /// 创建新的 MockCacheBackend
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 清空缓存
    pub async fn clear(&self) {
        self.store.write().await.clear();
    }

    /// 获取当前条目数
    pub async fn len(&self) -> usize {
        self.store.read().await.len()
    }

    /// 检查是否为空
    pub async fn is_empty(&self) -> bool {
        self.store.read().await.is_empty()
    }
}

#[async_trait]
impl CacheBackend for MockCacheBackend {
    async fn get(&self, key: &str) -> Option<String> {
        self.store.read().await.get(key).cloned()
    }

    async fn set(&self, key: &str, value: String, _ttl: Option<Duration>) -> CacheResult<()> {
        self.store.write().await.insert(key.to_string(), value);
        Ok(())
    }

    async fn delete(&self, key: &str) -> CacheResult<()> {
        self.store.write().await.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> bool {
        self.store.read().await.contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_cache_basic() {
        let cache = MockCacheBackend::new();

        // Test set and get
        cache.set("key1", "value1".to_string(), None).await.unwrap();
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));

        // Test exists
        assert!(cache.exists("key1").await);
        assert!(!cache.exists("nonexistent").await);

        // Test delete
        cache.delete("key1").await.unwrap();
        assert!(cache.get("key1").await.is_none());
    }

    #[tokio::test]
    async fn test_mock_cache_clear() {
        let cache = MockCacheBackend::new();

        cache.set("key1", "value1".to_string(), None).await.unwrap();
        cache.set("key2", "value2".to_string(), None).await.unwrap();

        assert_eq!(cache.len().await, 2);

        cache.clear().await;
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_mock_cache_overwrite() {
        let cache = MockCacheBackend::new();

        cache.set("key1", "value1".to_string(), None).await.unwrap();
        cache.set("key1", "value2".to_string(), None).await.unwrap();

        assert_eq!(cache.get("key1").await, Some("value2".to_string()));
        assert_eq!(cache.len().await, 1);
    }
}
