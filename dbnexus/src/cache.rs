// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 缓存模块
//!
//! 提供实体缓存功能，支持：
//! - LRU 缓存策略
//! - TTL (Time-To-Live) 过期机制
//! - 缓存穿透防护
//! - 缓存击穿保护
//!
//! # Example
//!
//! ```rust,no_run
//! use dbnexus::cache::{CacheConfig, CacheKey, CacheManager};
//!
//! fn main() {
//!     let cache: CacheManager<String> = CacheManager::new(CacheConfig::default());
//!     let key = CacheKey::new("user", "1");
//!
//!     tokio::runtime::Runtime::new().unwrap().block_on(async {
//!         cache.set(key.clone(), "Alice".to_string()).await;
//!         let _ = cache.get(&key).await;
//!     });
//! }
//! ```

use async_trait::async_trait;
use indexmap::IndexMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 最大条目数
    pub max_capacity: usize,
    /// 默认 TTL（秒）
    pub default_ttl: u64,
    /// 清理间隔（秒）
    pub cleanup_interval: u64,
    /// 是否启用统计
    pub enable_stats: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_capacity: 10000,
            default_ttl: 300,
            cleanup_interval: 60,
            enable_stats: true,
        }
    }
}

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    /// 缓存值
    value: T,
    /// 创建时间
    #[allow(dead_code)]
    created_at: Instant,
    /// 过期时间
    expires_at: Instant,
    /// 访问次数
    access_count: usize,
    /// 最后访问时间
    last_accessed: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            expires_at: now + ttl,
            access_count: 0,
            last_accessed: now,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    fn access(&mut self) {
        self.access_count += 1;
        self.last_accessed = Instant::now();
    }

    #[allow(dead_code)]
    fn remaining_ttl(&self) -> Duration {
        self.expires_at.saturating_duration_since(Instant::now())
    }
}

/// 缓存键
#[derive(Debug, Clone)]
pub struct CacheKey {
    /// 键的字符串表示
    key: String,
}

impl CacheKey {
    /// 创建缓存键
    pub fn new(table: &str, id: &str) -> Self {
        Self {
            key: format!("{}:{}", table, id),
        }
    }

    /// 从任意值创建缓存键
    pub fn from_value(table: &str, value: &(impl Hash + ?Sized)) -> Self
    where
        String: std::hash::Hash + std::cmp::Eq,
    {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        let hash = hasher.finish();
        Self {
            key: format!("{}:{:x}", table, hash),
        }
    }
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl PartialEq for CacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for CacheKey {}

/// 缓存策略
#[async_trait]
pub trait CacheStrategy: Send + Sync {
    /// 获取缓存名称
    fn name(&self) -> &'static str;

    /// 获取 TTL
    fn ttl(&self) -> Duration;

    /// 缓存命中时调用
    async fn on_hit(&self, key: &CacheKey);

    /// 缓存未命中时调用
    async fn on_miss(&self, key: &CacheKey);

    /// 缓存更新时调用
    async fn on_update(&self, key: &CacheKey);
}

/// LRU 缓存策略
#[derive(Debug, Default)]
pub struct LruStrategy {
    ttl: Duration,
}

impl LruStrategy {
    /// 创建 LRU 策略
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            ttl: Duration::from_secs(ttl_seconds),
        }
    }
}

#[async_trait]
impl CacheStrategy for LruStrategy {
    fn name(&self) -> &'static str {
        "lru"
    }

    fn ttl(&self) -> Duration {
        self.ttl
    }

    async fn on_hit(&self, _key: &CacheKey) {
        // LRU 策略在访问时自动提升优先级
    }

    async fn on_miss(&self, _key: &CacheKey) {
        // 记录未命中
    }

    async fn on_update(&self, _key: &CacheKey) {
        // 更新时不做特殊处理
    }
}

/// TTLAware 缓存策略 - 包装其他策略，提供 TTL 功能
#[derive(Debug)]
pub struct TtlAwareStrategy<S: CacheStrategy> {
    inner: S,
    /// 默认 TTL
    pub default_ttl: Duration,
}

impl<S: CacheStrategy> TtlAwareStrategy<S> {
    /// 创建带 TTL 的策略
    pub fn new(inner: S, ttl_seconds: u64) -> Self {
        Self {
            inner,
            default_ttl: Duration::from_secs(ttl_seconds),
        }
    }
}

