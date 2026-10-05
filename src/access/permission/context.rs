// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 权限上下文
//!
//! 提供权限检查的上下文环境。

use super::provider::PermissionProvider;
use super::rate_limiter::RateLimiter;
use super::stats::{CacheStats, PermissionCheckStats};
use super::types::{PermissionAction, PermissionConfig, PermissionError, RolePolicy};
use dashmap::DashMap;
use dbnexus_limiter_port::Limiter;
#[cfg(feature = "cache")]
use oxcache::Cache;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;

#[cfg(feature = "audit")]
use crate::domain::audit::{AuditEvent, AuditLogger, AuditOperation, AuditSeverity, AuditStatus};
use crate::i18n;

/// 权限检查速率限制默认值
const DEFAULT_RATE_LIMIT_MAX_REQUESTS: u32 = 100;
const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// 默认权限策略缓存容量
///
/// 此值作为后备默认值使用，实际应从 `CacheConfig.policy_cache_capacity` 获取。
const DEFAULT_POLICY_CACHE_CAPACITY: usize = 4096;

/// 构造内置令牌桶限流器（默认后端，实现 `Limiter` 端口）
fn token_bucket_limiter(max_requests: u32, window_secs: u64) -> Arc<dyn Limiter> {
    Arc::new(RateLimiter::new(
        max_requests,
        Duration::from_secs(window_secs),
        10000,
        max_requests,
    ))
}

/// 速率限制后端选择（双后端）
///
/// 旧令牌桶保留为默认后端；外部限流引擎（如 limiteron 适配器）实现
/// `Limiter` 端口后经 [`RateLimitBackend::External`] 注入——dbnexus 与
/// limiteron 互为消费方（Cargo 禁包级循环依赖），故限流端口独立于两者，
/// 装配发生在应用组合根。
#[derive(Clone)]
pub enum RateLimitBackend {
    /// 内置令牌桶（默认后端）
    TokenBucket {
        /// 时间窗口内最大请求数
        max_requests: u32,
        /// 时间窗口大小（秒）
        window_secs: u64,
    },
    /// 外部 `Limiter` 端口实现
    External(Arc<dyn Limiter>),
}

/// 表访问检查决策
///
/// 区分策略拒绝（403 语义）与限流拒绝（429 语义），后者携带
/// `Retry-After` 建议；布尔简版见
/// [`check_table_access`](PermissionContext::check_table_access)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableAccessDecision {
    /// 允许访问
    Allowed,
    /// 策略拒绝（权限不足）
    Denied,
    /// 被速率限制拒绝（HTTP 429 语义）
    RateLimited {
        /// 建议等待时间；`None` = 后端未提供（含后端故障 fail-closed）
        retry_after: Option<Duration>,
    },
}

/// 从同步上下文安全创建缓存
///
/// 处理两种运行场景：
/// - 在 tokio 运行时内（async 上下文）：使用 `block_in_place` + `Handle::block_on`
/// - 在 tokio 运行时外：创建临时运行时执行异步构建
///
/// # Panics
///
/// 若无 tokio 运行时且无法创建临时运行时，则 panic。
fn create_cache_sync(capacity: usize) -> Cache<String, RolePolicy> {
    let build_future = async { Cache::builder().capacity(capacity as u64).build().await };

    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // 在 tokio 运行时内，使用 block_in_place 避免 async 上下文死锁
            tokio::task::block_in_place(|| handle.block_on(build_future))
                .expect("Failed to create cache")
        }
        Err(_) => {
            // 无运行时，创建临时运行时
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(build_future).expect("Failed to create cache")
        }
    }
}

/// 权限上下文
///
/// 注意：此结构体需要启用 `cache` feature 才能使用。
#[cfg(feature = "cache")]
#[derive(Clone)]
pub struct PermissionContext {
    /// 角色名称
    role: String,

    /// 权限策略缓存（使用 oxcache，线程安全）
    policy_cache: Arc<Cache<String, RolePolicy>>,

    /// 缓存容量（用于统计信息）
    cache_capacity: usize,

    /// 权限检查速率限制器（`Limiter` 端口；默认令牌桶，可注入外部实现）
    rate_limiter: Option<Arc<dyn Limiter>>,

    /// 限流拒绝审计器（audit feature；未挂载时不产生审计事件）
    #[cfg(feature = "audit")]
    audit_logger: Option<Arc<AuditLogger>>,

    /// 权限检查统计
    check_stats: Arc<PermissionCheckStats>,

    /// 权限提供者（用于缓存未命中时重新加载策略）
    permission_provider: Option<Arc<dyn PermissionProvider>>,

    /// 请求合并 in_flight map（防止缓存击穿）
    in_flight: DashMap<String, Arc<InFlightEntry>>,
}

/// 单flight entry：保存加载结果和通知器
struct InFlightEntry {
    /// 加载结果（使用 tokio Mutex 保护，lock().await 会 park follower）
    result: TokioMutex<Option<RolePolicy>>,
    /// leader 是否已完成（用于 follower 检测）
    done: Arc<AtomicBool>,
}

#[cfg(feature = "cache")]
impl std::fmt::Debug for PermissionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermissionContext")
            .field("role", &self.role)
            .field("rate_limiter", &self.rate_limiter.is_some())
            .field(
                "has_permission_provider",
                &self.permission_provider.is_some(),
            )
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "cache")]
impl PermissionContext {
    /// 创建新的权限上下文（使用默认缓存大小）
    ///
    /// # Errors
    ///
    /// 如果默认缓存大小无效，返回错误
    pub async fn new_default() -> Result<Self, PermissionError> {
        Self::with_cache_size("admin".to_string(), DEFAULT_POLICY_CACHE_CAPACITY).await
    }

    /// 创建新的权限上下文（使用默认缓存大小和速率限制）
    pub async fn new_default_with_rate_limit(role: String) -> Result<Self, PermissionError> {
        Self::with_cache_size_and_rate_limit(
            role,
            DEFAULT_POLICY_CACHE_CAPACITY,
            DEFAULT_RATE_LIMIT_MAX_REQUESTS,
            DEFAULT_RATE_LIMIT_WINDOW_SECS,
        )
        .await
    }

