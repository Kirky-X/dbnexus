// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存模块
//!
//! 基于 oxcache 的高性能缓存系统

pub use oxcache::Cache;
pub use oxcache::CacheBuilder;
pub use oxcache::traits::Cacheable;

// ============================================================================
// 缓存类型别名
// ============================================================================

/// 异步缓存类型（固定使用 String 作为键类型）
pub type AsyncCache<V> = oxcache::Cache<String, V>;

/// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 最大容量
    pub capacity: usize,
    /// TTL（秒）
    pub ttl: Option<u64>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            capacity: 1000,
            ttl: None,
        }
    }
}

impl CacheConfig {
    /// 创建新配置
    pub fn new(capacity: usize, ttl: Option<u64>) -> Self {
        Self { capacity, ttl }
    }

    /// 设置容量
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// 设置TTL
    pub fn ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }
}

/// 缓存键特征
pub trait CacheKey: Send + Sync + 'static {
    /// 将键转换为字符串形式用于缓存存储
    fn to_cache_key(&self) -> String;
}

impl CacheKey for String {
    fn to_cache_key(&self) -> String {
        self.clone()
    }
}

/// 创建新的异步缓存
pub async fn create_cache<V>(capacity: usize) -> Result<AsyncCache<V>, Box<dyn std::error::Error>>
where
    V: Cacheable,
{
    let cache = AsyncCache::builder().capacity(capacity as u64).build().await?;
    Ok(cache)
}

/// 创建带TTL的异步缓存
pub async fn create_cache_with_ttl<V>(
    capacity: usize,
    ttl: std::time::Duration,
) -> Result<AsyncCache<V>, Box<dyn std::error::Error>>
where
    V: Cacheable,
{
    let cache = AsyncCache::builder().capacity(capacity as u64).ttl(ttl).build().await?;
    Ok(cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache = create_cache::<String>(100).await.unwrap();

        // Test set and get
        cache.set(&"key1".to_string(), &"value1".to_string()).await;
        let result = cache.get(&"key1".to_string()).await.unwrap();
        assert_eq!(result, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_cache_overwrite() {
        let cache = create_cache::<String>(10).await.unwrap();

        cache.set(&"key".to_string(), &"value1".to_string()).await;
        cache.set(&"key".to_string(), &"value2".to_string()).await;

        let result = cache.get(&"key".to_string()).await.unwrap();
        assert_eq!(result, Some("value2".to_string()));
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = create_cache::<String>(10).await.unwrap();

        cache.set(&"key1".to_string(), &"value1".to_string()).await;
        cache.set(&"key2".to_string(), &"value2".to_string()).await;

        assert!(cache.get(&"key1".to_string()).await.unwrap().is_some());
        assert!(cache.get(&"key2".to_string()).await.unwrap().is_some());

        cache.clear().await;

        assert!(cache.get(&"key1".to_string()).await.unwrap().is_none());
        assert!(cache.get(&"key2".to_string()).await.unwrap().is_none());
    }
}