#[async_trait]
impl<S: CacheStrategy> CacheStrategy for TtlAwareStrategy<S> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn ttl(&self) -> Duration {
        self.default_ttl
    }

    #[inline(never)]
    async fn on_hit(&self, key: &CacheKey) {
        let inner = &self.inner;
        inner.on_hit(key).await;
    }

    #[inline(never)]
    async fn on_miss(&self, key: &CacheKey) {
        let inner = &self.inner;
        inner.on_miss(key).await;
    }

    #[inline(never)]
    async fn on_update(&self, key: &CacheKey) {
        let inner = &self.inner;
        inner.on_update(key).await;
    }
}

/// 缓存统计信息
#[derive(Debug, Default)]
pub struct CacheStats {
    /// 命中次数
    pub hits: Arc<std::sync::atomic::AtomicU64>,
    /// 未命中次数
    pub misses: Arc<std::sync::atomic::AtomicU64>,
    /// 设置次数
    pub sets: Arc<std::sync::atomic::AtomicU64>,
    /// 删除次数
    pub deletes: Arc<std::sync::atomic::AtomicU64>,
    /// 过期清除次数
    pub expirations: Arc<std::sync::atomic::AtomicU64>,
}

impl CacheStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self {
            hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            misses: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            sets: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            deletes: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            expirations: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// 获取命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 { 0.0 } else { hits as f64 / total as f64 }
    }

    /// 增加命中计数
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 增加未命中计数
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 增加设置计数
    pub fn record_set(&self) {
        self.sets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 增加删除计数
    pub fn record_delete(&self) {
        self.deletes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 增加过期清除计数
    pub fn record_expiration(&self) {
        self.expirations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 缓存管理器
#[allow(dead_code)]
pub struct CacheManager<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// 内部存储 - 使用 IndexMap 实现 O(1) LRU
    /// IndexMap 维护插入顺序，move_to_end 实现访问顺序更新
    cache: RwLock<IndexMap<CacheKey, CacheEntry<T>>>,
    /// 配置
    config: CacheConfig,
    /// 缓存策略
    strategy: Box<dyn CacheStrategy>,
    /// 统计信息
    stats: CacheStats,
    /// 最大容量
    max_capacity: usize,
}

impl<T> CacheManager<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// 创建缓存管理器
    pub fn new(config: CacheConfig) -> Self {
        Self::with_strategy(config.clone(), Box::new(LruStrategy::new(config.default_ttl)))
    }

    /// 创建带策略的缓存管理器
    pub fn with_strategy(config: CacheConfig, strategy: Box<dyn CacheStrategy>) -> Self {
        Self {
            cache: RwLock::new(IndexMap::new()),
            config: config.clone(),
            strategy,
            stats: CacheStats::new(),
            max_capacity: config.max_capacity,
        }
    }

    /// 获取缓存值 - 读写分离优化版本
    ///
    /// 性能优化：
    /// - 读操作优先使用读锁，多个读取可并发
    /// - 仅在需要更新 LRU 顺序时升级为写锁
    /// - 减少不必要的克隆操作
    pub async fn get(&self, key: &CacheKey) -> Option<T> {
        // 第一阶段：使用读锁检查条目状态
        let cache = self.cache.read().await;

        // 获取条目的不可变引用用于读取
        let entry_ref = match cache.get(key) {
            Some(entry) if !entry.is_expired() => entry,
            Some(_) | None => {
                // 条目不存在或已过期，使用写锁清理
                drop(cache);
                let mut cache = self.cache.write().await;

                if let Some(entry) = cache.get(key) {
                    if entry.is_expired() {
                        cache.shift_remove(key);
                    }
                }

                self.stats.record_miss();
                self.strategy.on_miss(key).await;
                return None;
            }
        };

        // 读取值（仅一次克隆）
        let value = entry_ref.value.clone();
        let ttl = entry_ref.expires_at.saturating_duration_since(Instant::now());

        drop(cache);

        // 第二阶段：使用写锁更新 LRU 顺序
        let mut cache = self.cache.write().await;

        // 重新检查条目是否存在
        if cache.get(key).is_some() {
            // 移除并重新插入到末尾（更新 LRU 顺序）
            cache.shift_remove(key);
            cache.insert(key.clone(), CacheEntry::new(value.clone(), ttl));
        }

        self.stats.record_hit();
        self.strategy.on_hit(key).await;

        Some(value)
    }

    /// 设置缓存值
    pub async fn set(&self, key: CacheKey, value: T) {
        self.set_with_ttl(key, value, self.strategy.ttl()).await;
    }

    /// 设置缓存值（带自定义 TTL）
    pub async fn set_with_ttl(&self, key: CacheKey, value: T, ttl: Duration) {
        let mut cache = self.cache.write().await;

        // 检查容量，必要时淘汰最久未使用的项
        if cache.len() >= self.max_capacity && !cache.contains_key(&key) {
            // IndexMap 的 shift_remove_index 会移除第一个键（最久未使用）
            cache.shift_remove_index(0);
        }

        // 创建新条目
        let entry = CacheEntry::new(value, ttl);

        // 插入或更新条目
        cache.insert(key.clone(), entry);

        self.stats.record_set();
        self.strategy.on_update(&key).await;
    }

    /// 删除缓存值
    pub async fn delete(&self, key: &CacheKey) {
        let mut cache = self.cache.write().await;

        if cache.shift_remove(key).is_some() {
            self.stats.record_delete();
        }
    }

    /// 清空缓存
    pub async fn clear(&mut self) {
        let mut cache = self.cache.write().await;

        cache.clear();
        self.stats = CacheStats::new();
    }

    /// 获取缓存条目数
    pub async fn len(&self) -> usize {
        self.cache.read().await.len()
    }

    /// 检查缓存是否为空
    pub async fn is_empty(&self) -> bool {
        self.cache.read().await.is_empty()
    }

    /// 获取统计信息
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// 清理过期条目
    ///
    /// 使用分批清理策略，避免长时间持有写锁
    /// 每次最多清理 BATCH_SIZE 个过期条目
    pub async fn cleanup(&self) -> usize {
        const BATCH_SIZE: usize = 100;
        let mut total_removed = 0;

        loop {
            let mut cache = self.cache.write().await;

            // 如果缓存很小，直接清理全部
            if cache.len() <= BATCH_SIZE {
                let before = cache.len();
                cache.retain(|_key, entry| {
                    if entry.is_expired() {
                        self.stats.record_expiration();
                        false
                    } else {
                        true
                    }
                });
                total_removed += before - cache.len();
                return total_removed;
            }

            // 大批量只清理一部分
            let keys_to_remove: Vec<_> = cache
                .iter()
                .filter(|(_, entry)| entry.is_expired())
                .take(BATCH_SIZE)
                .map(|(k, _)| k.clone())
                .collect();

            if keys_to_remove.is_empty() {
                return total_removed;
            }

            for key in &keys_to_remove {
                cache.shift_remove(key);
                self.stats.record_expiration();
            }

            total_removed += keys_to_remove.len();
        }
    }

    /// 预热缓存
    ///
    /// 批量加载热点数据到缓存中
    ///
    /// # Arguments
    ///
    /// * `data` - 要预加载的数据列表 (key, value, ttl)
    ///
    /// # Returns
    ///
    /// 返回成功加载的条目数
    pub async fn warmup(&self, data: Vec<(CacheKey, T, Duration)>) -> usize {
        let mut cache = self.cache.write().await;
        let mut loaded = 0;

        for (key, value, ttl) in data {
            if cache.len() < self.max_capacity {
                let entry = CacheEntry::new(value, ttl);
                cache.insert(key, entry);
                loaded += 1;
                self.stats.record_set();
            } else {
                return loaded;
            }
        }

        loaded
    }

    /// 批量预热（使用默认 TTL）
    ///
    /// 批量加载热点数据到缓存中，使用默认 TTL
    ///
    /// # Arguments
    ///
    /// * `data` - 要预加载的数据列表 (key, value)
    ///
    /// # Returns
    ///
    /// 返回成功加载的条目数
    pub async fn warmup_with_default_ttl(&self, data: Vec<(CacheKey, T)>) -> usize {
        let default_ttl = self.strategy.ttl();
        let mut data_with_ttl = Vec::with_capacity(data.len());
        for (key, value) in data {
            data_with_ttl.push((key, value, default_ttl));
        }
        self.warmup(data_with_ttl).await
    }

    /// 批量获取缓存值
    ///
    /// # Arguments
    ///
    /// * `keys` - 要获取的缓存键列表
    ///
    /// # Returns
    ///
    /// 返回缓存值列表，未命中的键返回 None
    pub async fn batch_get(&self, keys: &[CacheKey]) -> Vec<Option<T>> {
        let mut cache = self.cache.write().await;
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            if let Some(entry) = cache.get_mut(key) {
                if !entry.is_expired() {
                    entry.access();
                    let value = entry.value.clone();

                    // 更新 LRU 顺序
                    let ttl = entry.expires_at.saturating_duration_since(Instant::now());
                    cache.shift_remove(key);
                    cache.insert(key.clone(), CacheEntry::new(value, ttl));

                    self.stats.record_hit();
                    self.strategy.on_hit(key).await;

                    results.push(Some(cache.get(key).map(|e| e.value.clone()).unwrap()));
                } else {
                    cache.shift_remove(key);
                    self.stats.record_miss();
                    self.strategy.on_miss(key).await;
                    results.push(None);
                }
            } else {
                self.stats.record_miss();
                self.strategy.on_miss(key).await;
                results.push(None);
            }
        }

        results
    }

    /// 批量设置缓存值
    ///
    /// # Arguments
    ///
    /// * `items` - 要设置的缓存项列表 (key, value)
    ///
    /// # Note
    ///
    /// 使用默认 TTL
    pub async fn batch_set(&self, items: Vec<(CacheKey, T)>) {
        let mut cache = self.cache.write().await;

        for (key, value) in items {
            // 检查容量
            if cache.len() >= self.max_capacity && !cache.contains_key(&key) {
                cache.shift_remove_index(0);
            }

            let entry = CacheEntry::new(value, self.strategy.ttl());
            cache.insert(key.clone(), entry);
            self.stats.record_set();
            self.strategy.on_update(&key).await;
        }
    }

    /// 批量删除缓存值
    ///
    /// # Arguments
    ///
    /// * `keys` - 要删除的缓存键列表
    ///
    /// # Returns
    ///
    /// 返回成功删除的条目数
    pub async fn batch_delete(&self, keys: &[CacheKey]) -> usize {
        let mut cache = self.cache.write().await;
        let mut removed = 0;

        for key in keys {
            if cache.shift_remove(key).is_some() {
                removed += 1;
                self.stats.record_delete();
            }
        }

        removed
    }
}

