// Copyright (c) 2025-2026 Kirky.X🌠
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

/// 链表节点：槽位池存储，prev/next 为槽位索引（None 即链端）；
/// value 为调用方准备产物，SQL 由 map 键承载（节点持有同值 Arc 供淘汰时清理 map）
struct Node<V> {
    key: Arc<str>,
    value: Arc<V>,
    prev: Option<usize>,
    next: Option<usize>,
}

/// 语句级 LRU 缓存
///
/// 线程安全（内部 `Mutex`）；容量上限在构造时固定，淘汰策略为最近最少
/// 使用：侵入式双链表维护访问序，命中/插入前移链首、淘汰摘链尾，
/// 全部操作 O(1)（无满容量线性扫描串行点）。
pub struct PreparedStatementCache<V> {
    capacity: usize,
    inner: Mutex<LruState<V>>,
}

struct LruState<V> {
    /// 键 → 槽位索引（节点顺序与统计由链表/计数承载）
    map: HashMap<Arc<str>, usize>,
    /// 节点槽位池（含空闲槽；淘汰回收的槽位经 free 复用，避免重复分配）
    slab: Vec<Node<V>>,
    free: Vec<usize>,
    /// 访问序双链表：head 为最近使用，tail 为最久未使用
    head: Option<usize>,
    tail: Option<usize>,
    hits: u64,
    misses: u64,
    evictions: u64,
}

impl<V> LruState<V> {
    /// 从访问序链表摘除槽位节点（不动 map 与 free）
    fn unlink(&mut self, idx: usize) {
        let (prev, next) = (self.slab[idx].prev, self.slab[idx].next);
        match prev {
            Some(p) => self.slab[p].next = next,
            None => self.head = next,
        }
        match next {
            Some(n) => self.slab[n].prev = prev,
            None => self.tail = prev,
        }
        self.slab[idx].prev = None;
        self.slab[idx].next = None;
    }

    /// 将槽位节点链接到链首（最近使用端）
    fn push_front(&mut self, idx: usize) {
        self.slab[idx].prev = None;
        self.slab[idx].next = self.head;
        if let Some(old_head) = self.head {
            self.slab[old_head].prev = Some(idx);
        } else {
            self.tail = Some(idx);
        }
        self.head = Some(idx);
    }

    /// 命中刷新：已在链中的节点前移至链首
    fn touch(&mut self, idx: usize) {
        if self.head != Some(idx) {
            self.unlink(idx);
            self.push_front(idx);
        }
    }

    /// 淘汰链尾（最久未使用）：摘链、清 map 并回收槽位
    fn evict_lru(&mut self) {
        let tail = self.tail.expect("淘汰仅在容量已满的非空缓存发生");
        self.unlink(tail);
        let key = Arc::clone(&self.slab[tail].key);
        self.map.remove(&key);
        self.free.push(tail);
        self.evictions += 1;
    }

    /// 分配槽位：优先复用淘汰回收的空闲槽，否则追加
    fn alloc_slot(&mut self, key: Arc<str>, value: Arc<V>) -> usize {
        let node = Node {
            key,
            value,
            prev: None,
            next: None,
        };
        match self.free.pop() {
            Some(idx) => {
                self.slab[idx] = node;
                idx
            }
            None => {
                self.slab.push(node);
                self.slab.len() - 1
            }
        }
    }
}