    /// 创建新的权限上下文（使用自定义缓存大小）
    ///
    /// # Errors
    ///
    /// 如果 `cache_capacity` 为 0，返回 `InvalidCacheCapacity` 错误
    pub async fn with_cache_size(
        role: String,
        cache_capacity: usize,
    ) -> Result<Self, PermissionError> {
        let policy_cache = Cache::builder()
            .capacity(cache_capacity as u64)
            .build()
            .await
            .map_err(|_| PermissionError::InvalidCacheCapacity)?;
        Ok(Self {
            role,
            policy_cache: Arc::new(policy_cache),
            cache_capacity,
            rate_limiter: Some(token_bucket_limiter(
                DEFAULT_RATE_LIMIT_MAX_REQUESTS,
                DEFAULT_RATE_LIMIT_WINDOW_SECS,
            )),
            #[cfg(feature = "audit")]
            audit_logger: None,
            check_stats: Arc::new(PermissionCheckStats::new()),
            permission_provider: None,
            in_flight: DashMap::new(),
        })
    }

    /// 创建新的权限上下文（使用自定义缓存大小和速率限制）
    ///
    /// # Errors
    ///
    /// 如果 `cache_capacity` 为 0，返回 `InvalidCacheCapacity` 错误
    pub async fn with_cache_size_and_rate_limit(
        role: String,
        cache_capacity: usize,
        max_requests: u32,
        window_secs: u64,
    ) -> Result<Self, PermissionError> {
        let policy_cache = Cache::builder()
            .capacity(cache_capacity as u64)
            .build()
            .await
            .map_err(|_| PermissionError::InvalidCacheCapacity)?;
        Ok(Self {
            role,
            policy_cache: Arc::new(policy_cache),
            cache_capacity,
            rate_limiter: Some(token_bucket_limiter(max_requests, window_secs)),
            #[cfg(feature = "audit")]
            audit_logger: None,
            check_stats: Arc::new(PermissionCheckStats::new()),
            permission_provider: None,
            in_flight: DashMap::new(),
        })
    }

    /// 创建新的权限上下文（自定义缓存容量 + 限流后端选择）
    ///
    /// 双后端切换入口：[`RateLimitBackend::TokenBucket`] 为默认令牌桶；
    /// [`RateLimitBackend::External`] 注入外部 `Limiter` 端口实现
    /// （limiteron 适配器等），装配由应用组合根完成。
    ///
    /// # Errors
    ///
    /// 如果 `cache_capacity` 为 0，返回 `InvalidCacheCapacity` 错误
    pub async fn with_cache_size_and_backend(
        role: String,
        cache_capacity: usize,
        backend: RateLimitBackend,
    ) -> Result<Self, PermissionError> {
        let policy_cache = Cache::builder()
            .capacity(cache_capacity as u64)
            .build()
            .await
            .map_err(|_| PermissionError::InvalidCacheCapacity)?;
        Ok(Self {
            role,
            policy_cache: Arc::new(policy_cache),
            cache_capacity,
            rate_limiter: Some(match backend {
                RateLimitBackend::TokenBucket {
                    max_requests,
                    window_secs,
                } => token_bucket_limiter(max_requests, window_secs),
                RateLimitBackend::External(limiter) => limiter,
            }),
            #[cfg(feature = "audit")]
            audit_logger: None,
            check_stats: Arc::new(PermissionCheckStats::new()),
            permission_provider: None,
            in_flight: DashMap::new(),
        })
    }

    /// 挂载限流拒绝审计器（audit feature）
    ///
    /// 挂载后，每次因速率限制拒绝的权限检查都会产生一条审计事件
    /// （operation=`rate_limit_exceeded`，result=Failure，severity=Medium）。
    #[cfg(feature = "audit")]
    pub fn set_audit_logger(&mut self, logger: Arc<AuditLogger>) {
        self.audit_logger = Some(logger);
    }

    /// 创建新的权限上下文（使用 DbConfig 配置）
    ///
    /// 从 `DbConfig.cache_config()` 获取缓存容量配置。
    /// 这是推荐的创建方式，确保缓存容量可配置。
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称
    /// * `config` - 数据库配置引用
    ///
    /// # Example
    ///
    /// ```ignore
    /// use dbnexus::permission::PermissionContext;
    /// use dbnexus::DbConfig;
    ///
    /// let config = DbConfig {
    ///     url: "sqlite::memory:".to_string(),
    ///     cache_config: dbnexus::CacheConfig {
    ///         policy_cache_capacity: 8192,
    ///         ..Default::default()
    ///     },
    ///     ..Default::default()
    /// };
    ///
    /// let ctx = PermissionContext::with_config("admin".to_string(), &config).await;
    /// ```
    pub async fn with_config(
        role: String,
        config: &crate::foundation::DbConfig,
    ) -> Result<Self, PermissionError> {
        let cache_capacity = config.cache_config.policy_cache_capacity as usize;
        Self::with_cache_size(role, cache_capacity).await
    }

    /// 创建新的权限上下文（使用 DbConfig 配置和速率限制）
    ///
    /// 从 `DbConfig.cache_config` 获取缓存容量配置，同时支持速率限制。
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称
    /// * `config` - 数据库配置引用
    /// * `max_requests` - 速率限制最大请求数
    /// * `window_secs` - 速率限制时间窗口（秒）
    pub async fn with_config_and_rate_limit(
        role: String,
        config: &crate::foundation::DbConfig,
        max_requests: u32,
        window_secs: u64,
    ) -> Result<Self, PermissionError> {
        let cache_capacity = config.cache_config.policy_cache_capacity as usize;
        Self::with_cache_size_and_rate_limit(role, cache_capacity, max_requests, window_secs).await
    }

    /// 获取角色
    pub fn role(&self) -> &str {
        &self.role
    }

    /// 获取权限检查统计
    pub fn check_stats(&self) -> &Arc<PermissionCheckStats> {
        &self.check_stats
    }

