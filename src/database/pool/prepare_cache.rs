// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 语句级 prepared statement 缓存（oxcache 承载）
//!
//! 按连接池复用的语句缓存端口：以 SQL 文本为键做容量有界缓存（oxcache
//! `Cache`，原生同步后端 + `sync_mode`），`get_or_prepare` 保证同一语句的
//! 准备逻辑（校验/解析/驱动 prepare）在顺序访问的缓存命中时不重复执行，
//! 并输出命中/未命中计数。
//!
//! # 淘汰语义（手写 LRU → oxcache 后端 FIFO）
//!
//! 淘汰由 oxcache memory 后端承载，为插入序 FIFO 容量淘汰（非严格
//! LRU）：访问不改变淘汰序，满容量时淘汰最早插入的条目。原手写侵入式
//! 双链表 LRU 的逐 victim 精确序列断言已删除，测试改锚容量有界 +
//! 统计账目一致 + 最新条目存活三性质。逐出计数后端不暴露，
//! `PrepareCacheStats::evictions` 恒 0，由 `size` 容量界与命中率口径观测。
//!
//! # 并发语义差异（如实降级）
//!
//! 同步面 get→set 存在窗口，并发双未命中可能双 prepare：prepare 为
//! 纯校验函数（决策层，无副作用），双执行的代价可接受；顺序语义不受
//! 影响（每键至多执行一次 prepare，既有顺序测试继续钉住）。未如原
//! 实现持 Mutex 跨 prepare 调用——不为"至多一次"自建 per-key in-flight
//! 守卫（见 design D2 Alternatives）。
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

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use oxcache::{Cache, DashMapMemoryBackend};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::i18n;

/// 池级缓存实例别名（DbPool 集成口径 —— 就绪标记 + 命中率指标；
/// 下游如需缓存驱动句柄可用自定义 V 的 PreparedStatementCache）
// 生产消费方（enable_prepare_cache/execute_cached）随 prepare-cache 门控；
// 仅 sql-parser 路径（无 prepare-cache）编译本模块时无生产调用点
#[cfg_attr(not(feature = "prepare-cache"), allow(dead_code))]
pub type PoolPrepareCache = PreparedStatementCache<()>;

/// 缓存命中率统计
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrepareCacheStats {
    /// 命中次数
    pub hits: u64,
    /// 未命中（实际执行 prepare）次数
    pub misses: u64,
    /// 逐出次数
    ///
    /// oxcache memory 后端不暴露淘汰计数，本字段恒为 0：淘汰行为经
    /// `size` 容量界与命中率口径观测（池级就绪标记 + 命中率指标为生产
    /// 口径），不为此自建计数状态。
    pub evictions: u64,
    /// 当前缓存语句数（oxcache memory 后端条目计数）
    pub size: usize,
}

/// 语句级缓存（oxcache 承载）
///
/// 以 SQL 文本为键的容量有界缓存：存储经 oxcache `Cache`（
/// [`DashMapMemoryBackend`] 原生同步后端 + `sync_mode`）承载，淘汰为
/// 插入序 FIFO 容量淘汰（非严格 LRU——访问不改变淘汰序，与手写版
/// 侵入式链表 LRU 的逐出对象集合在极端访问序下不同）。
///
/// 泛型 `V` 需可 serde 序列化（oxcache 存储面的往返约束）。
pub struct PreparedStatementCache<V>
where
    V: Serialize + DeserializeOwned,
{
    /// oxcache 实例（同步面，无需 tokio runtime 即可读写）
    inner: Cache<String, V>,
    /// 命中计数（oxcache 同步读路径不产出 recorder 事件，命中率为
    /// 生产口径，由本轻量计数承载）
    hits: AtomicU64,
    /// 未命中计数
    misses: AtomicU64,
}

