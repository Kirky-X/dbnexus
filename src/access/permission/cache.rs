// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in project root for full license information.

//! 权限缓存（TTL + SWR）
//!
//! 提供 `PermissionCache`：基于 `DashMap` 的线程安全权限策略缓存，支持：
//! - **TTL 过期**：每个条目在插入时记录时间戳，超过 TTL 后视为过期
//! - **后台刷新**：`get` 命中过期条目时返回 `None` 并 `tokio::spawn` 后台刷新
//! - **SWR（stale-while-revalidate）**：启用时返回旧值并后台刷新，避免缓存击穿
//!
//! 与 `PermissionContext`（基于 oxcache）的关系：
//! - `PermissionContext` 提供容量限制的缓存 + 速率限制 + 单飞行防护
//! - `PermissionCache` 提供显式 TTL + SWR 后台刷新，适合需要"过期但仍可用"语义的场景
//!
//! 两者互补共存，不强制替换。

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use super::provider::PermissionProvider;
use super::types::RolePolicy;

/// 后台刷新失败时的日志输出（避免引入 tracing feature 依赖）
///
/// 当 `tracing` feature 启用时可通过 `tracing::warn!` 替代此宏；
/// 当前用 `eprintln!` 保持 `permission` 模块的独立性。
macro_rules! warn_log {
    ($($arg:tt)*) => {
        eprintln!("[WARN] PermissionCache: {}", format!($($arg)*));
    };
}

/// 默认 TTL（5 分钟）
const DEFAULT_TTL: Duration = Duration::from_secs(300);
/// 默认最小刷新间隔（60 秒，防止刷新风暴）
const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

/// 缓存条目
#[derive(Clone, Debug)]
struct CacheEntry {
    /// 缓存的权限策略
    value: RolePolicy,
    /// 插入时间戳
    inserted_at: Instant,
}

/// 权限缓存配置（只读快照，构建后不可变）
#[derive(Clone, Debug)]
pub struct PermissionCacheConfig {
    /// 缓存 TTL（条目过期时间）
    pub ttl: Duration,
    /// 后台刷新最小间隔（防止同一 key 短时间内重复刷新）
    pub refresh_interval: Duration,
    /// 是否启用 SWR（stale-while-revalidate）模式
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
/// 基于 `DashMap` 实现，线程安全。可附加 `PermissionProvider` 用于后台刷新。
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
/// cache.insert("admin", RolePolicy::default());
/// assert!(cache.get("admin").is_some());
/// # }
/// ```
pub struct PermissionCache {
    /// 内部 DashMap 存储（用 Arc 共享，clone 是廉价的引用计数操作）
    inner: Arc<DashMap<String, CacheEntry>>,
    /// 配置
    config: PermissionCacheConfig,
    /// 权限提供者（可选，用于后台刷新）
    provider: Option<Arc<dyn PermissionProvider>>,
    /// 上次刷新时间（key -> Instant），用于 refresh_interval 节流
    last_refresh: Arc<DashMap<String, Instant>>,
}

impl std::fmt::Debug for PermissionCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermissionCache")
            .field("entry_count", &self.inner.len())
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
    /// 创建新的权限缓存（使用默认配置）
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            config: PermissionCacheConfig::default(),
            provider: None,
            last_refresh: Arc::new(DashMap::new()),
        }
    }

    /// 链式设置 TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.config.ttl = ttl;
        self
    }

    /// 链式设置后台刷新最小间隔
    pub fn with_refresh_interval(mut self, interval: Duration) -> Self {
        self.config.refresh_interval = interval;
        self
    }

    /// 链式启用/禁用 SWR（stale-while-revalidate）模式
    pub fn with_stale_while_revalidate(mut self, enabled: bool) -> Self {
        self.config.stale_while_revalidate = enabled;
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
    pub fn insert(&self, key: &str, value: RolePolicy) {
        let entry = CacheEntry {
            value,
            inserted_at: Instant::now(),
        };
        self.inner.insert(key.to_string(), entry);
    }

    /// 失效单个条目（删除）
    pub fn invalidate(&self, key: &str) {
        self.inner.remove(key);
        self.last_refresh.remove(key);
    }

    /// 清空所有缓存条目
    pub fn clear(&self) {
        self.inner.clear();
        self.last_refresh.clear();
    }

    /// 当前缓存条目数量
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// 缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// 查询缓存条目
    ///
    /// - **未过期**：返回 `Some(value)`
    /// - **已过期 + SWR 启用**：返回 `Some(旧值)` 并后台刷新
    /// - **已过期 + SWR 禁用**：返回 `None` 并后台刷新
    /// - **未命中**：返回 `None`（不触发刷新，调用方应主动 `insert`）
    pub fn get(&self, key: &str) -> Option<RolePolicy> {
        if let Some(entry) = self.inner.get(key) {
            let elapsed = entry.inserted_at.elapsed();
            if elapsed < self.config.ttl {
                // 未过期：直接返回
                return Some(entry.value.clone());
            }
            // 已过期
            self.maybe_spawn_refresh(key);
            if self.config.stale_while_revalidate {
                // SWR：返回旧值
                Some(entry.value.clone())
            } else {
                // 非 SWR：返回 None
                None
            }
        } else {
            None
        }
    }

    /// 检查条目是否过期（仅查时间戳，不触发刷新）
    pub fn is_expired(&self, key: &str) -> bool {
        if let Some(entry) = self.inner.get(key) {
            entry.inserted_at.elapsed() >= self.config.ttl
        } else {
            // 不存在的条目视为"过期"
            true
        }
    }

    /// 后台刷新条目（stale-while-revalidate）
    ///
    /// 使用 `refresh_interval` 节流，防止短时间内重复刷新。
    /// 刷新失败时保留旧值并记录 warn 日志。
    pub async fn refresh(&self, key: &str) {
        // 节流：检查上次刷新时间
        if let Some(last) = self.last_refresh.get(key) {
            if last.elapsed() < self.config.refresh_interval {
                return;
            }
        }
        self.last_refresh.insert(key.to_string(), Instant::now());

        let provider = match &self.provider {
            Some(p) => p.clone(),
            None => {
                warn_log!("refresh called without provider; key={}", key);
                return;
            }
        };

        let key_owned = key.to_string();
        // 同步调用 provider（trait 方法是同步的）
        match provider.get_role_policy(&key_owned) {
            Some(new_policy) => {
                self.insert(&key_owned, new_policy);
            }
            None => {
                // 角色不存在：保留旧值（stale-while-revalidate 语义）
                warn_log!("refresh returned None for role '{}'; keeping stale value", key_owned);
            }
        }
    }

    /// 触发后台刷新（如果配置了 provider 且未在节流窗口内）
    fn maybe_spawn_refresh(&self, key: &str) {
        if self.provider.is_none() {
            return;
        }
        // 节流检查
        if let Some(last) = self.last_refresh.get(key) {
            if last.elapsed() < self.config.refresh_interval {
                return;
            }
        }
        let key_owned = key.to_string();
        let provider = self.provider.clone().unwrap();
        let ttl = self.config.ttl;
        // Arc<DashMap> clone 是廉价的引用计数，spawn 任务写入会反映到原 cache
        let inner = self.inner.clone();
        let last_refresh = self.last_refresh.clone();

        tokio::spawn(async move {
            last_refresh.insert(key_owned.clone(), Instant::now());
            match provider.get_role_policy(&key_owned) {
                Some(new_policy) => {
                    let entry = CacheEntry {
                        value: new_policy,
                        inserted_at: Instant::now(),
                    };
                    inner.insert(key_owned, entry);
                }
                None => {
                    warn_log!(
                        "Background refresh returned None for role '{}'; keeping stale value (ttl={:?})",
                        key_owned,
                        ttl
                    );
                }
            }
        });
    }
}

