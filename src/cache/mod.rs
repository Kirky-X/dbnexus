// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存模块
//!
//! 基于 moka 的高性能缓存系统
//!
//! # Feature Requirements
//!
//! 此模块需要启用 `cache` feature。

#[cfg(feature = "cache")]
pub use moka::future::Cache;
#[cfg(feature = "cache")]
pub use moka::future::CacheBuilder;

// ============================================================================
// 缓存类型别名
// ============================================================================

/// 异步缓存类型（固定使用 String 作为键类型）
#[cfg(feature = "cache")]
pub type AsyncCache<V> = moka::future::Cache<String, V>;

/// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 最大容量
    pub capacity: u64,
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
    pub fn new(capacity: u64, ttl: Option<u64>) -> Self {
        Self { capacity, ttl }
    }

    /// 设置容量
    pub fn capacity(mut self, capacity: u64) -> Self {
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
#[cfg(feature = "cache")]
pub async fn create_cache<V>(capacity: u64) -> Result<AsyncCache<V>, Box<dyn std::error::Error + Send + Sync>>
where
    V: Clone + Send + Sync + 'static,
{
    let cache = CacheBuilder::new(capacity).build();
    Ok(cache)
}

/// 创建带TTL的异步缓存
#[cfg(feature = "cache")]
pub async fn create_cache_with_ttl<V>(
    capacity: u64,
    ttl: std::time::Duration,
) -> Result<AsyncCache<V>, Box<dyn std::error::Error + Send + Sync>>
where
    V: Clone + Send + Sync + 'static,
{
    let cache = CacheBuilder::new(capacity).time_to_live(ttl).build();
    Ok(cache)
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 缓存条目数
    pub entry_count: u64,
}

impl CacheStats {
    /// 计算缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache = create_cache::<String>(100).await.unwrap();

        // Test set and get
        cache.insert("key1".to_string(), "value1".to_string()).await;
        let result = cache.get(&"key1".to_string()).await;
        assert_eq!(result, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_cache_overwrite() {
        let cache = create_cache::<String>(10).await.unwrap();

        cache.insert("key".to_string(), "value1".to_string()).await;
        cache.insert("key".to_string(), "value2".to_string()).await;

        let result = cache.get(&"key".to_string()).await;
        assert_eq!(result, Some("value2".to_string()));
    }

    #[tokio::test]
    async fn test_cache_invalidate() {
        let cache = create_cache::<String>(10).await.unwrap();

        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;

        assert!(cache.get(&"key1".to_string()).await.is_some());
        assert!(cache.get(&"key2".to_string()).await.is_some());

        cache.invalidate(&"key1".to_string()).await;

        assert!(cache.get(&"key1".to_string()).await.is_none());
        assert!(cache.get(&"key2".to_string()).await.is_some());
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let stats = CacheStats {
            hits: 80,
            misses: 20,
            entry_count: 100,
        };
        assert!((stats.hit_rate() - 0.8).abs() < 0.001);

        let empty_stats = CacheStats::default();
        assert_eq!(empty_stats.hit_rate(), 0.0);

        let all_hits = CacheStats {
            hits: 100,
            misses: 0,
            entry_count: 100,
        };
        assert_eq!(all_hits.hit_rate(), 1.0);
    }
}
