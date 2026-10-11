// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 权限缓存（TTL + SWR）
//!
//! 提供 `PermissionCache`：基于 oxcache `Cache`（stale + offload 特性）的
//! 线程安全权限策略缓存，支持：
//! - **TTL 过期**：SWR 双时间戳 envelope（expire_at/stale_at）判定三态
//!   （Fresh / Stale / Expired），物理 TTL = ttl + stale 窗口
//! - **后台刷新**：`get` 命中 stale 条目时返回旧值并后台刷新，同 key 并发
//!   至多一次回源（offload in-flight 去重 + 回源准入闸）
//! - **SWR（stale-while-revalidate）**：由 `stale_policy(OffloadRevalidate)`
//!   承载；禁用时 stale 窗口为 0，过期条目立即视同 miss
//!
//! # 迁移说明（原 DashMap 手写实现 → oxcache 承载）
//!
//! - 原 `len`/`is_empty`/`is_expired` 无 oxcache 直接等价且无生产消费方，
//!   已移除；条目存在性经 `get` 返回值判定。
//! - 读写面（`get`/`insert`/`invalidate`/`clear`/`refresh`）为异步方法
//!   （oxcache 异步面承载；`sync_mode` 与 stale 特性互斥）。
//! - `with_refresh_interval` 映射为 stale 窗口长度（`stale_ttl`）：原
//!   "最小刷新间隔"时间窗节流由 stale 窗口 + 在飞去重承担。
//! - miss 路径新增 provider 回填能力（原实现 miss 不回源，要求调用方手动
//!   `insert`）；provider 返回 None 时任何路径不写缓存、保留旧值。
//! - 与 `PermissionContext` 的关系：两者均基于 oxcache；`PermissionContext`
//!   面向容量限制的上下文缓存 + 速率限制，本类型面向显式 TTL + SWR 语义，
//!   按场景选用。
//!
//! # 约束
//!
//! builder 链（`with_*`）须在首次读写前完成：配置变更会以新配置重建底层
//! oxcache 实例，此前写入的条目不保留。

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use oxcache::features::stale::StalePolicy;
use oxcache::{Cache, OxCacheError};
use tokio::sync::Mutex as AsyncMutex;

use crate::i18n;

use super::provider::PermissionProvider;
use super::types::RolePolicy;

/// 默认 TTL（5 分钟）
const DEFAULT_TTL: Duration = Duration::from_secs(300);
/// 默认最小刷新间隔（60 秒，防止刷新风暴）
///
/// oxcache 承载后该值映射为 stale 窗口长度（`stale_ttl`）：窗口内过期条目
/// 仍返回旧值并触发至多一次后台刷新，窗口外条目视同 miss。
const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
/// oxcache memory 后端容量上限
///
/// 迁移前 DashMap 无容量界；moka 后端要求显式容量，取实际不可达的上界，
/// 使容量淘汰在本缓存语义下不触发（与迁移前一致）。
const CACHE_CAPACITY: u64 = u64::MAX;

/// 权限缓存配置（只读快照，构建后不可变）
#[derive(Clone, Debug)]
pub struct PermissionCacheConfig {
    /// 缓存 TTL（条目过期时间）
    pub ttl: Duration,
    /// 后台刷新最小间隔（防止同一 key 短时间内重复刷新）
    ///
    /// oxcache 承载后映射为 stale 窗口长度（`stale_ttl`）。
    pub refresh_interval: Duration,
    /// 是否启用 SWR（stale-while-revalidate）模式
    ///
    /// 禁用时 stale 窗口为 0：过期条目立即视同 miss。
    pub stale_while_revalidate: bool,
}

impl Default for PermissionCacheConfig {
    fn default() -> Self {
        Self {
            ttl: DEFAULT_TTL,
            refresh_interval: DEFAULT_REFRESH_INTERVAL,
            stale_while_revalidate: true,
        }
    }
}