impl<V> PreparedStatementCache<V>
where
    V: Serialize + DeserializeOwned,
{
    /// 创建容量为 `capacity` 的缓存（容量 0 视为 1）
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        // 原生同步后端：默认 moka 后端的同步面是 block_in_place 桥，
        // 在 current-thread tokio runtime 内返回 NotSupported——
        // execute_cached 位于 async fn 内，必须全 runtime 口味可用
        let backend = DashMapMemoryBackend::builder().capacity(capacity).build();
        let inner = Cache::<String, V>::builder()
            .sync_backend_arc(Arc::new(backend))
            .sync_mode(true)
            .build_sync()
            .expect("building the oxcache user sync backend should not fail (no remaining fallible branches)");
        Self {
            inner,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// 取缓存的准备产物；未命中调用 `prepare` 并写入缓存
    ///
    /// 返回 `(产物, 是否命中)`。`prepare` 只在未命中时被调用；顺序访问下
    /// 每键至多执行一次。并发双未命中可能双 prepare（同步面 get→set 存在
    /// 窗口，无 single-flight；prepare 为纯校验函数无副作用，见模块 doc）。
    // 生产消费方 `execute_cached` 随 prepare-cache 门控；仅 sql-parser
    // 路径编译本模块时该方法仅由单元测试消费，保留为单一实现
    #[cfg_attr(not(feature = "prepare-cache"), allow(dead_code))]
    pub fn get_or_prepare(
        &self,
        sql: impl Into<Arc<str>>,
        prepare: impl FnOnce(&str) -> V,
    ) -> (Arc<V>, bool) {
        let key: Arc<str> = sql.into();
        let cache_key = key.to_string();
        match self.inner.get_sync(&cache_key) {
            Ok(Some(value)) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return (Arc::new(value), true);
            }
            Ok(None) => {}
            Err(e) => {
                log::warn!(
                    "{}",
                    i18n::t(
                        "prepare-cache-log-read-miss",
                        &[
                            ("op", "get_or_prepare".to_string()),
                            ("error", e.to_string())
                        ]
                    )
                );
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        let value = Arc::new(prepare(&key));
        if let Err(e) = self.inner.set_sync(&cache_key, &value) {
            log::warn!(
                "{}",
                i18n::t(
                    "prepare-cache-log-write-failed",
                    &[
                        ("op", "get_or_prepare".to_string()),
                        ("error", e.to_string())
                    ]
                )
            );
        }
        (value, false)
    }

    /// 当前统计快照
    pub fn stats(&self) -> PrepareCacheStats {
        let size = self.inner.len_sync().unwrap_or(0) as usize;
        PrepareCacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            evictions: 0,
            size,
        }
    }

    /// 探测缓存条目；命中计入命中统计，未命中计入未命中统计
    ///
    /// 与 [`Self::get_or_prepare`] 的差异：不执行 prepare 闭包，命中与否由
    /// 调用方分支处理（如解析失败的结果不允许入缓存）。
    pub fn get(&self, key: &str) -> Option<Arc<V>> {
        match self.inner.get_sync(&key.to_string()) {
            Ok(Some(value)) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(Arc::new(value))
            }
            Ok(None) => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
            Err(e) => {
                log::warn!(
                    "{}",
                    i18n::t(
                        "prepare-cache-log-read-miss",
                        &[("op", "get".to_string()), ("error", e.to_string())]
                    )
                );
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// 插入条目；键已存在时原位覆盖（不触发淘汰），新键在容量已满时
    /// 按插入序 FIFO 淘汰最旧条目
    pub fn insert(&self, key: impl Into<Arc<str>>, value: V) {
        let key: Arc<str> = key.into();
        if let Err(e) = self.inner.set_sync(&key.to_string(), &value) {
            log::warn!(
                "{}",
                i18n::t(
                    "prepare-cache-log-write-failed",
                    &[("op", "insert".to_string()), ("error", e.to_string())]
                )
            );
        }
    }

    /// 清空全部条目并重置命中/未命中统计
    pub fn clear(&self) {
        if let Err(e) = self.inner.clear_sync() {
            log::warn!(
                "{}",
                i18n::t(
                    "prepare-cache-log-clear-failed",
                    &[("error", e.to_string())]
                )
            );
        }
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// (a) 顺序命中语义：同一语句的 prepare 只执行一次
    #[test]
    fn test_second_access_is_cache_hit() {
        let cache: PreparedStatementCache<String> = PreparedStatementCache::new(8);
        let prepares = AtomicU32::new(0);

        let (v1, hit1) = cache.get_or_prepare("SELECT 1", |sql| {
            prepares.fetch_add(1, Ordering::SeqCst);
            format!("prepared:{sql}")
        });
        assert!(!hit1, "the first access should miss");
        assert_eq!(&*v1, "prepared:SELECT 1");

        let (v2, hit2) = cache.get_or_prepare("SELECT 1", |sql| {
            prepares.fetch_add(1, Ordering::SeqCst);
            format!("prepared:{sql}")
        });
        assert!(hit2, "the second access should hit");
        assert_eq!(
            &*v2, "prepared:SELECT 1",
            "a hit should return the same prepared product"
        );
        assert_eq!(
            prepares.load(Ordering::SeqCst),
            1,
            "prepare should run only once"
        );

        // 不同语句 → 未命中
        let (_, hit3) = cache.get_or_prepare("SELECT 2", |_| String::new());
        assert!(!hit3);

        let stats = cache.stats();
        assert_eq!((stats.hits, stats.misses), (1, 2));
        assert_eq!(stats.size, 2);
    }

    /// (c) 插入语义：同键原位覆盖（不触发淘汰），统计只记命中/未命中口径
    #[test]
    fn test_insert_overwrites_in_place() {
        let cache: PreparedStatementCache<u32> = PreparedStatementCache::new(2);
        cache.insert("a", 1);
        cache.insert("a", 10);

        assert_eq!(
            cache.get("a").map(|v| *v),
            Some(10),
            "the same key should overwrite the previous value"
        );
        let stats = cache.stats();
        assert_eq!(stats.size, 1, "overwriting must not add an entry");
        assert_eq!(stats.evictions, 0, "overwriting must not trigger eviction");
        // 唯一一次命中来自上面的 get 探测；插入自身不计入命中/未命中
        assert_eq!(
            (stats.hits, stats.misses),
            (1, 0),
            "insert does not count as a hit or a miss"
        );
    }

    /// (d) 清空语义：条目与统计一并归零
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
        assert_eq!(cache.get("a"), None, "entries are unreachable after clear");
    }

    /// (b) 容量有界与统计账目一致：满容量稳态循环后 size 恒等于 capacity、
    /// 最新条目存活（moka TinyLFU 准入非严格 LRU，不锚逐 victim 精确序列）
    #[test]
    fn test_sustained_eviction_keeps_capacity_and_recency() {
        let cache: PreparedStatementCache<u32> = PreparedStatementCache::new(3);

        // 装满：a → b → c
        for (k, v) in [("a", 1), ("b", 2), ("c", 3)] {
            cache.insert(k, v);
        }
        assert_eq!(cache.stats().size, 3);

        // 稳态循环：每轮 1 条新键进、淘汰 1 条旧键
        for round in 0..6 {
            cache.insert(format!("n{round}"), round as u32);
            assert_eq!(
                cache.stats().size,
                3,
                "the capacity must not be exceeded after round {round}"
            );
        }

        // 性质：最新条目存活
        for k in ["n3", "n4", "n5"] {
            assert!(cache.get(k).is_some(), "{k} should survive");
        }

        // 统计口径：oxcache memory 后端不暴露淘汰计数——evictions 恒 0，
        // 淘汰行为由 size 容量界与命中率口径观测（见 PrepareCacheStats 字段 doc）
        let stats = cache.stats();
        assert_eq!(
            stats.size, 3,
            "the surviving set must not exceed the capacity"
        );
        assert_eq!(
            stats.evictions, 0,
            "the oxcache memory backend does not expose an eviction count, so evictions is always 0"
        );
        // 账目一致：3 次 get 探测命中 + 淘汰发生后被淘汰键的探测未命中
        assert_eq!(stats.hits, 3);
    }

    /// (b) 容量界与最近使用条目存活：逐 victim 精确序列断言已随
    /// TinyLFU 准入删除（design D2），改锚容量界 + 账目一致 + 存活性质
    #[test]
    fn test_lru_eviction_respects_capacity_and_recency() {
        let cache: PreparedStatementCache<()> = PreparedStatementCache::new(2);

        assert!(
            !cache.get_or_prepare("a", |_| ()).1,
            "the first access of a should miss"
        );
        assert!(
            !cache.get_or_prepare("b", |_| ()).1,
            "the first access of b should miss"
        );
        // 访问 a
        assert!(cache.get_or_prepare("a", |_| ()).1);
        // 插入 c → 容量满，发生淘汰（淘汰对象由 TinyLFU 准入决定，不锚定）
        assert!(!cache.get_or_prepare("c", |_| ()).1);

        // 性质：容量恒等
        let stats = cache.stats();
        assert_eq!(stats.size, 2);
        // 性质：账目中 evictions 恒 0（后端不暴露）
        assert_eq!(stats.evictions, 0);

        // 性质：最新插入的 c 存活；容量 2 下 a/b 中至多一个存活
        // （oxcache DashMapMemoryBackend 为插入序 FIFO 淘汰——访问不改变
        // 淘汰序，"最近使用条目存活"属于已删除的 LRU 性质， newest 存活）
        assert!(cache.get("c").is_some(), "c should still survive");
        let reachable = ["a", "b", "c"]
            .iter()
            .filter(|k| cache.get(k).is_some())
            .count();
        assert_eq!(
            reachable, 2,
            "with capacity 2 exactly 2 keys should be reachable, got {reachable}"
        );
    }

    /// (b) 探测语义：命中/未命中计入统计，不锚淘汰对象
    #[test]
    fn test_get_probes_and_counts_stats() {
        let cache: PreparedStatementCache<u32> = PreparedStatementCache::new(2);

        assert_eq!(
            cache.get("a"),
            None,
            "a probe on an empty cache should miss"
        );
        let stats = cache.stats();
        assert_eq!((stats.hits, stats.misses), (0, 1));

        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(
            cache.get("a").map(|v| *v),
            Some(1),
            "a hit probe should return the entry"
        );
        // 新键 c 插入：容量满，淘汰一例（对象不锚定）
        cache.insert("c", 3);

        // 性质：容量恒等
        assert_eq!(cache.stats().size, 2);
        // 性质：账目一致——1 次探测命中；evictions 恒 0
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.evictions, 0);
        // 性质：最新条目存活
        assert!(cache.get("c").is_some(), "c should still survive");
    }
}