impl<V> PreparedStatementCache<V> {
    /// 创建容量为 `capacity` 的缓存（容量 0 视为 1）
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            inner: Mutex::new(LruState {
                map: HashMap::new(),
                slab: Vec::new(),
                free: Vec::new(),
                head: None,
                tail: None,
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

        // 命中路径：条目前移至链首（刷新访问序）
        if let Some(&idx) = state.map.get(&key) {
            state.touch(idx);
            let value = Arc::clone(&state.slab[idx].value);
            state.hits += 1;
            return (value, true);
        }

        // 未命中：执行 prepare
        state.misses += 1;
        let value = Arc::new(prepare(&key));

        // 容量已满 → 淘汰链尾（最久未使用，O(1)）
        if state.map.len() >= self.capacity {
            state.evict_lru();
        }

        let idx = state.alloc_slot(Arc::clone(&key), Arc::clone(&value));
        state.push_front(idx);
        state.map.insert(key, idx);
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

    /// 探测缓存条目；命中刷新访问序并计入命中统计，未命中计入未命中统计
    ///
    /// 与 [`Self::get_or_prepare`] 的差异：不执行 prepare 闭包，命中与否由
    /// 调用方分支处理（如解析失败的结果不允许入缓存）。
    pub fn get(&self, key: &str) -> Option<Arc<V>> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match state.map.get(key).copied() {
            Some(idx) => {
                state.touch(idx);
                let value = Arc::clone(&state.slab[idx].value);
                state.hits += 1;
                Some(value)
            }
            None => {
                state.misses += 1;
                None
            }
        }
    }

    /// 插入条目；键已存在时原位覆盖（前移访问序，不触发淘汰），
    /// 新键在容量已满时先淘汰最久未使用条目
    pub fn insert(&self, key: impl Into<Arc<str>>, value: V) {
        let key: Arc<str> = key.into();
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(&idx) = state.map.get(&key) {
            state.slab[idx].value = Arc::new(value);
            state.touch(idx);
            return;
        }
        if state.map.len() >= self.capacity {
            state.evict_lru();
        }
        let idx = state.alloc_slot(Arc::clone(&key), Arc::new(value));
        state.push_front(idx);
        state.map.insert(key, idx);
    }

    /// 清空全部条目并重置命中/未命中/淘汰统计
    pub fn clear(&self) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.map.clear();
        state.slab.clear();
        state.free.clear();
        state.head = None;
        state.tail = None;
        state.hits = 0;
        state.misses = 0;
        state.evictions = 0;
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

    /// 探测语义：命中/未命中计入统计，命中刷新访问时钟（影响后续淘汰对象）
    #[test]
    fn test_get_probes_and_refreshes_recency() {
        let cache: PreparedStatementCache<u32> = PreparedStatementCache::new(2);

        assert_eq!(cache.get("a"), None, "空缓存探测应未命中");
        let stats = cache.stats();
        assert_eq!((stats.hits, stats.misses), (0, 1));

        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(cache.get("a").map(|v| *v), Some(1), "探测命中应返回条目");
        // 命中 a → b 成为最久未使用；新键 c 插入时淘汰 b
        cache.insert("c", 3);
        assert_eq!(cache.get("b"), None, "b 应被淘汰");
        assert_eq!(cache.get("a").map(|v| *v), Some(1), "a 应仍存活");

        let stats = cache.stats();
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.size, 2);
    }

    /// 插入语义：同键原位覆盖（不触发淘汰），统计只记淘汰不记命中/未命中
    #[test]
    fn test_insert_overwrites_in_place() {
        let cache: PreparedStatementCache<u32> = PreparedStatementCache::new(2);
        cache.insert("a", 1);
        cache.insert("a", 10);

        assert_eq!(cache.get("a").map(|v| *v), Some(10), "同键应覆盖原值");
        let stats = cache.stats();
        assert_eq!(stats.size, 1, "覆盖不得新增条目");
        assert_eq!(stats.evictions, 0, "覆盖不得触发淘汰");
        // 唯一一次命中来自上面的 get 探测；插入自身不计入命中/未命中
        assert_eq!((stats.hits, stats.misses), (1, 0), "插入不计入命中/未命中");
    }

    /// 清空语义：条目与统计一并归零
    #[test]
    fn test_clear_resets_entries_and_stats() {
        let cache: PreparedStatementCache<u32> = PreparedStatementCache::new(2);
        cache.insert("a", 1);
        let _ = cache.get("a");
        cache.insert("b", 2);
        cache.insert("c", 3);

        cache.clear();
        let stats = cache.stats();
        assert_eq!(stats.size, 0);
        assert_eq!((stats.hits, stats.misses, stats.evictions), (0, 0, 0));
        assert_eq!(cache.get("a"), None, "清空后条目不可达");
    }

    /// 满容量稳态循环：槽位回收复用下淘汰对象仍严格按 LRU 访问序
    /// （逐 victims 断言长序列，链表槽位错链即在此红灯）
    #[test]
    fn test_sustained_eviction_keeps_recency_order() {
        let cache: PreparedStatementCache<u32> = PreparedStatementCache::new(3);

        // 装满：a → b → c（访问序 c > b > a）
        for (k, v) in [("a", 1), ("b", 2), ("c", 3)] {
            cache.insert(k, v);
        }
        // 命中 a → 访问序 a > c > b
        assert_eq!(cache.get("a").map(|v| *v), Some(1));

        // 稳态循环：每轮 1 条新键进、淘汰最久未使用者（槽位全部走 free
        // 回收路径），淘汰对象预期 b → c → a → n0 → n1 → n2
        let expected_victims = ["b", "c", "a", "n0", "n1", "n2"];
        for (round, victim) in expected_victims.iter().enumerate() {
            cache.insert(format!("n{round}"), round as u32);
            assert_eq!(
                cache.get(victim),
                None,
                "第 {round} 轮应淘汰最久未使用的 {victim}"
            );
        }

        // 循环后存活集合恰为最新 3 条（n3/n4/n5），统计账目一致
        let stats = cache.stats();
        assert_eq!(stats.size, 3);
        assert_eq!(stats.evictions, 6);
        for k in ["n3", "n4", "n5"] {
            assert!(cache.get(k).is_some(), "{k} 应存活");
        }
        assert_eq!(cache.stats().size, 3, "存活集合不得越界");
    }
}