/// 权限缓存（TTL + SWR）
///
/// 由 oxcache `Cache`（stale + offload 特性）承载：过期行为由 SWR 双时间戳
/// envelope 判定（Fresh / Stale / Expired 三态），后台刷新去重由 offload
/// in-flight 同 key 去重承担。线程安全，可附加 `PermissionProvider` 用于
/// 后台刷新。
///
/// # 示例
///
/// ```ignore
/// use std::sync::Arc;
/// use std::time::Duration;
/// use dbnexus::permission::{PermissionCache, PermissionProvider, YamlPermissionProvider};
///
/// # async fn example() {
/// let provider: Arc<dyn PermissionProvider> = Arc::new(YamlPermissionProvider::new());
/// let cache = PermissionCache::new()
///     .with_ttl(Duration::from_secs(60))
///     .with_refresh_interval(Duration::from_secs(10))
///     .with_stale_while_revalidate(true)
///     .with_provider(provider);
///
/// cache.insert("admin", RolePolicy::default()).await;
/// assert!(cache.get("admin").await.is_some());
/// # }
/// ```
pub struct PermissionCache {
    /// oxcache 实例（Arc 共享，clone 是廉价的引用计数操作）
    inner: Arc<Cache<String, RolePolicy>>,
    /// 配置
    config: PermissionCacheConfig,
    /// 权限提供者（可选，用于后台刷新）
    provider: Option<Arc<dyn PermissionProvider>>,
    /// 同 key 回源准入闸（key -> 锁）
    ///
    /// oxcache offload 的在飞去重为 `contains_key` → `insert` 两步非原子
    /// 操作，屏障锁步下的并发调用会同时通过检查（实测 16 锁步并发 4-8 次
    /// 穿透，oxcache 0.5.0-rc.6 无锁步覆盖）。本闸以非阻塞 `try_lock`
    /// 把同 key 的 `get_or_refresh` 调用串到在飞去重之后：持锁者走完整
    /// 刷新路径，未持锁者只读当前值不触发刷新（与"他人已在回源"语义等价）。
    /// 无时间窗、不阻塞，生命周期随 `invalidate`/`clear` 清理。
    /// `Arc` 包装使 `clone` 的各副本共享同一张准入表（DashMap 自身 Clone
    /// 为深拷贝，直接持有会让克隆副本各持独立闸门而失效）。
    revalidate_gate: Arc<DashMap<String, Arc<AsyncMutex<()>>>>,
}