    /// 创建新的权限上下文（同步版本，使用默认配置）
    ///
    /// 此方法为需要同步创建权限上下文的场景提供便利，例如在 Session 初始化过程中。
    /// 使用默认的缓存大小和速率限制配置。
    ///
    /// # Panics
    ///
    /// 在 async 上下文中调用时不会死锁（使用 `block_in_place`）。
    /// 若无 tokio 运行时，会创建临时运行时。
    pub fn new_with_defaults(role: String) -> Self {
        let cache = create_cache_sync(DEFAULT_POLICY_CACHE_CAPACITY);
        Self {
            role,
            policy_cache: Arc::new(cache),
            cache_capacity: DEFAULT_POLICY_CACHE_CAPACITY,
            rate_limiter: Some(token_bucket_limiter(
                DEFAULT_RATE_LIMIT_MAX_REQUESTS,
                DEFAULT_RATE_LIMIT_WINDOW_SECS,
            )),
            #[cfg(feature = "audit")]
            audit_logger: None,
            check_stats: Arc::new(PermissionCheckStats::new()),
            permission_provider: None,
            in_flight: DashMap::new(),
        }
    }

    /// 创建新的权限上下文（同步版本，使用 DbConfig 配置）
    ///
    /// 从 `DbConfig.cache_config` 获取缓存容量配置。
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称
    /// * `config` - 数据库配置引用
    pub fn new_with_config(role: String, config: &crate::foundation::DbConfig) -> Self {
        let cache_capacity = config.cache_config.policy_cache_capacity as usize;
        let cache = create_cache_sync(cache_capacity);
        Self {
            role,
            policy_cache: Arc::new(cache),
            cache_capacity,
            rate_limiter: Some(token_bucket_limiter(
                DEFAULT_RATE_LIMIT_MAX_REQUESTS,
                DEFAULT_RATE_LIMIT_WINDOW_SECS,
            )),
            #[cfg(feature = "audit")]
            audit_logger: None,
            check_stats: Arc::new(PermissionCheckStats::new()),
            permission_provider: None,
            in_flight: DashMap::new(),
        }
    }

    /// 创建新的权限上下文（使用指定的缓存实例）
    ///
    /// 此方法允许外部传入已创建的缓存实例，用于测试和高级使用场景。
    pub fn new(role: String, policy_cache: Arc<Cache<String, RolePolicy>>) -> Self {
        Self {
            role,
            policy_cache,
            cache_capacity: DEFAULT_POLICY_CACHE_CAPACITY,
            rate_limiter: Some(token_bucket_limiter(
                DEFAULT_RATE_LIMIT_MAX_REQUESTS,
                DEFAULT_RATE_LIMIT_WINDOW_SECS,
            )),
            #[cfg(feature = "audit")]
            audit_logger: None,
            check_stats: Arc::new(PermissionCheckStats::new()),
            permission_provider: None,
            in_flight: DashMap::new(),
        }
    }

    /// 创建新的权限上下文（使用指定的缓存实例和权限提供者）
    ///
    /// 此方法允许外部传入已创建的缓存实例和权限提供者，
    /// 用于测试和高级使用场景。权限提供者用于缓存未命中时重新加载策略。
    pub fn new_with_provider(
        role: String,
        policy_cache: Arc<Cache<String, RolePolicy>>,
        permission_provider: Arc<dyn PermissionProvider>,
    ) -> Self {
        Self {
            role,
            policy_cache,
            cache_capacity: DEFAULT_POLICY_CACHE_CAPACITY,
            rate_limiter: Some(token_bucket_limiter(
                DEFAULT_RATE_LIMIT_MAX_REQUESTS,
                DEFAULT_RATE_LIMIT_WINDOW_SECS,
            )),
            #[cfg(feature = "audit")]
            audit_logger: None,
            check_stats: Arc::new(PermissionCheckStats::new()),
            permission_provider: Some(permission_provider),
            in_flight: DashMap::new(),
        }
    }

    /// 创建新的权限上下文（使用指定的缓存实例、权限提供者和 DbConfig 配置）
    ///
    /// 此方法允许外部传入已创建的缓存实例和权限提供者，
    /// 同时从配置中获取速率限制参数。
    ///
    /// # Arguments
    ///
    /// * `role` - 角色名称
    /// * `policy_cache` - 已创建的缓存实例
    /// * `permission_provider` - 权限提供者
    /// * `config` - 数据库配置引用（用于获取速率限制配置）
    pub fn new_with_provider_and_config(
        role: String,
        policy_cache: Arc<Cache<String, RolePolicy>>,
        permission_provider: Arc<dyn PermissionProvider>,
        config: &crate::foundation::DbConfig,
    ) -> Self {
        Self {
            role,
            policy_cache,
            cache_capacity: config.cache_config.policy_cache_capacity as usize,
            rate_limiter: Some(token_bucket_limiter(
                DEFAULT_RATE_LIMIT_MAX_REQUESTS,
                DEFAULT_RATE_LIMIT_WINDOW_SECS,
            )),
            #[cfg(feature = "audit")]
            audit_logger: None,
            check_stats: Arc::new(PermissionCheckStats::new()),
            permission_provider: Some(permission_provider),
            in_flight: DashMap::new(),
        }
    }

    /// 执行实际的策略加载（不含缓存写入）
    fn do_load_policy(&self) -> Option<RolePolicy> {
        if let Some(provider) = &self.permission_provider
            && let Some(policy) = provider.get_role_policy(&self.role)
        {
            return Some(policy);
        }
        None
    }

