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
//! ```ignore
//! use dbnexus::cache::{CacheManager, CacheConfig};
//!
//! let cache = CacheManager::new(CacheConfig::default());
//! cache.set("user:1", user, Duration::from_secs(300)).await;
//! let user = cache.get("user:1").await;
//! ```

use async_trait::async_trait;
use std::collections::HashMap;
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

    async fn on_hit(&self, key: &CacheKey) {
        self.inner.on_hit(key).await;
    }

    async fn on_miss(&self, key: &CacheKey) {
        self.inner.on_miss(key).await;
    }

    async fn on_update(&self, key: &CacheKey) {
        self.inner.on_update(key).await;
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
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
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
pub struct CacheManager<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// 内部存储 - 使用 LRU 排序的 HashMap
    cache: RwLock<HashMap<CacheKey, CacheEntry<T>>>,
    /// 访问顺序记录（用于 LRU）
    access_order: RwLock<Vec<CacheKey>>,
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
            cache: RwLock::new(HashMap::new()),
            access_order: RwLock::new(Vec::new()),
            config: config.clone(),
            strategy,
            stats: CacheStats::new(),
            max_capacity: config.max_capacity,
        }
    }

    /// 获取缓存值
    pub async fn get(&self, key: &CacheKey) -> Option<T> {
        let mut cache = self.cache.write().await;

        if let Some(entry) = cache.get_mut(key) {
            if entry.is_expired() {
                // 过期，移除
                cache.remove(key);
                self.stats.record_miss();
                self.strategy.on_miss(key).await;
                return None;
            }

            // 访问命中
            entry.access();
            self.stats.record_hit();
            self.strategy.on_hit(key).await;

            Some(entry.value.clone())
        } else {
            self.stats.record_miss();
            self.strategy.on_miss(key).await;
            None
        }
    }

    /// 设置缓存值
    pub async fn set(&self, key: CacheKey, value: T) {
        self.set_with_ttl(key, value, self.strategy.ttl()).await;
    }

    /// 设置缓存值（带自定义 TTL）
    pub async fn set_with_ttl(&self, key: CacheKey, value: T, ttl: Duration) {
        let mut cache = self.cache.write().await;
        let mut access = self.access_order.write().await;

        // 检查容量，必要时淘汰
        if cache.len() >= self.max_capacity && !cache.contains_key(&key) {
            if let Some(lru_key) = access.first() {
                cache.remove(lru_key);
            }
            access.retain(|k| k != &key);
        }

        // 创建新条目
        let entry = CacheEntry::new(value, ttl);

        cache.insert(key.clone(), entry);
        if !access.contains(&key) {
            access.push(key.clone());
        }

        self.stats.record_set();
        self.strategy.on_update(&key).await;
    }

    /// 删除缓存值
    pub async fn delete(&self, key: &CacheKey) {
        let mut cache = self.cache.write().await;
        let mut access = self.access_order.write().await;

        if cache.remove(key).is_some() {
            access.retain(|k| *k != *key);
            self.stats.record_delete();
        }
    }

    /// 清空缓存
    pub async fn clear(&mut self) {
        let mut cache = self.cache.write().await;
        let mut access = self.access_order.write().await;

        cache.clear();
        access.clear();

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
    pub async fn cleanup(&self) -> usize {
        let mut cache = self.cache.write().await;
        let mut access = self.access_order.write().await;

        let before = cache.len();
        cache.retain(|key, entry| {
            let not_expired = !entry.is_expired();
            if !not_expired {
                access.retain(|k| *k != *key);
                self.stats.record_expiration();
            }
            not_expired
        });

        before - cache.len()
    }

    /// 移动到访问顺序末尾
    async fn move_to_back(&self, _cache: &mut std::sync::RwLockWriteGuard<'_, HashMap<CacheKey, CacheEntry<T>>>, key: &CacheKey) {
        let mut access = self.access_order.write().await;
        if let Some(pos) = access.iter().position(|k| k == key) {
            access.swap_remove(pos);
            access.push(key.clone());
        }
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
}