impl std::fmt::Debug for PermissionCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermissionCache")
            .field("config", &self.config)
            .field("has_provider", &self.provider.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for PermissionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionCache {
    /// 按当前配置构建 oxcache 实例
    ///
    /// - `ttl` / `stale_ttl`：SWR 双时间戳 envelope（物理 TTL = ttl + stale_ttl）；
    /// - `stale_policy(OffloadRevalidate)`：stale 命中返回旧值并后台刷新，
    ///   builder 自动挂载 OffloadManager（容量上限显式配置为 8，不依赖默认值）。
    fn build_inner(config: &PermissionCacheConfig) -> Cache<String, RolePolicy> {
        let stale_window = if config.stale_while_revalidate {
            config.refresh_interval
        } else {
            Duration::ZERO
        };
        Cache::<String, RolePolicy>::builder()
            .capacity(CACHE_CAPACITY)
            .ttl(config.ttl)
            .stale_ttl(stale_window)
            .stale_policy(StalePolicy::OffloadRevalidate)
            .build_sync()
            .expect("oxcache default memory backend build must not fail (no user backend can fail)")
    }

    /// 创建新的权限缓存（使用默认配置）
    pub fn new() -> Self {
        let config = PermissionCacheConfig::default();
        Self {
            inner: Arc::new(Self::build_inner(&config)),
            config,
            provider: None,
            revalidate_gate: Arc::new(DashMap::new()),
        }
    }

    /// 链式设置 TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.config.ttl = ttl;
        self.inner = Arc::new(Self::build_inner(&self.config));
        self
    }

    /// 链式设置后台刷新最小间隔（映射为 oxcache stale 窗口长度）
    pub fn with_refresh_interval(mut self, interval: Duration) -> Self {
        self.config.refresh_interval = interval;
        self.inner = Arc::new(Self::build_inner(&self.config));
        self
    }

    /// 链式启用/禁用 SWR（stale-while-revalidate）模式
    pub fn with_stale_while_revalidate(mut self, enabled: bool) -> Self {
        self.config.stale_while_revalidate = enabled;
        self.inner = Arc::new(Self::build_inner(&self.config));
        self
    }

    /// 链式附加权限提供者（用于后台刷新）
    pub fn with_provider(mut self, provider: Arc<dyn PermissionProvider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// 获取当前配置快照
    pub fn config(&self) -> &PermissionCacheConfig {
        &self.config
    }

    /// 插入或更新缓存条目（重置时间戳）
    pub async fn insert(&self, key: &str, value: RolePolicy) {
        if let Err(e) = self
            .inner
            .set_with_ttl(&key.to_string(), &value, Some(self.config.ttl))
            .await
        {
            log::warn!(
                "{}",
                i18n::t("perm-cache-log-insert-failed", &[("error", e.to_string())])
            );
        }
    }

    /// 失效单个条目（删除）
    pub async fn invalidate(&self, key: &str) {
        self.revalidate_gate.remove(key);
        if let Err(e) = self.inner.delete(&key.to_string()).await {
            log::warn!(
                "{}",
                i18n::t(
                    "perm-cache-log-invalidate-failed",
                    &[("error", e.to_string())]
                )
            );
        }
    }

    /// 清空所有缓存条目
    pub async fn clear(&self) {
        self.revalidate_gate.clear();
        if let Err(e) = self.inner.clear().await {
            log::warn!(
                "{}",
                i18n::t("perm-cache-log-clear-failed", &[("error", e.to_string())])
            );
        }
    }

    /// 查询缓存条目
    ///
    /// - **未过期**：返回 `Some(当前值)`（Fresh）
    /// - **已过期 + stale 窗口内**：返回 `Some(旧值)` 并触发后台刷新（Stale，
    ///   同 key 并发至多一次回源）
    /// - **超过 stale 窗口**：条目物理删除，视同 miss——经 provider 回源，
    ///   provider 返回 None 或未配置时返回 `None`
    /// - **未命中**：同上，经 provider 回源；无可用策略时返回 `None`
    ///
    /// 并发语义：回源准入闸（见 `revalidate_gate`）持锁者走完整刷新路径；
    /// 未持锁者只读当前值——同 key 并发刷新去重由 offload 在飞 + 准入闸
    /// 共同保证（屏障锁步下 oxcache 在飞去热的两步检查会穿透）。
    pub async fn get(&self, key: &str) -> Option<RolePolicy> {
        let key_owned = key.to_string();
        let gate = self
            .revalidate_gate
            .entry(key_owned.clone())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone();
        if let Ok(_admitted) = gate.try_lock() {
            // 持锁：走完整 get_or_refresh（stale 命中 → offload 后台刷新）；
            // fallback 仅在持锁路径构造（未持锁者只读，省一次 Arc/String 克隆）
            let provider = self.provider.clone();
            let key_for_fallback = key_owned.clone();
            let fallback = move || {
                let provider = provider.clone();
                let key_owned = key_for_fallback.clone();
                async move {
                    match provider {
                        // provider 返回 None（角色缺失）映射为 Err：任何路径不写
                        // 缓存，旧值保留（负缓存属 Non-Goals）
                        Some(p) => p.get_role_policy(&key_owned).map(Ok).unwrap_or_else(|| {
                            Err(OxCacheError::NotFound(i18n::t(
                                "perm-cache-provider-no-policy",
                                &[("role", key_owned.clone())],
                            )))
                        }),
                        None => Err(OxCacheError::NotFound(i18n::t(
                            "perm-cache-provider-not-configured",
                            &[("role", key_owned.clone())],
                        ))),
                    }
                }
            };
            self.inner
                .get_or_refresh(&key_owned, Some(self.config.ttl), fallback)
                .await
                .ok()
        } else {
            // 未持锁：同 key 已有调用方在回源，只读当前值不触发刷新
            // （stale 窗口内返回旧值，与 SWR 语义一致）
            self.inner.get(&key_owned).await.ok().flatten()
        }
    }

    /// 后台刷新条目（stale-while-revalidate）
    ///
    /// 语义由 oxcache 三态承载：Fresh 条目不重复回源（时间维度由 stale
    /// 窗口承担）；Stale 条目经 offload in-flight 去重后台刷新（并发调用
    /// 至多一次回源）；Expired/缺失条目经 single-flight 同步回源。provider
    /// 返回 None 时任何路径不写缓存（保留旧值）。
    pub async fn refresh(&self, key: &str) {
        let _ = self.get(key).await;
    }
}

impl Clone for PermissionCache {
    /// 克隆缓存（共享内部 oxcache 实例）
    ///
    /// 内部 `Cache` 用 `Arc` 包装，clone 是廉价的引用计数操作，
    /// 写入会反映到所有 clone 副本。
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            config: self.config.clone(),
            provider: self.provider.clone(),
            revalidate_gate: self.revalidate_gate.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::access::permission::PermissionProviderError;
    use crate::access::{PermissionAction, TablePermission};

    fn sample_policy(table: &str) -> RolePolicy {
        RolePolicy {
            tables: vec![TablePermission {
                name: table.to_string(),
                operations: vec![PermissionAction::Select],
            }],
        }
    }

    /// 计数 provider：统计 get_role_policy 调用次数，并阻塞一段时间
    /// 放大并发窗口（模拟慢 provider）
    struct CountingProvider {
        calls: AtomicUsize,
        /// provider 内部阻塞时长（放大竞态窗口 / 模拟慢源）
        delay: Duration,
    }

    impl CountingProvider {
        fn new() -> Self {
            Self::with_delay(Duration::from_millis(50))
        }

        fn with_delay(delay: Duration) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                delay,
            }
        }
    }

    impl PermissionProvider for CountingProvider {
        fn get_role_policy(&self, _role: &str) -> Option<RolePolicy> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(self.delay);
            Some(sample_policy("users"))
        }

        fn check_access(
            &self,
            role: &str,
            table: &str,
            operation: PermissionAction,
        ) -> Result<bool, PermissionProviderError> {
            Ok(self.get_role_policy(role).is_some_and(|p| {
                p.tables
                    .iter()
                    .any(|t| t.name == table && t.operations.contains(&operation))
            }))
        }

        fn get_roles(&self) -> Vec<String> {
            vec!["admin".to_string()]
        }
    }

    /// TTL 内已插入条目：get 返回当前值（Fresh）
    #[tokio::test]
    async fn test_fresh_get_returns_value() {
        let cache = PermissionCache::new().with_ttl(Duration::from_secs(60));
        cache.insert("admin", sample_policy("users")).await;
        let got = cache.get("admin").await;
        assert!(got.is_some(), "fresh entry should hit");
        assert_eq!(got.unwrap().tables.len(), 1);
    }

    /// 未命中且无 provider：get 返回 None（不触发刷新）
    #[tokio::test]
    async fn test_get_missing_returns_none() {
        let cache = PermissionCache::new();
        assert!(cache.get("ghost").await.is_none());
    }

    /// (a) 过期条目在 stale 窗口内：get 返回旧值（SWR）
    ///
    /// stale 窗口由 `with_refresh_interval` 映射承载（见 R-permission-cache-004）。
    #[tokio::test]
    async fn test_expired_within_stale_window_returns_stale_value() {
        let cache = PermissionCache::new()
            .with_ttl(Duration::from_millis(30))
            .with_refresh_interval(Duration::from_secs(5));
        cache.insert("admin", sample_policy("users")).await;
        // 超过 TTL（30ms）但仍在 stale 窗口（5s）内
        tokio::time::sleep(Duration::from_millis(80)).await;
        let got = cache.get("admin").await;
        assert!(
            got.is_some(),
            "the stale value should be returned within the stale window"
        );
        assert_eq!(got.unwrap().tables[0].name, "users");
    }

    /// (b) 超过 stale 窗口：get 返回 None（Expired，视同 miss）
    #[tokio::test]
    async fn test_beyond_stale_window_returns_none() {
        let cache = PermissionCache::new()
            .with_ttl(Duration::from_millis(10))
            .with_refresh_interval(Duration::from_millis(30));
        cache.insert("admin", sample_policy("users")).await;
        // 物理窗口 = TTL 10ms + stale 30ms ≈ 40ms；等 120ms 后彻底过期
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(
            cache.get("admin").await.is_none(),
            "should be a miss once past the stale window"
        );
    }

    /// (c) 并发 get 过期 key：provider 恰好被调用一次
    ///（offload in-flight 同 key 去重承载，替代手写 try_reserve_refresh 时间窗节流）
    // TTL 50ms + burst 前等 60ms：条目进入 stale 窗口触发回源；回源写回后
    // 条目有 50ms 新鲜窗，覆盖被阻塞 provider 拖住的迟到竞速者（避免其把
    // "刚刷新又转 stale"的条目再次回源——那是 SWR 语义而非去重失效）。
    // CountingProvider 以 std::thread::sleep 阻塞 worker，worker_threads
    // 取任务数 + 2 防止 burst 任务排队。
    #[tokio::test(flavor = "multi_thread", worker_threads = 10)]
    async fn test_concurrent_get_single_background_refresh() {
        let provider = Arc::new(CountingProvider::new());
        let cache = PermissionCache::new()
            .with_ttl(Duration::from_millis(50))
            .with_refresh_interval(Duration::from_secs(60))
            .with_provider(provider.clone());
        cache.insert("admin", sample_policy("users")).await;
        tokio::time::sleep(Duration::from_millis(60)).await;

        let mut handles = Vec::new();
        for _ in 0..8 {
            let cache = cache.clone();
            handles.push(tokio::spawn(async move {
                let _ = cache.get("admin").await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        wait_for_provider_calls(&provider, &cache, "admin", 1).await;
    }

    /// (d) provider 返回 None（角色缺失）时旧值保留且缓存不写入
    #[tokio::test]
    async fn test_refresh_keeps_old_value_when_role_missing() {
        struct NoPolicyProvider;

        impl PermissionProvider for NoPolicyProvider {
            fn get_role_policy(&self, _role: &str) -> Option<RolePolicy> {
                None
            }

            fn check_access(
                &self,
                _role: &str,
                _table: &str,
                _operation: PermissionAction,
            ) -> Result<bool, PermissionProviderError> {
                Ok(false)
            }

            fn get_roles(&self) -> Vec<String> {
                Vec::new()
            }
        }

        let cache = PermissionCache::new()
            .with_ttl(Duration::from_millis(1))
            .with_refresh_interval(Duration::from_secs(60))
            .with_provider(Arc::new(NoPolicyProvider));
        cache.insert("ghost", sample_policy("users")).await;
        tokio::time::sleep(Duration::from_millis(20)).await;

        // provider 返回 None：fallback 映射为 Err，后台任务不写缓存
        cache.refresh("ghost").await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            cache.get("ghost").await.is_some(),
            "refresh must keep the stale value when the role is missing"
        );
    }

    /// (e) invalidate 后条目不可达；clear 清空全部
    #[tokio::test]
    async fn test_invalidate_and_clear_hide_entries() {
        let cache = PermissionCache::new().with_ttl(Duration::from_secs(60));
        cache.insert("a", sample_policy("t1")).await;
        cache.insert("b", sample_policy("t2")).await;
        cache.invalidate("a").await;
        assert!(
            cache.get("a").await.is_none(),
            "unreachable after invalidate"
        );
        assert!(cache.get("b").await.is_some());
        cache.clear().await;
        assert!(cache.get("b").await.is_none(), "unreachable after clear");
    }

    /// Debug 形态：配置快照 + provider 存在性
    #[tokio::test]
    async fn test_debug_shape() {
        let cache = PermissionCache::new().with_provider(Arc::new(CountingProvider::new()));
        let debug = format!("{cache:?}");
        assert!(debug.contains("PermissionCache"), "got: {debug}");
        assert!(debug.contains("has_provider: true"), "got: {debug}");
        assert!(
            debug.contains("stale_while_revalidate: true"),
            "got: {debug}"
        );
    }

    /// 并发 refresh 过期 key：provider 恰好被调用一次
    ///（R-permission-cache-002：in-flight 去重同时覆盖显式 refresh 路径）
    #[tokio::test(flavor = "multi_thread", worker_threads = 18)]
    async fn test_concurrent_refresh_single_provider_call() {
        let provider = Arc::new(CountingProvider::with_delay(Duration::from_millis(20)));
        let cache = PermissionCache::new()
            .with_ttl(Duration::from_millis(50))
            .with_refresh_interval(Duration::from_secs(60))
            .with_provider(provider.clone());
        cache.insert("admin", sample_policy("users")).await;
        tokio::time::sleep(Duration::from_millis(60)).await;

        let mut handles = Vec::new();
        for _ in 0..16 {
            let cache = cache.clone();
            handles.push(tokio::spawn(async move {
                cache.refresh("admin").await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        wait_for_provider_calls(&provider, &cache, "admin", 1).await;
    }

    // =========================================================================
    // 强同步并发回归测试（Barrier 多轮竞速版）
    //
    // oxcache 承载下刷新去重由 offload in-flight 表承担：每轮 16 个任务先在
    // Barrier 汇合，再经原子倒计数（race_start）近似同时起跑，同时触发
    // refresh / get 两条路径——同 key 的在飞去重必须保证每轮 provider
    // 恰好被调用 1 次。刷新成功回写后条目回到 Fresh，stale 窗口内不重复
    // 回源（时间维度由 stale 窗口承担）。
    // =========================================================================

    /// 竞速任务数（与 worker_threads 相同，保证放行时全员并行）
    const RACE_TASKS: usize = 16;
    /// 独立竞速轮数（单轮命中是概率事件，多轮累积保证稳定检出）
    const RACE_ROUNDS: usize = 24;

    /// 竞速起跑线：Barrier 负责全员汇合，但 futex 唤醒有微秒级抖动，
    /// 不足以命中纳秒级竞态窗口。汇合后各任务对原子倒计数做 fetch_sub
    /// 并等待归零——最后一个减到 0 的任务立即起跑，其余任务在一个缓存行
    /// 传播周期内观察到 0 并同时起跑，实现近似锁步放行。
    ///
    /// 等待采用"短自旋 + yield"混合：纯自旋会阻塞 worker 线程，若同一线程
    /// 的任务队列中还有未 poll 的竞速者，会使其永远无法运行（倒计数无法
    /// 归零）导致死锁；短自旋保证计数临近归零时纳秒级起跑，兜底 yield
    /// 保证饿死不可能发生。
    async fn race_start(barrier: &tokio::sync::Barrier, latch: &AtomicUsize) {
        barrier.wait().await;
        latch.fetch_sub(1, Ordering::AcqRel);
        while latch.load(Ordering::Acquire) != 0 {
            for _ in 0..4096 {
                std::hint::spin_loop();
                if latch.load(Ordering::Acquire) == 0 {
                    return;
                }
            }
            tokio::task::yield_now().await;
        }
    }

    /// 等待 provider 累计调用次数达到 `expected`
    ///
    /// 后台回源是 best-effort：oxcache offload 在同 key 已在飞**或并发许可
    /// 耗尽**时直接丢弃 spawn（fire-and-forget 不排队，被丢弃的一轮不再回源，
    /// 只能由下一次请求触发）。因此固定 sleep 后断言精确次数在高负载下会
    /// 假失败——改用轮询等待，未达期望时以 `cache.refresh(key)` 按契约补触发
    /// （fresh 条目不回源、同 key 在飞被去重吸收，均不产生额外调用）；
    /// 轮询期间一旦发现调用数超过 `expected`，即判定去重失效（本组测试
    /// 要抓的回归）。
    async fn wait_for_provider_calls(
        provider: &CountingProvider,
        cache: &PermissionCache,
        key: &str,
        expected: usize,
    ) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let calls = provider.calls.load(Ordering::SeqCst);
            assert!(
                calls <= expected,
                "provider must be called at most {expected} time(s) for {key}, got {calls} (dedup broken)"
            );
            if calls == expected {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "provider was never called {expected} time(s) for {key}: calls = {calls}"
            );
            // offload 丢弃后的再次触发（同 key 在飞时被在飞去重吸收，不额外回源）
            cache.refresh(key).await;
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// 强同步 refresh/get 混合路径：每轮锁步放行后两条路径同时触发，
    /// 在飞去重下每轮 provider 仍只应被调用 1 次
    #[tokio::test(flavor = "multi_thread", worker_threads = 18)]
    async fn test_barrier_mixed_refresh_and_get_single_provider_call() {
        // 每轮独立 key：竞速窗口天然隔离——上一轮在飞任务（同 key 才会
        // 被在飞去重吸收）不会穿透到本轮，条目写入亦互不可见。
        // TTL 50ms + burst 前等 60ms：条目进入 stale 窗口；回源写回后
        // 50ms 新鲜窗覆盖被阻塞 provider 拖住的迟到 burst（否则迟到者把
        // "刚刷新又转 stale"的条目再次回源，属 SWR 语义而非去重失效）。
        let provider = Arc::new(CountingProvider::with_delay(Duration::from_millis(20)));
        let cache = PermissionCache::new()
            .with_ttl(Duration::from_millis(50))
            .with_refresh_interval(Duration::from_secs(60))
            .with_provider(provider.clone());

        let barrier = Arc::new(tokio::sync::Barrier::new(RACE_TASKS));
        for round in 0..RACE_ROUNDS {
            let key = format!("admin_{round}");
            cache.insert(&key, sample_policy("users")).await;
            // 等待条目过期（ttl = 50ms），进入 stale 窗口
            tokio::time::sleep(Duration::from_millis(60)).await;

            let latch = Arc::new(AtomicUsize::new(RACE_TASKS));
            let mut handles = Vec::with_capacity(RACE_TASKS);
            for i in 0..RACE_TASKS {
                let cache = cache.clone();
                let barrier = barrier.clone();
                let latch = latch.clone();
                let key = key.clone();
                handles.push(tokio::spawn(async move {
                    race_start(&barrier, &latch).await;
                    if i % 2 == 0 {
                        // 偶数任务走显式 refresh 路径
                        cache.refresh(&key).await;
                    } else {
                        // 奇数任务走 get 过期 key → offload 后台刷新路径
                        let _ = cache.get(&key).await;
                    }
                }));
            }
            for h in handles {
                h.await.unwrap();
            }

            // 每轮竞速后等该轮回源落地再进入下一轮：计数逐轮推进，
            // 轮与轮之间不会互相借调计数。去重保证每轮至多 1 次回源。
            wait_for_provider_calls(&provider, &cache, &key, round + 1).await;
        }

        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            RACE_ROUNDS,
            "concurrent refresh and get paths must still allow only 1 provider call per round"
        );
    }

    /// 计数 provider 的权限判定契约与缓存默认构造
    #[tokio::test]
    async fn test_counting_provider_contract_and_default_cache() {
        let provider = CountingProvider::with_delay(Duration::ZERO);

        assert!(
            provider
                .check_access("admin", "users", PermissionAction::Select)
                .expect("check_access"),
            "the sample policy grants select on users"
        );
        assert!(
            !provider
                .check_access("admin", "users", PermissionAction::Update)
                .expect("check_access"),
            "the sample policy does not grant update"
        );
        assert_eq!(
            provider.get_roles(),
            vec!["admin".to_string()],
            "the counting provider advertises the admin role"
        );

        // Default 与 new 等价：空缓存未命中返回 None
        let cache = PermissionCache::default();
        assert!(cache.get("missing").await.is_none());
    }
}