    /// 使用请求合并（singleflight）方式加载策略
    ///
    /// 使用 tokio::sync::Mutex：
    /// - lock().await 会 park 任务直到锁可用（正确等待）
    /// - 第一个获得锁的任务为 leader，执行实际加载
    /// - leader 在锁内设置 done = true 后释放锁
    /// - follower park 在 lock().await，唤醒后检测 done = true 并跳过重新加载
    /// - stampede 计数器在 follower 检测到 done = true 时递增（thundering herd 发生）
    async fn get_or_load_policy_coalesced(&self) -> Option<RolePolicy> {
        // Step 1: 快速路径 - 缓存命中
        if let Some(policy) = self.policy_cache.get(&self.role).await.ok().flatten() {
            return Some(policy);
        }

        // Step 2: 获取或创建 in_flight entry（限定作用域，确保释放 DashMap 借用）
        let result = {
            let entry = self.in_flight.entry(self.role.clone()).or_insert_with(|| {
                Arc::new(InFlightEntry {
                    result: TokioMutex::new(None),
                    done: Arc::new(AtomicBool::new(false)),
                })
            });

            // 循环：follower 检测到 leader 失败后，重置 done 并重新获取锁作为 leader
            loop {
                // Step 3: 获得锁
                let mut guard = entry.result.lock().await;

                // Step 4: 检查 done 标志
                if entry.done.load(Ordering::SeqCst) {
                    // Follower: leader 已完成，检测到 thundering herd
                    self.check_stats.record_stampede();
                    let leader_result = (*guard).clone();
                    if let Some(policy) = leader_result {
                        // Leader 成功加载，返回结果
                        drop(guard);
                        self.check_stats.record_cache_hit();
                        break Some(policy);
                    }
                    // Leader 加载失败（无 provider 等），重置 done 并重新循环
                    entry.done.store(false, Ordering::SeqCst);
                    drop(guard);
                    continue;
                } else {
                    // Step 5: Leader: 获得锁且 done = false，开始加载
                    let result = self.do_load_policy();
                    *guard = result.clone();

                    // done = true 必须在释放锁之前设置（防止 follower 误认为需要重新加载）
                    entry.done.store(true, Ordering::SeqCst);

                    // 释放锁
                    drop(guard);

                    // 异步缓存插入
                    if let Some(ref policy) = result {
                        self.policy_cache.set(&self.role, policy).await.ok();
                    }

                    break result;
                }
            }
        };

        // 注意：in_flight 条目有意保留，不在此处移除。
        // 原因：并发 follower 可能在 leader 完成后仍有延迟到达，
        // 提前移除会导致 follower 创建新 entry 并触发重复加载（stampede 防护失效）。
        // in_flight 条目内存开销很小（每个 role 一个 Arc<InFlightEntry>），
        // 且角色数量通常有界。

        result
    }

    /// 设置权限提供者
    ///
    /// 允许在创建后设置权限提供者，用于缓存未命中时重新加载策略。
    pub fn set_permission_provider(&mut self, provider: Arc<dyn PermissionProvider>) {
        self.permission_provider = Some(provider);
    }

    /// 尝试重新加载权限策略
    ///
    /// 当缓存未命中时，尝试从权限提供者重新加载策略到缓存。
    /// 此方法用于解决 TOCTOU (Time-of-check to time-of-use) 竞争条件问题。
    ///
    /// # Returns
    ///
    /// - `true` - 成功加载策略到缓存
    /// - `false` - 无法加载（无权限提供者或角色不存在）
    pub async fn try_reload_policy(&self) -> bool {
        if let Some(provider) = &self.permission_provider
            && let Some(policy) = provider.get_role_policy(&self.role)
        {
            self.policy_cache.set(&self.role, &policy).await.ok();
            return true;
        }
        false
    }

    /// 检查表访问权限（决策版 - 区分限流拒绝与策略拒绝）
    ///
    /// 此方法会先经限流端口检查，然后检查缓存。如果缓存未命中，
    /// 会尝试从权限提供者重新加载策略，避免 TOCTOU 竞争条件。
    ///
    /// # Returns
    ///
    /// - [`TableAccessDecision::Allowed`] - 允许访问
    /// - [`TableAccessDecision::Denied`] - 策略拒绝（403 语义）
    /// - [`TableAccessDecision::RateLimited`] - 限流拒绝（429 语义，携带 Retry-After）
    ///
    /// # Security
    ///
    /// 此方法实现了安全的缓存未命中处理：
    /// 1. 缓存命中时直接返回缓存的策略结果
    /// 2. 缓存未命中时尝试重新加载策略
    /// 3. 重新加载成功后重新检查权限
    /// 4. 重新加载失败时安全地拒绝访问
    ///
    /// 限流后端故障时 fail-closed：按限流拒绝处理（计入 `rate_limited_checks`
    /// 并产生告警日志），绝不静默放行。
    pub async fn check_table_access_decision(
        &self,
        table: &str,
        operation: &PermissionAction,
    ) -> TableAccessDecision {
        // 1. 检查速率限制
        if let Some(limiter) = &self.rate_limiter {
            let decision = match limiter.check(&self.role).await {
                Ok(decision) => decision,
                Err(err) => {
                    // 后端故障 fail-closed：按限流拒绝处理，显性记录故障
                    log::warn!(
                        "{}",
                        i18n::t("perm-rate-limiter-error", &[("error", err.to_string())])
                    );
                    dbnexus_limiter_port::RateLimitDecision::deny(None)
                }
            };
            if !decision.allowed {
                self.check_stats.record_rate_limited();
                self.audit_rate_limited(table, operation, decision.retry_after)
                    .await;
                return TableAccessDecision::RateLimited {
                    retry_after: decision.retry_after,
                };
            }
        }

        // 2. 尝试从缓存获取，如果未命中则尝试加载
        if let Some(policy) = self.policy_cache.get(&self.role).await.ok().flatten() {
            // 缓存命中
            let allowed = policy.allows(table, operation);
            if allowed {
                self.check_stats.record_allowed();
            } else {
                self.check_stats.record_denied();
            }
            self.check_stats.record_cache_hit();
            return Self::policy_decision(allowed);
        }

        // 缓存未命中，使用 stampede-protected 加载
        self.check_stats.record_cache_miss();

        match self.get_or_load_policy_coalesced().await {
            Some(policy) => {
                let allowed = policy.allows(table, operation);
                if allowed {
                    self.check_stats.record_allowed();
                } else {
                    self.check_stats.record_denied();
                }
                Self::policy_decision(allowed)
            }
            None => {
                self.check_stats.record_denied();
                TableAccessDecision::Denied
            }
        }
    }

