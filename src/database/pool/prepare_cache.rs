// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 语句级 prepared statement LRU 缓存
//!
//! 按连接池复用的语句缓存端口：以 SQL 文本为键做容量有界 LRU，
//! `get_or_prepare` 保证同一语句的准备逻辑（校验/解析/驱动 prepare）
//! 在缓存命中时不重复执行，并输出命中/未命中/淘汰计数。
//!
//! # 口径说明
//!
//! dbnexus 经 sea-orm/sqlx 访问数据库，驱动层已按连接维护 wire 级
//! prepared statement；本缓存工作在语句**决策层**——把"该语句是否已
//! 准备/校验"的结果按池（而非按连接）共享，供执行路径跳过重复准备
//! 开销，并暴露命中率指标。`prepare` 闭包返回的值（校验结论、驱动
//! 句柄包装等）由调用方定义，缓存对其透明。
//!
//! # 示例
//!
//! ```ignore
//! let cache = PreparedStatementCache::new(128);
//! let (value, hit) = cache.get_or_prepare("SELECT 1", |sql| prepare_driver(sql));
//! assert!(!hit); // 首次未命中
//! let (_, hit) = cache.get_or_prepare("SELECT 1", |sql| prepare_driver(sql));
//! assert!(hit);  // 二次命中
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 池级缓存实例别名（DbPool 集成口径 —— 就绪标记 + 命中率指标；
/// 下游如需缓存驱动句柄可用自定义 V 的 PreparedStatementCache）
pub type PoolPrepareCache = PreparedStatementCache<()>;

/// 缓存命中率统计
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrepareCacheStats {
    /// 命中次数
    pub hits: u64,
    /// 未命中（实际执行 prepare）次数
    pub misses: u64,
    /// LRU 淘汰次数
    pub evictions: u64,
    /// 当前缓存语句数
    pub size: usize,
}

/// 缓存条目：调用方准备产物（SQL 由 map 键承载，条目不重复存储）
struct Entry<V> {
    value: Arc<V>,
    /// LRU 访问时钟（每次命中/插入递增，容量淘汰时剔除最小者）
    last_used: u64,
}

/// 语句级 LRU 缓存
///
/// 线程安全（内部 `Mutex`）；容量上限在构造时固定，淘汰策略为
/// 最近最少使用（按访问时钟）。
pub struct PreparedStatementCache<V> {
    capacity: usize,
    inner: Mutex<LruState<V>>,
}

struct LruState<V> {
    map: HashMap<Arc<str>, Entry<V>>,
    clock: u64,
    hits: u64,
    misses: u64,
    evictions: u64,
}

impl<V> PreparedStatementCache<V> {
    /// 创建容量为 `capacity` 的缓存（容量 0 视为 1）
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            inner: Mutex::new(LruState {
                map: HashMap::new(),
                clock: 0,
                hits: 0,
                misses: 0,
                evictions: 0,
            }),
        }
    }

    /// 取缓存的准备产物；未命中或容量为 1 的覆盖场景调用 `prepare`
    ///
    /// 返回 `(产物, 是否命中)`。`prepare` 只在未命中时被调用。
    pub fn get_or_prepare(
        &self,
        sql: impl Into<Arc<str>>,
        prepare: impl FnOnce(&str) -> V,
    ) -> (Arc<V>, bool) {
        let key: Arc<str> = sql.into();
        // 锁中毒可恢复：条目状态在 prepare 调用点前后均保持一致，取回内部
        // 数据继续服务（避免单次 panic 永久杀死整个缓存）
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.clock += 1;
        let clock = state.clock;

        // 命中路径：刷新访问时钟
        if let Some(entry) = state.map.get_mut(&key) {
            entry.last_used = clock;
            let value = Arc::clone(&entry.value);
            state.hits += 1;
            return (value, true);
        }

        // 未命中：执行 prepare
        state.misses += 1;
        let value = Arc::new(prepare(&key));

        // 容量已满 → 淘汰最久未使用条目
        if state.map.len() >= self.capacity
            && let Some(oldest_key) = state
                .map
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| Arc::clone(key))
        {
            state.map.remove(&oldest_key);
            state.evictions += 1;
        }

        state.map.insert(
            Arc::clone(&key),
            Entry {
                value: Arc::clone(&value),
                last_used: clock,
            },
        );
        (value, false)
    }

    /// 当前统计快照
    pub fn stats(&self) -> PrepareCacheStats {
        let state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        PrepareCacheStats {
            hits: state.hits,
            misses: state.misses,
            evictions: state.evictions,
            size: state.map.len(),
        }
    }

    /// 缓存容量
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// 命中断言：同一语句的 prepare 只执行一次
    #[test]
    fn test_second_access_is_cache_hit() {
        let cache: PreparedStatementCache<String> = PreparedStatementCache::new(8);
        let prepares = AtomicU32::new(0);

        let (v1, hit1) = cache.get_or_prepare("SELECT 1", |sql| {
            prepares.fetch_add(1, Ordering::SeqCst);
            format!("prepared:{sql}")
        });
        assert!(!hit1, "首次应未命中");
        assert_eq!(&*v1, "prepared:SELECT 1");

        let (v2, hit2) = cache.get_or_prepare("SELECT 1", |sql| {
            prepares.fetch_add(1, Ordering::SeqCst);
            format!("prepared:{sql}")
        });
        assert!(hit2, "二次应命中");
        assert_eq!(&*v2, "prepared:SELECT 1", "命中应返回同一准备产物");
        assert_eq!(prepares.load(Ordering::SeqCst), 1, "prepare 只应执行一次");

        // 不同语句 → 未命中
        let (_, hit3) = cache.get_or_prepare("SELECT 2", |_| String::new());
        assert!(!hit3);

        let stats = cache.stats();
        assert_eq!((stats.hits, stats.misses), (1, 2));
        assert_eq!(stats.size, 2);
    }

    /// LRU 淘汰：容量满后剔除最久未使用条目
    #[test]
    fn test_lru_eviction_respects_recency() {
        let cache: PreparedStatementCache<()> = PreparedStatementCache::new(2);

        assert!(!cache.get_or_prepare("a", |_| ()).1, "a 首次未命中");
        assert!(!cache.get_or_prepare("b", |_| ()).1, "b 首次未命中");
        // 访问 a → b 成为最久未使用
        assert!(cache.get_or_prepare("a", |_| ()).1);
        // 插入 c → 容量满，淘汰 b
        assert!(!cache.get_or_prepare("c", |_| ()).1);

        let stats = cache.stats();
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.size, 2);
        assert_eq!(cache.capacity(), 2);
        // b 已被淘汰（重新探测为未命中）
        assert!(!cache.get_or_prepare("b", |_| ()).1, "b 应已被淘汰");
    }
}