impl Clone for PermissionCache {
    /// 克隆缓存（共享内部 DashMap）
    ///
    /// 内部 DashMap 用 `Arc` 包装，clone 是廉价的引用计数操作，
    /// 写入会反映到所有 clone 副本。
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            config: self.config.clone(),
            provider: self.provider.clone(),
            last_refresh: self.last_refresh.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::permission::types::{PermissionAction, TablePermission};

    fn sample_policy(table: &str) -> RolePolicy {
        RolePolicy {
            tables: vec![TablePermission {
                name: table.to_string(),
                operations: vec![PermissionAction::Select],
            }],
        }
    }

    #[tokio::test]
    async fn test_insert_and_get_fresh() {
        let cache = PermissionCache::new().with_ttl(Duration::from_secs(60));
        cache.insert("admin", sample_policy("users"));
        let got = cache.get("admin");
        assert!(got.is_some());
        assert_eq!(got.unwrap().tables.len(), 1);
    }

    #[tokio::test]
    async fn test_get_missing_returns_none() {
        let cache = PermissionCache::new();
        assert!(cache.get("ghost").is_none());
    }

    #[tokio::test]
    async fn test_expired_without_swr_returns_none() {
        let cache = PermissionCache::new()
            .with_ttl(Duration::from_millis(1))
            .with_stale_while_revalidate(false);
        cache.insert("admin", sample_policy("users"));
        // 等待过期
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(cache.get("admin").is_none());
    }

    #[tokio::test]
    async fn test_expired_with_swr_returns_stale() {
        let cache = PermissionCache::new()
            .with_ttl(Duration::from_millis(1))
            .with_stale_while_revalidate(true);
        cache.insert("admin", sample_policy("users"));
        tokio::time::sleep(Duration::from_millis(20)).await;
        let got = cache.get("admin");
        assert!(got.is_some(), "SWR should return stale value");
    }

    #[tokio::test]
    async fn test_is_expired() {
        let cache = PermissionCache::new().with_ttl(Duration::from_millis(10));
        cache.insert("admin", sample_policy("users"));
        assert!(!cache.is_expired("admin"));
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(cache.is_expired("admin"));
        assert!(cache.is_expired("ghost"));
    }

    #[tokio::test]
    async fn test_invalidate_and_clear() {
        let cache = PermissionCache::new();
        cache.insert("a", sample_policy("t1"));
        cache.insert("b", sample_policy("t2"));
        assert_eq!(cache.len(), 2);
        cache.invalidate("a");
        assert_eq!(cache.len(), 1);
        assert!(cache.get("a").is_none());
        cache.clear();
        assert!(cache.is_empty());
    }
}