    /// 策略判定结果映射
    fn policy_decision(allowed: bool) -> TableAccessDecision {
        if allowed {
            TableAccessDecision::Allowed
        } else {
            TableAccessDecision::Denied
        }
    }

    /// 限流拒绝审计事件（audit feature；未挂载审计器时为 no-op）
    #[cfg(feature = "audit")]
    async fn audit_rate_limited(
        &self,
        table: &str,
        operation: &PermissionAction,
        retry_after: Option<Duration>,
    ) {
        let Some(logger) = &self.audit_logger else {
            return;
        };
        let retry_after_secs = retry_after.map_or(0, |d| d.as_secs());
        let mut event = AuditEvent::create("table_access", table, &self.role)
            .with_user(&self.role, "")
            .with_result(AuditStatus::Failure)
            .with_severity(AuditSeverity::Medium)
            .with_extra(&format!(
                r#"{{"reason":"rate_limit_exceeded","operation":"{operation}","retry_after_secs":{retry_after_secs}}}"#
            ));
        event.operation = AuditOperation::Other("rate_limit_exceeded".to_string());
        if let Err(err) = logger.log(event).await {
            // 审计失败不阻断权限判定，但必须显性记录
            log::warn!(
                "{}",
                i18n::t(
                    "perm-rate-limit-audit-dropped",
                    &[("error", err.to_string())]
                )
            );
        }
    }

    /// 限流拒绝审计 no-op（未启用 audit feature）
    #[cfg(not(feature = "audit"))]
    async fn audit_rate_limited(
        &self,
        _table: &str,
        _operation: &PermissionAction,
        _retry_after: Option<Duration>,
    ) {
    }

    /// 检查表访问权限（布尔简版）
    ///
    /// 限流拒绝与策略拒绝均返回 `false`；需区分两者（429/403 语义）时使用
    /// [`check_table_access_decision`](Self::check_table_access_decision)。
    pub async fn check_table_access(&self, table: &str, operation: &PermissionAction) -> bool {
        matches!(
            self.check_table_access_decision(table, operation).await,
            TableAccessDecision::Allowed
        )
    }

    /// 验证角色是否有权限执行特定操作（细粒度验证）
    ///
    /// 此方法提供比 `check_table_access` 更详细的验证，
    /// 包括操作类型、条件的详细检查
    ///
    /// # Arguments
    ///
    /// * `table` - 表名
    /// * `operation` - 操作类型
    /// * `conditions` - 可选的额外条件（如行级安全策略）
    ///
    /// # Returns
    ///
    /// 如果有权限返回 true，否则返回 false
    ///
    /// # 条件评估
    ///
    /// 若提供 `conditions`，当前实现会拒绝访问（fail-safe），
    /// 因为行级安全策略需要行级上下文（当前不可用）。
    /// 调用方应使用无条件的 `check_table_access` 进行表级权限检查。
    pub async fn verify_operation(
        &self,
        table: &str,
        operation: &PermissionAction,
        conditions: Option<&str>,
    ) -> bool {
        // 基础权限检查
        if !self.check_table_access(table, operation).await {
            return false;
        }

        // 条件评估：行级安全策略需要行级上下文，当前不支持；
        // 采用 fail-safe 策略——有未评估的条件时拒绝访问（契约见 doc comment §条件评估）
        if conditions.is_some() {
            return false;
        }

        true
    }

    /// 批量检查多个权限
    ///
    /// 一次性检查多个表和操作的权限，比单独调用更高效
    ///
    /// # Arguments
    ///
    /// * `permissions` - 权限检查请求列表
    ///
    /// # Returns
    ///
    /// 每个请求的检查结果
    pub async fn batch_check_permissions(
        &self,
        permissions: &[(String, PermissionAction)],
    ) -> Vec<bool> {
        let mut results = Vec::with_capacity(permissions.len());

        for (table, operation) in permissions {
            results.push(self.check_table_access(table, operation).await);
        }

        results
    }

    /// 加载权限策略到缓存
    ///
    /// 从权限配置文件中加载指定角色的策略并缓存
    ///
    /// # Errors
    ///
    /// 如果加载失败，返回错误信息
    pub async fn load_policy(&self, config: &PermissionConfig) -> Result<(), String> {
        if let Some(policy) = config.get_role_policy(&self.role) {
            self.policy_cache.set(&self.role, policy).await.ok();
            Ok(())
        } else {
            Err("Role not found in permission config".to_string())
        }
    }

    /// 获取缓存统计信息
    pub async fn cache_stats(&self) -> CacheStats {
        CacheStats {
            cached_roles: self.policy_cache.len().await.unwrap_or(0) as usize,
            capacity: self.cache_capacity,
        }
    }

    /// 获取缓存指标（命中率、未命中数、击穿事件数、缓存条目数）
    pub async fn get_cache_metrics(&self) -> (f64, u64, u64, usize) {
        let snapshot = self.check_stats.snapshot();
        let hit_rate = snapshot.cache_hit_rate();
        let miss_count = snapshot.cache_misses;
        let stampede_count = snapshot.stampede_events;
        let cache_size = self.policy_cache.len().await.unwrap_or(0) as usize;
        (hit_rate, miss_count, stampede_count, cache_size)
    }

    /// 清除权限缓存
    pub async fn clear_cache(&self) {
        self.policy_cache.clear().await.ok();
    }
}

#[cfg(all(test, feature = "cache"))]
mod tests {
    use super::*;
    use crate::access::MemoryPermissionProvider;
    use crate::access::TablePermission;
    #[cfg(feature = "permission-engine")]
    use futures;

    /// 创建测试用缓存的辅助函数
    async fn create_test_cache() -> Arc<Cache<String, RolePolicy>> {
        Arc::new(
            Cache::builder()
                .capacity(256)
                .build()
                .await
                .expect("Failed to create test cache"),
        )
    }