/// 生成缓存键
pub fn make_cache_key(table_name: &str, id: &str) -> CacheKey {
    CacheKey::new(table_name, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let config = CacheConfig {
            max_capacity: 100,
            default_ttl: 60,
            cleanup_interval: 10,
            enable_stats: true,
        };
        let cache = CacheManager::<String>::new(config);

        let key = CacheKey::new("users", "1");

        // 初始为空
        assert!(cache.get(&key).await.is_none());

        // 设置值
        cache.set(key.clone(), "test_value".to_string()).await;

        // 获取值
        let value = cache.get(&key).await;
        assert_eq!(value, Some("test_value".to_string()));

        // 统计信息
        assert_eq!(cache.stats().hits.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(cache.stats().misses.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_cache_ttl() {
        let config = CacheConfig {
            max_capacity: 100,
            default_ttl: 1,
            cleanup_interval: 10,
            enable_stats: true,
        };
        let cache = CacheManager::<String>::new(config);

        let key = CacheKey::new("users", "1");
        cache.set(key.clone(), "test_value".to_string()).await;

        // 立即获取应该成功
        assert!(cache.get(&key).await.is_some());

        // 等待过期
        tokio::time::sleep(Duration::from_secs(2)).await;

        // 过期后获取应该失败
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let config = CacheConfig {
            max_capacity: 3,
            default_ttl: 60,
            cleanup_interval: 10,
            enable_stats: true,
        };
        let cache = CacheManager::<String>::new(config);

        // 添加 3 个条目
        for i in 0..3 {
            let key = CacheKey::new("users", &i.to_string());
            cache.set(key, format!("value_{}", i)).await;
        }

        assert_eq!(cache.len().await, 3);

        // 添加第 4 个条目，应该触发淘汰
        let key = CacheKey::new("users", "3");
        cache.set(key.clone(), "value_3".to_string()).await;

        // 应该有 3 个条目（淘汰了 1 个）
        assert_eq!(cache.len().await, 3);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let config = CacheConfig::default();
        let mut cache = CacheManager::<String>::new(config);

        let key = CacheKey::new("users", "1");
        cache.set(key.clone(), "test".to_string()).await;

        assert!(!cache.is_empty().await);

        cache.clear().await;

        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let config = CacheConfig::default();
        let cache = CacheManager::<String>::new(config);

        let key = CacheKey::new("users", "1");

        // 未命中
        cache.get(&key).await;
        assert_eq!(cache.stats().misses.load(std::sync::atomic::Ordering::Relaxed), 1);

        // 设置
        cache.set(key.clone(), "value".to_string()).await;
        assert_eq!(cache.stats().sets.load(std::sync::atomic::Ordering::Relaxed), 1);

        // 命中
        cache.get(&key).await;
        assert_eq!(cache.stats().hits.load(std::sync::atomic::Ordering::Relaxed), 1);

        // 删除
        cache.delete(&key).await;
        assert_eq!(cache.stats().deletes.load(std::sync::atomic::Ordering::Relaxed), 1);

        // 命中率
        assert!((cache.stats().hit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cache_key_from_value_and_hash() {
        let k1 = CacheKey::from_value("users", &"abc");
        let k2 = CacheKey::from_value("users", &"abc");
        let k3 = CacheKey::from_value("users", &"def");

        assert_eq!(k1, k2);
        assert_ne!(k1, k3);

        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        k1.hash(&mut h1);
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        k2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());

        let k4 = make_cache_key("t", "1");
        assert_eq!(k4, CacheKey::new("t", "1"));
    }

    #[test]
    fn test_cache_entry_remaining_ttl() {
        let entry = CacheEntry::new("v".to_string(), Duration::from_secs(1));
        let remaining = entry.remaining_ttl();
        assert!(remaining <= Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_cache_cleanup_small_and_large_batches() {
        let config = CacheConfig {
            max_capacity: 1000,
            default_ttl: 1,
            cleanup_interval: 10,
            enable_stats: true,
        };
        let cache = CacheManager::<String>::new(config);

        cache
            .set_with_ttl(CacheKey::new("t", "1"), "v1".to_string(), Duration::from_millis(0))
            .await;
        cache
            .set_with_ttl(CacheKey::new("t", "2"), "v2".to_string(), Duration::from_secs(60))
            .await;

        let removed_small = cache.cleanup().await;
        assert_eq!(removed_small, 1);

        for i in 0..150 {
            cache
                .set_with_ttl(
                    CacheKey::new("batch", &i.to_string()),
                    i.to_string(),
                    Duration::from_millis(0),
                )
                .await;
        }

        let removed_large = cache.cleanup().await;
        assert!(removed_large > 0);
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn test_cache_warmup_and_batch_ops() {
        let config = CacheConfig {
            max_capacity: 2,
            default_ttl: 60,
            cleanup_interval: 10,
            enable_stats: true,
        };
        let cache = CacheManager::<String>::new(config);

        let loaded = cache
            .warmup(vec![
                (CacheKey::new("w", "1"), "a".to_string(), Duration::from_secs(60)),
                (CacheKey::new("w", "2"), "b".to_string(), Duration::from_secs(60)),
                (CacheKey::new("w", "3"), "c".to_string(), Duration::from_secs(60)),
            ])
            .await;
        assert_eq!(loaded, 2);

        let loaded2 = cache
            .warmup_with_default_ttl(vec![(CacheKey::new("w", "4"), "d".to_string())])
            .await;
        assert_eq!(loaded2, 0);

        let k_hit = CacheKey::new("b", "hit");
        let k_expired = CacheKey::new("b", "expired");
        let k_miss = CacheKey::new("b", "miss");

        cache
            .set_with_ttl(k_hit.clone(), "vh".to_string(), Duration::from_secs(60))
            .await;
        cache
            .set_with_ttl(k_expired.clone(), "ve".to_string(), Duration::from_millis(0))
            .await;

        let got = cache
            .batch_get(&[k_hit.clone(), k_expired.clone(), k_miss.clone()])
            .await;
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], Some("vh".to_string()));
        assert_eq!(got[1], None);
        assert_eq!(got[2], None);

        cache
            .batch_set(vec![
                (CacheKey::new("s", "1"), "v1".to_string()),
                (CacheKey::new("s", "2"), "v2".to_string()),
            ])
            .await;

        let removed = cache
            .batch_delete(&[CacheKey::new("s", "1"), CacheKey::new("s", "nope")])
            .await;
        assert_eq!(removed, 1);
    }

    #[tokio::test]
    async fn test_cache_warmup_full_load_returns_loaded() {
        let config = CacheConfig {
            max_capacity: 10,
            default_ttl: 60,
            cleanup_interval: 10,
            enable_stats: true,
        };
        let cache = CacheManager::<String>::new(config);

        let loaded = cache
            .warmup(vec![
                (CacheKey::new("wf", "1"), "a".to_string(), Duration::from_secs(60)),
                (CacheKey::new("wf", "2"), "b".to_string(), Duration::from_secs(60)),
            ])
            .await;
        assert_eq!(loaded, 2);
        assert_eq!(cache.len().await, 2);
    }

    #[test]
    fn test_lru_strategy_name_and_ttl() {
        let strategy = LruStrategy::new(7);
        assert_eq!(strategy.name(), "lru");
        assert_eq!(strategy.ttl(), Duration::from_secs(7));
    }

    #[tokio::test]
    async fn test_ttl_aware_strategy_delegation() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Debug, Clone)]
        struct CountingStrategy {
            hits: Arc<AtomicUsize>,
            misses: Arc<AtomicUsize>,
            updates: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl CacheStrategy for CountingStrategy {
            fn name(&self) -> &'static str {
                "counting"
            }

            fn ttl(&self) -> Duration {
                Duration::from_secs(1)
            }

            async fn on_hit(&self, _key: &CacheKey) {
                self.hits.fetch_add(1, Ordering::Relaxed);
            }

            async fn on_miss(&self, _key: &CacheKey) {
                self.misses.fetch_add(1, Ordering::Relaxed);
            }

            async fn on_update(&self, _key: &CacheKey) {
                self.updates.fetch_add(1, Ordering::Relaxed);
            }
        }

        let hits = Arc::new(AtomicUsize::new(0));
        let misses = Arc::new(AtomicUsize::new(0));
        let updates = Arc::new(AtomicUsize::new(0));

        let inner = CountingStrategy {
            hits: hits.clone(),
            misses: misses.clone(),
            updates: updates.clone(),
        };

        let wrapped = TtlAwareStrategy::new(inner, 123);
        assert_eq!(wrapped.name(), "counting");
        assert_eq!(wrapped.ttl(), Duration::from_secs(123));
        assert_eq!(wrapped.default_ttl, Duration::from_secs(123));

        let key = CacheKey::new("t", "1");
        wrapped.on_hit(&key).await;
        wrapped.on_miss(&key).await;
        wrapped.on_update(&key).await;

        assert_eq!(hits.load(Ordering::Relaxed), 1);
        assert_eq!(misses.load(Ordering::Relaxed), 1);
        assert_eq!(updates.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_cache_cleanup_large_batch_no_expired_breaks() {
        let config = CacheConfig {
            max_capacity: 1000,
            default_ttl: 3600,
            cleanup_interval: 10,
            enable_stats: true,
        };
        let cache = CacheManager::<String>::new(config);

        for i in 0..101 {
            cache
                .set_with_ttl(
                    CacheKey::new("keep", &i.to_string()),
                    format!("v{}", i),
                    Duration::from_secs(3600),
                )
                .await;
        }
        assert!(cache.len().await > 100);

        let removed = cache.cleanup().await;
        assert_eq!(removed, 0);
        assert!(cache.len().await > 100);
    }

    #[tokio::test]
    async fn test_cache_cleanup_large_batch_multiple_iterations() {
        let config = CacheConfig {
            max_capacity: 1000,
            default_ttl: 3600,
            cleanup_interval: 10,
            enable_stats: true,
        };
        let cache = CacheManager::<String>::new(config);

        for i in 0..210 {
            cache
                .set_with_ttl(
                    CacheKey::new("exp", &i.to_string()),
                    format!("v{}", i),
                    Duration::from_secs(3600),
                )
                .await;
        }
        for i in 0..10 {
            cache
                .set_with_ttl(
                    CacheKey::new("keep", &i.to_string()),
                    format!("k{}", i),
                    Duration::from_secs(3600),
                )
                .await;
        }

        assert_eq!(cache.len().await, 220);

        let (exp_key_count, keep_key_count) = {
            let cache_guard = cache.cache.read().await;
            let exp_count = cache_guard.keys().filter(|k| k.key.starts_with("exp:")).count();
            let keep_count = cache_guard.keys().filter(|k| k.key.starts_with("keep:")).count();
            (exp_count, keep_count)
        };
        assert_eq!(exp_key_count, 210);
        assert_eq!(keep_key_count, 10);

        {
            let now = Instant::now();
            let mut cache_guard = cache.cache.write().await;
            for (key, entry) in cache_guard.iter_mut() {
                if key.key.starts_with("exp:") {
                    entry.expires_at = now - Duration::from_secs(1);
                }
            }
        }

        let expired_after = {
            let cache_guard = cache.cache.read().await;
            cache_guard.iter().filter(|(_, entry)| entry.is_expired()).count()
        };
        assert_eq!(expired_after, 210);

        let removed = cache.cleanup().await;
        assert_eq!(removed, 210);
        assert_eq!(cache.len().await, 10);
    }
}