    /// PermissionContext 创建和访问测试
    #[tokio::test]
    async fn test_permission_context_creation() {
        let cache = create_test_cache().await;
        let ctx = PermissionContext::new("admin".to_string(), cache);

        assert_eq!(ctx.role(), "admin");
    }

    #[tokio::test]
    async fn test_permission_context_load_policy_then_check_access() {
        let config = PermissionConfig {
            roles: [(
                "test_role".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        let ctx = PermissionContext::with_cache_size("test_role".to_string(), 256)
            .await
            .unwrap();
        ctx.load_policy(&config).await.unwrap();

        assert!(
            ctx.check_table_access("users", &PermissionAction::Select)
                .await
        );
        assert!(
            !ctx.check_table_access("users", &PermissionAction::Delete)
                .await
        );
    }

    #[tokio::test]
    async fn test_permission_context_check_table_access_with_config_role_missing_denies() {
        let _config = PermissionConfig {
            roles: [(
                "defined_role".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        let ctx = PermissionContext::with_cache_size("missing_role".to_string(), 256)
            .await
            .unwrap();
        assert!(
            !ctx.check_table_access("users", &PermissionAction::Select)
                .await
        );
    }

    #[tokio::test]
    async fn test_permission_context_check_table_access_rate_limited_denies() {
        let config = PermissionConfig {
            roles: [(
                "test_role".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        let ctx =
            PermissionContext::with_cache_size_and_rate_limit("test_role".to_string(), 256, 1, 60)
                .await
                .unwrap();

        // Load policy first
        ctx.load_policy(&config).await.unwrap();

        // First request should succeed
        assert!(
            ctx.check_table_access("users", &PermissionAction::Select)
                .await
        );
        // Second request should be rate limited
        assert!(
            !ctx.check_table_access("users", &PermissionAction::Select)
                .await
        );
    }

    // ============================================================================
    // 缓存未命中容错机制测试 (TOCTOU 修复验证)
    // ============================================================================

    /// 缓存未命中时自动重新加载策略 - 成功场景
    #[tokio::test]
    async fn test_cache_miss_reload_success() {
        // 创建权限配置
        let config = PermissionConfig {
            roles: [(
                "test_role".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select, PermissionAction::Insert],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        // 创建权限提供者
        let provider = Arc::new(MemoryPermissionProvider::new());
        provider
            .add_role("test_role", config.roles.get("test_role").unwrap().clone())
            .await;

        // 创建缓存和权限上下文（不预加载策略）
        let cache = create_test_cache().await;
        let ctx = PermissionContext::new_with_provider("test_role".to_string(), cache, provider);

        // 缓存未命中时应该自动重新加载并检查权限
        assert!(
            ctx.check_table_access("users", &PermissionAction::Select)
                .await
        );
        assert!(
            ctx.check_table_access("users", &PermissionAction::Insert)
                .await
        );
        assert!(
            !ctx.check_table_access("users", &PermissionAction::Delete)
                .await
        );
        assert!(
            !ctx.check_table_access("orders", &PermissionAction::Select)
                .await
        );
    }

    /// 缓存未命中时无权限提供者 - 安全拒绝
    #[tokio::test]
    async fn test_cache_miss_no_provider_safe_deny() {
        // 创建权限上下文（不配置权限提供者）
        let cache = create_test_cache().await;
        let ctx = PermissionContext::new("test_role".to_string(), cache);

        // 缓存未命中且无权限提供者时应该安全拒绝
        assert!(
            !ctx.check_table_access("users", &PermissionAction::Select)
                .await
        );

        // 验证统计信息
        let stats = ctx.check_stats().snapshot();
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.denied_checks, 1);
    }

    /// 缓存未命中时角色不存在于提供者 - 安全拒绝
    #[tokio::test]
    async fn test_cache_miss_role_not_found_safe_deny() {
        // 创建空的权限提供者
        let provider = Arc::new(MemoryPermissionProvider::new());

        // 创建权限上下文
        let cache = create_test_cache().await;
        let ctx =
            PermissionContext::new_with_provider("non_existent_role".to_string(), cache, provider);

        // 角色不存在时应该安全拒绝
        assert!(
            !ctx.check_table_access("users", &PermissionAction::Select)
                .await
        );

        // 验证统计信息
        let stats = ctx.check_stats().snapshot();
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.denied_checks, 1);
    }

    /// try_reload_policy 方法测试 - 成功场景
    #[tokio::test]
    async fn test_try_reload_policy_success() {
        // 创建权限配置
        let config = PermissionConfig {
            roles: [(
                "admin".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "*".to_string(),
                        operations: vec![
                            PermissionAction::Select,
                            PermissionAction::Insert,
                            PermissionAction::Update,
                            PermissionAction::Delete,
                        ],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        // 创建权限提供者
        let provider = Arc::new(MemoryPermissionProvider::new());
        provider
            .add_role("admin", config.roles.get("admin").unwrap().clone())
            .await;

        // 创建权限上下文
        let cache = create_test_cache().await;
        let ctx = PermissionContext::new_with_provider("admin".to_string(), cache, provider);

        // 调用 try_reload_policy
        let result = ctx.try_reload_policy().await;
        assert!(result);

        // 验证策略已加载到缓存
        let cached = ctx
            .policy_cache
            .get(&"admin".to_string())
            .await
            .ok()
            .flatten();
        assert!(cached.is_some());
    }

    /// try_reload_policy 方法测试 - 无权限提供者
    #[tokio::test]
    async fn test_try_reload_policy_no_provider() {
        // 创建权限上下文（无权限提供者）
        let cache = create_test_cache().await;
        let ctx = PermissionContext::new("admin".to_string(), cache);

        // 调用 try_reload_policy
        let result = ctx.try_reload_policy().await;
        assert!(!result);
    }

    /// 缓存命中后缓存未命中的混合场景
    #[tokio::test]
    async fn test_cache_hit_then_miss_reload() {
        // 创建权限配置
        let config = PermissionConfig {
            roles: [(
                "editor".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "articles".to_string(),
                        operations: vec![
                            PermissionAction::Select,
                            PermissionAction::Insert,
                            PermissionAction::Update,
                        ],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        // 创建权限提供者
        let provider = Arc::new(MemoryPermissionProvider::new());
        provider
            .add_role("editor", config.roles.get("editor").unwrap().clone())
            .await;

        // 创建权限上下文
        let cache = create_test_cache().await;
        let ctx = PermissionContext::new_with_provider("editor".to_string(), cache, provider);

        // 首次访问（缓存未命中，自动重新加载）
        assert!(
            ctx.check_table_access("articles", &PermissionAction::Select)
                .await
        );

        // 验证缓存命中
        let stats_after_hit = ctx.check_stats().snapshot();
        assert!(stats_after_hit.cache_hits > 0 || stats_after_hit.cache_misses > 0);

        // 清除缓存
        ctx.clear_cache().await;

        // 再次访问（缓存未命中，自动重新加载）
        assert!(
            ctx.check_table_access("articles", &PermissionAction::Insert)
                .await
        );
        assert!(
            !ctx.check_table_access("articles", &PermissionAction::Delete)
                .await
        );
    }

    /// set_permission_provider 方法测试
    #[tokio::test]
    async fn test_set_permission_provider() {
        // 创建权限配置
        let config = PermissionConfig {
            roles: [(
                "viewer".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "reports".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        // 创建权限提供者
        let provider = Arc::new(MemoryPermissionProvider::new());
        provider
            .add_role("viewer", config.roles.get("viewer").unwrap().clone())
            .await;

        // 创建权限上下文（无权限提供者）
        let cache = create_test_cache().await;
        let mut ctx = PermissionContext::new("viewer".to_string(), cache);

        // 首次访问（无权限提供者，应该拒绝）
        assert!(
            !ctx.check_table_access("reports", &PermissionAction::Select)
                .await
        );

        // 设置权限提供者
        ctx.set_permission_provider(provider);

        // 清除缓存后再次访问（现在应该能自动重新加载）
        ctx.clear_cache().await;
        assert!(
            ctx.check_table_access("reports", &PermissionAction::Select)
                .await
        );
        assert!(
            !ctx.check_table_access("reports", &PermissionAction::Insert)
                .await
        );
    }

    /// PermissionContext 使用 DbConfig 配置化缓存容量
    #[tokio::test]
    async fn test_permission_context_with_config() {
        use crate::foundation::{CacheConfig, DbConfig};

        // 创建自定义缓存容量配置
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            cache_config: CacheConfig {
                policy_cache_capacity: 8192,
                ..Default::default()
            },
            ..Default::default()
        };

        // 使用配置创建权限上下文
        let ctx = PermissionContext::with_config("admin".to_string(), &config)
            .await
            .unwrap();

        // 验证缓存容量
        let stats = ctx.cache_stats().await;
        assert_eq!(stats.capacity, 8192);
    }

    /// PermissionContext 同步版本使用 DbConfig 配置
    #[test]
    fn test_permission_context_new_with_config() {
        use crate::foundation::{CacheConfig, DbConfig};

        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();

        // 创建自定义缓存容量配置
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            cache_config: CacheConfig {
                policy_cache_capacity: 16384,
                ..Default::default()
            },
            ..Default::default()
        };

        // 使用配置创建权限上下文（同步版本）
        let ctx = PermissionContext::new_with_config("admin".to_string(), &config);

        // 验证缓存容量
        let stats = rt.block_on(async { ctx.cache_stats().await });
        assert_eq!(stats.capacity, 16384);
    }

    /// PermissionContext 默认缓存容量测试
    #[tokio::test]
    async fn test_permission_context_default_capacity() {
        let ctx = PermissionContext::new_default().await.unwrap();

        // 验证默认缓存容量
        let stats = ctx.cache_stats().await;
        assert_eq!(stats.capacity, 4096);
    }

    /// PermissionContext 自定义缓存容量测试
    #[tokio::test]
    async fn test_permission_context_custom_capacity() {
        let ctx = PermissionContext::with_cache_size("admin".to_string(), 2048)
            .await
            .unwrap();

        // 验证自定义缓存容量
        let stats = ctx.cache_stats().await;
        assert_eq!(stats.capacity, 2048);
    }

    /// PermissionContext 配置化缓存容量与速率限制组合测试
    #[tokio::test]
    async fn test_permission_context_with_config_and_rate_limit() {
        use crate::foundation::{CacheConfig, DbConfig};

        // 创建自定义缓存容量配置
        let config = DbConfig {
            url: "sqlite::memory:".to_string(),
            cache_config: CacheConfig {
                policy_cache_capacity: 4096,
                ..Default::default()
            },
            ..Default::default()
        };

        // 使用配置和速率限制创建权限上下文
        let ctx = PermissionContext::with_config_and_rate_limit(
            "admin".to_string(),
            &config,
            10, // max_requests
            60, // window_secs
        )
        .await
        .unwrap();

        // 验证缓存容量
        let stats = ctx.cache_stats().await;
        assert_eq!(stats.capacity, 4096);
    }

    /// 缓存击穿防护 - 并发请求触发 stampede 保护
    #[tokio::test]
    async fn test_cache_stampede_counter_increments() {
        use crate::access::TablePermission;

        let config = PermissionConfig {
            roles: [(
                "reports_role".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "reports".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        let provider = Arc::new(MemoryPermissionProvider::new());
        provider
            .add_role(
                "reports_role",
                config.roles.get("reports_role").unwrap().clone(),
            )
            .await;

        let cache = create_test_cache().await;
        let ctx = PermissionContext::new_with_provider("reports_role".to_string(), cache, provider);

        // 初始状态：stampede 计数为 0
        let initial = ctx.check_stats.snapshot();
        assert_eq!(initial.stampede_events, 0);

        // 第一次访问：清除缓存 + 检查（无并发，无击穿）
        ctx.clear_cache().await;
        let _ = ctx
            .check_table_access("reports", &PermissionAction::Select)
            .await;

        let after_first = ctx.check_stats.snapshot();
        assert_eq!(
            after_first.stampede_events, 0,
            "Single request should not count as stampede"
        );

        // 第二次访问：个请求（会触发击穿）
        ctx.clear_cache().await;
        let mut handles = Vec::new();
        for _ in 0..10 {
            let ctx_clone = ctx.clone();
            handles.push(tokio::spawn(async move {
                ctx_clone
                    .check_table_access("reports", &PermissionAction::Select)
                    .await
            }));
        }
        let results = futures::future::join_all(handles).await;
        for r in results {
            assert!(r.is_ok_and(|v| v));
        }

        // 验证：stampede 事件数应该 > 0（说明击穿防护触发了）
        let after_concurrent = ctx.check_stats.snapshot();
        assert!(
            after_concurrent.stampede_events > 0,
            "Concurrent requests should trigger stampede protection, got {} events",
            after_concurrent.stampede_events
        );
    }

    /// 缓存击穿防护 - get_cache_metrics 返回正确指标
    #[tokio::test]
    async fn test_get_cache_metrics() {
        let config = PermissionConfig {
            roles: [(
                "admin".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "*".to_string(),
                        operations: vec![
                            PermissionAction::Select,
                            PermissionAction::Insert,
                            PermissionAction::Update,
                            PermissionAction::Delete,
                        ],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        let provider = Arc::new(MemoryPermissionProvider::new());
        provider
            .add_role("admin", config.roles.get("admin").unwrap().clone())
            .await;

        let cache = create_test_cache().await;
        let ctx = PermissionContext::new_with_provider("admin".to_string(), cache, provider);

        // 执行一些访问以产生指标
        ctx.clear_cache().await;
        ctx.check_table_access("users", &PermissionAction::Select)
            .await;
        ctx.check_table_access("users", &PermissionAction::Select)
            .await;
        ctx.check_table_access("orders", &PermissionAction::Select)
            .await;

        let (hit_rate, miss_count, _stampede_count, _cache_size) = ctx.get_cache_metrics().await;

        // 第一次 Select 产生缓存未命中，后续两次命中
        // 缓存命中率应大于 0（至少有后续命中）
        assert!(
            hit_rate > 0.0,
            "Should have cache hit rate > 0 after previous access"
        );
        // 至少有一次未命中（首次访问触发加载）
        assert!(
            miss_count >= 1,
            "Should have at least 1 cache miss for the first access"
        );
    }

    /// 验证 verify_operation 三态契约（行级条件 fail-safe）：
    /// 1) 表级无权限 → false
    /// 2) 表级有权限 + 无条件 → true
    /// 3) 表级有权限 + 行级条件（行级上下文当前不支持）→ fail-safe false
    #[tokio::test]
    async fn test_verify_operation_failsafe_contract() {
        let config = PermissionConfig {
            roles: [(
                "test_role".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };
        let ctx = PermissionContext::with_cache_size("test_role".to_string(), 256)
            .await
            .unwrap();
        ctx.load_policy(&config).await.unwrap();

        // 未授权表 → 拒绝
        assert!(
            !ctx.verify_operation("orders", &PermissionAction::Select, None)
                .await
        );
        // 授权表 + 无条件 → 允许
        assert!(
            ctx.verify_operation("users", &PermissionAction::Select, None)
                .await
        );
        // 授权表 + 行级条件 → fail-safe 拒绝（行级评估未实现，绝不因未评估条件放行）
        assert!(
            !ctx.verify_operation("users", &PermissionAction::Select, Some("tenant_id = 1"))
                .await
        );
    }

    // ===== 补充测试：构造器 / 批量检查 / Debug =====

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_context_new_default_constructors() {
        let ctx = PermissionContext::new_default().await.unwrap();
        assert_eq!(ctx.role(), "admin");

        let ctx = PermissionContext::new_default_with_rate_limit("ops".to_string())
            .await
            .unwrap();
        assert_eq!(ctx.role(), "ops");
        assert!(ctx.rate_limiter.is_some());

        // 同步构造器
        let ctx = PermissionContext::new_with_defaults("sync_role".to_string());
        assert_eq!(ctx.role(), "sync_role");
        assert!(ctx.rate_limiter.is_some());

        // 带 provider 与配置的同步构造器
        let cache = create_test_cache().await;
        let provider: Arc<dyn PermissionProvider> = Arc::new(MemoryPermissionProvider::new());
        let config = crate::foundation::DbConfig::default();
        let ctx = PermissionContext::new_with_provider_and_config(
            "prov_role".to_string(),
            cache,
            provider,
            &config,
        );
        assert_eq!(ctx.role(), "prov_role");

        // Debug 形态：只报布尔存在性
        let debug = format!("{ctx:?}");
        assert!(debug.contains("prov_role"), "got: {debug}");
        assert!(debug.contains("has_permission_provider: true"));
    }

    #[tokio::test]
    async fn test_context_load_policy_missing_role_errors() {
        let config = PermissionConfig {
            roles: [("other_role".to_string(), RolePolicy { tables: vec![] })]
                .into_iter()
                .collect(),
        };
        let ctx = PermissionContext::with_cache_size("me".to_string(), 64)
            .await
            .unwrap();
        let err = ctx.load_policy(&config).await;
        assert!(err.is_err(), "未定义角色加载必须报错");
    }

    #[tokio::test]
    async fn test_context_batch_check_permissions() {
        let config = PermissionConfig {
            roles: [(
                "batch_role".to_string(),
                RolePolicy {
                    tables: vec![TablePermission {
                        name: "users".to_string(),
                        operations: vec![PermissionAction::Select, PermissionAction::Insert],
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };
        let ctx = PermissionContext::with_cache_size("batch_role".to_string(), 64)
            .await
            .unwrap();
        ctx.load_policy(&config).await.unwrap();

        let results = ctx
            .batch_check_permissions(&[
                ("users".to_string(), PermissionAction::Select),
                ("users".to_string(), PermissionAction::Insert),
                ("users".to_string(), PermissionAction::Delete),
                ("secret".to_string(), PermissionAction::Select),
            ])
            .await;
        assert_eq!(results, vec![true, true, false, false]);
    }
}
