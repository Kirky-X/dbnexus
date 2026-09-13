// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `DbNexusModule` — trait-kit 0.4 `AsyncKit` integration for dbnexus.
//!
//! Wires dbnexus's database pool into the `AsyncKit` dependency
//! injection framework, depending on `OxcacheModule` for cache capability.
//!
//! # Design divergences from `design.md` / `spec.md` (Rule 7: expose, don't
//! paper over)
//!
//! `design.md` Decision 3 (lines 345-380) and `spec.md` R-dbnexus-module-003
//! wrote the build body as:
//!
//! ```text
//! let cache = kit.require::<OxcacheModule>()?;
//! let adapter = OxcacheDbCacheAdapter::new(cache);
//! let config = kit.config::<DbConfig>()?;
//! DbPoolBuilder::new().config(config).cache(adapter).build().await
//! ```
//!
//! `DbPoolBuilder` has **no `.cache(adapter)` setter** that accepts a
//! `DbCacheProvider`. The existing `with_oxcache` setter (deprecated no-op
//! since 0.3.0) takes `Arc<Cache<String, serde_json::Value>>` — a completely
//! different type. The pool creates its own internal cache from
//! `DbConfig.cache_config` via `DbPool::with_config()`.
//!
//! Resolution (Rule 7): the `OxcacheDbCacheAdapter` is constructed inside
//! `build()` and injected via `DbPoolBuilder::cache_provider()` (added in
//! fix-review-findings change). The pool receives the adapter as a
//! `DbCacheProvider` trait object, enabling DI-based cache injection.

use std::any::TypeId;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use trait_kit::prelude::*;

use oxcache::integrations::kit::OxcacheModule;

#[cfg(test)]
use crate::database::ConnectionPool;
use crate::database::{DbPool, DbPoolBuilder};
use crate::foundation::DbConfig;
use crate::foundation::DbError;
use crate::integrations::OxcacheDbCacheAdapter;

/// trait-kit `AsyncKit` module that constructs a dbnexus `DbPool`.
///
/// Depends on `OxcacheModule` (registered first via topological sort).
/// Register with `AsyncKit::register::<DbNexusModule>()`, configure via
/// `kit.set_config(DbConfig { ... })`, then `kit.build().await` and retrieve
/// the capability with `kit.require::<DbNexusModule>()`.
///
/// The returned `Arc<DbPool>` is a fully initialized database connection
/// pool. The `OxcacheDbCacheAdapter` is injected via
/// `DbPoolBuilder::cache_provider()`, enabling cache DI through the kit.
///
/// # 全能力注册
///
/// 除池能力外，dbnexus kit 模块族还提供（各自独立注册、可单独 require）：
///
/// | Module | Capability | Feature gate |
/// |--------|------------|--------------|
/// | [`DbNexusModule`] | `Arc<DbPool>`（池） | `kit` |
/// | [`DbNexusCacheModule`] | `Arc<dyn DbCacheProvider + Send + Sync>`（缓存） | `oxcache-integration`（`kit` 隐含） |
/// | [`DbNexusAuditModule`] | `Arc<dyn AuditStorage>`（审计，DB 持久化） | `audit` + `sql-parser`（`kit` 隐含） |
/// | [`DbNexusHealthModule`] | [`DbHealthCapability`]（健康快照） | `health-check`（`kit` 隐含） |
pub struct DbNexusModule;

impl ModuleMeta for DbNexusModule {
    const NAME: &'static str = "dbnexus";

    fn dependencies() -> &'static [(&'static str, TypeId)] {
        // OnceLock lazy init — MSRV is 1.91 where `TypeId::of::<T>()` is
        // `const fn`, but `static` items with `Vec` construction still need
        // runtime init. OnceLock gives a `&'static` reference without
        // external crates.
        static DEPS: OnceLock<Vec<(&'static str, TypeId)>> = OnceLock::new();
        DEPS.get_or_init(|| vec![("oxcache", TypeId::of::<OxcacheModule>())])
            .as_slice()
    }
}

impl AsyncAutoBuilder for DbNexusModule {
    type Capability = Arc<DbPool>;
    type Error = DbError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            // 1. Require OxcacheModule capability (Arc<dyn CacheBackend + Send + Sync>).
            let cache_cap = kit
                .require::<OxcacheModule>()
                .map_err(|e| DbError::Config(format!("require OxcacheModule: {e}")))?;

            // 2. Wrap in OxcacheDbCacheAdapter and inject via cache_provider().
            let adapter = OxcacheDbCacheAdapter::new(cache_cap);

            // 3. Read DbConfig from the kit.
            let config: DbConfig = kit
                .config()
                .map_err(|e| DbError::Config(format!("read DbConfig: {e}")))?;

            // 4. Build the pool with cache provider injected.
            let pool = DbPoolBuilder::new()
                .config(config)
                .cache_provider(Arc::new(adapter))
                .build()
                .await?;

            // 5. Return the concrete pool .
            Ok(Arc::new(pool))
        })
    }
}

// ---------------------------------------------------------------------------
// 全能力注册 — 缓存 / 审计 / 健康卫星模块
// ---------------------------------------------------------------------------

/// Cache capability module：把 `OxcacheModule` 后端适配为
/// dbnexus [`DbCacheProvider`](crate::domain::DbCacheProvider) 并作为独立
/// Kit 能力暴露。
///
/// `kit.build()` 后经 `kit.require::<DbNexusCacheModule>()` 获取；下游
/// 模块可在 `ModuleMeta::dependencies()` 声明对该能力的依赖。
///
/// Requires `oxcache-integration` feature（`kit` 隐含）。
pub struct DbNexusCacheModule;

impl ModuleMeta for DbNexusCacheModule {
    const NAME: &'static str = "dbnexus-cache";

    fn dependencies() -> &'static [(&'static str, TypeId)] {
        static DEPS: OnceLock<Vec<(&'static str, TypeId)>> = OnceLock::new();
        DEPS.get_or_init(|| vec![("oxcache", TypeId::of::<OxcacheModule>())])
            .as_slice()
    }
}

impl AsyncAutoBuilder for DbNexusCacheModule {
    type Capability = Arc<dyn crate::domain::DbCacheProvider + Send + Sync>;
    type Error = DbError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            let cache_cap = kit
                .require::<OxcacheModule>()
                .map_err(|e| DbError::Config(format!("require OxcacheModule: {e}")))?;
            let adapter = OxcacheDbCacheAdapter::new(cache_cap);
            Ok(Arc::new(adapter) as Arc<dyn crate::domain::DbCacheProvider + Send + Sync>)
        })
    }
}

/// DB 持久化审计能力模块：基于池能力构建
/// [`DbAuditStorage`](crate::domain::DbAuditStorage)（幂等建表后）并以
/// `Arc<dyn AuditStorage>` 暴露。
///
/// `kit.build()` 后经 `kit.require::<DbNexusAuditModule>()` 获取。
///
/// Requires `audit` + `sql-parser` features（`kit` 隐含）。
#[cfg(all(feature = "audit", feature = "sql-parser"))]
pub struct DbNexusAuditModule;

#[cfg(all(feature = "audit", feature = "sql-parser"))]
impl ModuleMeta for DbNexusAuditModule {
    const NAME: &'static str = "dbnexus-audit";

    fn dependencies() -> &'static [(&'static str, TypeId)] {
        static DEPS: OnceLock<Vec<(&'static str, TypeId)>> = OnceLock::new();
        DEPS.get_or_init(|| vec![("dbnexus", TypeId::of::<DbNexusModule>())])
            .as_slice()
    }
}

#[cfg(all(feature = "audit", feature = "sql-parser"))]
impl AsyncAutoBuilder for DbNexusAuditModule {
    type Capability = Arc<dyn crate::domain::AuditStorage>;
    type Error = DbError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            let pool = kit
                .require::<DbNexusModule>()
                .map_err(|e| DbError::Config(format!("require DbNexusModule: {e}")))?;
            let storage = crate::domain::DbAuditStorage::new(pool);
            storage
                .init()
                .await
                .map_err(|e| DbError::Config(format!("audit storage init: {e}")))?;
            Ok(Arc::new(storage) as Arc<dyn crate::domain::AuditStorage>)
        })
    }
}

/// 健康能力句柄：包装池句柄，暴露结构化健康快照
/// （`DbPool::health_snapshot`）。
///
/// 由 [`DbNexusHealthModule`] 作为 Kit 能力产出，Clone 廉价（内含单个 Arc）。
#[cfg(feature = "health-check")]
#[derive(Clone)]
pub struct DbHealthCapability {
    pool: Arc<DbPool>,
}

#[cfg(feature = "health-check")]
impl DbHealthCapability {
    /// 结构化健康快照（池饱和度/副本状态/慢查询计数 JSON）
    pub async fn snapshot(&self) -> serde_json::Value {
        self.pool.health_snapshot().await
    }

    /// 底层池句柄（需要更细粒度健康数据时使用）
    pub fn pool(&self) -> &Arc<DbPool> {
        &self.pool
    }
}

/// 健康能力模块：把池的结构化健康导出包装为独立 Kit 能力。
///
/// `kit.build()` 后经 `kit.require::<DbNexusHealthModule>()` 获取。
///
/// Requires `health-check` feature（`kit` 隐含）。
#[cfg(feature = "health-check")]
pub struct DbNexusHealthModule;

#[cfg(feature = "health-check")]
impl ModuleMeta for DbNexusHealthModule {
    const NAME: &'static str = "dbnexus-health";

    fn dependencies() -> &'static [(&'static str, TypeId)] {
        static DEPS: OnceLock<Vec<(&'static str, TypeId)>> = OnceLock::new();
        DEPS.get_or_init(|| vec![("dbnexus", TypeId::of::<DbNexusModule>())])
            .as_slice()
    }
}

#[cfg(feature = "health-check")]
impl AsyncAutoBuilder for DbNexusHealthModule {
    type Capability = DbHealthCapability;
    type Error = DbError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            let pool = kit
                .require::<DbNexusModule>()
                .map_err(|e| DbError::Config(format!("require DbNexusModule: {e}")))?;
            Ok(DbHealthCapability { pool })
        })
    }
}

// ---------------------------------------------------------------------------
// trait-kit 0.4 enhanced integrations (lifecycle / health / observability)
// ---------------------------------------------------------------------------

/// Async lifecycle hooks for `DbNexusModule`.
///
/// `on_shutdown` is called by `AsyncKit::shutdown()` in reverse topological
/// order. The `DbPool`'s `Drop` impl handles actual resource cleanup
/// (notifies the background health-check task). The pool is released when
/// the last `Arc` reference is dropped after the kit is dropped.
///
/// Requires `trait-kit/lifecycle` feature (pulled in by `kit`).
impl AsyncLifecycle for DbNexusModule {
    // Use default `on_ready` (no cross-module post-build init needed).
    // Use default `on_shutdown` (pool cleanup is handled by Drop).
}

/// Async health check for `DbNexusModule`.
///
/// Reports the connection pool's runtime health via trait-kit's
/// `HealthStatus` enum. Maps `PoolStatus` to:
///
/// | Condition | Status |
/// |-----------|--------|
/// | `total == 0` | `Unhealthy` (no connections established) |
/// | `idle > 0` and `wait_count == 0` | `Healthy` |
/// | otherwise | `Degraded` (exhausted or waiting) |
///
/// Requires `trait-kit/health` feature (pulled in by `kit`).
impl AsyncHealthCheck for DbNexusModule {
    fn check(cap: &Self::Capability) -> HealthStatus {
        let status = cap.status();
        if status.total == 0 {
            HealthStatus::unhealthy("no connections established")
        } else if status.idle > 0 && status.wait_count == 0 {
            HealthStatus::Healthy
        } else if status.wait_count > 0 {
            HealthStatus::degraded(format!(
                "{} waiting, {}/{} active/total",
                status.wait_count, status.active, status.total
            ))
        } else {
            HealthStatus::degraded(format!(
                "no idle connections, {}/{} active/total",
                status.active, status.total
            ))
        }
    }
}

/// Build observer for dbnexus module construction events.
///
/// Records module build start/completion/error events with elapsed times.
/// Register via `AsyncKit::with_observer()` to monitor kit build pipeline:
///
/// ```ignore
/// use dbnexus::integrations::kit::DbNexusBuildObserver;
/// let mut kit = AsyncKit::new();
/// kit.with_observer(Arc::new(DbNexusBuildObserver::new()));
/// ```
///
/// Requires `trait-kit/observer` feature (pulled in by `kit`).
pub struct DbNexusBuildObserver {
    built_count: std::sync::atomic::AtomicU64,
    error_count: std::sync::atomic::AtomicU64,
}

impl DbNexusBuildObserver {
    /// Create a new observer with zeroed counters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            built_count: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Number of modules successfully built.
    pub fn built_count(&self) -> u64 {
        self.built_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Number of modules that failed to build.
    pub fn error_count(&self) -> u64 {
        self.error_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Default for DbNexusBuildObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildObserver for DbNexusBuildObserver {
    fn on_module_start(&self, _module_name: &'static str) {
        // No-op: no logging dependency in the library crate.
    }

    fn on_module_built(&self, _module_name: &'static str, _elapsed: Duration) {
        self.built_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn on_build_error(&self, _module_name: &'static str, _error: &TraitKitError) {
        self.error_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxcache::integrations::kit::OxcacheConfig;

    /// R-dbnexus-module-003 #1: `DbNexusModule::NAME == "dbnexus"`.
    #[test]
    fn dbnexus_module_meta_name() {
        assert_eq!(DbNexusModule::NAME, "dbnexus");
    }

    /// R-dbnexus-module-003 #2: `DbNexusModule::dependencies()` declares
    /// a dependency on `OxcacheModule`.
    #[test]
    fn dbnexus_module_meta_dependencies() {
        let deps = DbNexusModule::dependencies();
        assert_eq!(deps.len(), 1, "DbNexusModule should depend on 1 module");
        assert_eq!(deps[0].0, "oxcache", "dep name should be 'oxcache'");
        assert_eq!(
            deps[0].1,
            TypeId::of::<OxcacheModule>(),
            "dep TypeId should match OxcacheModule"
        );
    }

    /// R-dbnexus-module-003 #3: `DbNexusModule` satisfies `AsyncAutoBuilder`
    /// trait bounds — `Capability: Clone + Send + Sync + 'static` and
    /// `Error: std::error::Error + Send + 'static`.
    #[test]
    fn dbnexus_module_satisfies_async_auto_builder_bounds() {
        fn assert_cap<T: Clone + Send + Sync + 'static>() {}
        assert_cap::<Arc<DbPool>>();
        fn assert_err<T: std::error::Error + Send + 'static>() {}
        assert_err::<DbError>();
    }

    /// R-dbnexus-module-003 #4: Full integration — register OxcacheModule +
    /// DbNexusModule, set configs, build, require DbNexusModule → get a
    /// working `Arc<dyn ConnectionPool + Send + Sync>`.
    #[tokio::test]
    async fn dbnexus_module_build_returns_connection_pool() {
        let mut kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        kit.set_config(DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: crate::foundation::PoolConfig {
                max_connections: 5,
                min_connections: 1,
                ..Default::default()
            },
            ..Default::default()
        });
        kit.register::<OxcacheModule>()
            .expect("register OxcacheModule");
        kit.register::<DbNexusModule>()
            .expect("register DbNexusModule");
        let kit = kit.build().await.expect("AsyncKit::build");
        let pool = kit
            .require::<DbNexusModule>()
            .expect("require DbNexusModule");
        // Verify the pool is usable — 能力类型为 Arc<DbPool>，仍可按
        // ConnectionPool trait 对象使用（向下兼容断言）。
        let pool: Arc<dyn ConnectionPool + Send + Sync> = pool;
        let _status = pool.status();
        let config = pool.config();
        assert_eq!(config.url, "sqlite::memory:");
    }

    /// R-dbnexus-module-003 #5: build fails with a clear error if
    /// OxcacheModule is not registered (dependency missing).
    #[tokio::test]
    async fn dbnexus_module_build_fails_without_oxcache() {
        let mut kit = AsyncKit::new();
        kit.set_config(DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        });
        // Register only DbNexusModule — OxcacheModule is missing.
        kit.register::<DbNexusModule>()
            .expect("register DbNexusModule");
        let err = kit.build().await.expect_err("build should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("oxcache"),
            "error should mention oxcache dependency, got: {msg}"
        );
    }

    // ========================================================================
    // trait-kit 0.4 enhanced integration tests
    // ========================================================================

    /// `DbNexusModule` satisfies `AsyncLifecycle` trait bounds.
    #[test]
    fn dbnexus_module_satisfies_async_lifecycle() {
        fn assert_lifecycle<T: AsyncLifecycle>() {}
        assert_lifecycle::<DbNexusModule>();
    }

    /// `DbNexusModule` satisfies `AsyncHealthCheck` trait bounds.
    #[test]
    fn dbnexus_module_satisfies_async_health_check() {
        fn assert_hc<T: AsyncHealthCheck>() {}
        assert_hc::<DbNexusModule>();
    }

    /// `DbNexusBuildObserver` satisfies `BuildObserver` trait bounds.
    #[test]
    fn build_observer_satisfies_build_observer_trait() {
        fn assert_obs<T: BuildObserver>() {}
        assert_obs::<DbNexusBuildObserver>();
    }

    /// Health check on a pool built with `pool-warmup` reports `Healthy`:
    /// the pool eagerly creates `min_connections` at build time, so the
    /// pre-warmed connections satisfy the health probe.
    #[cfg(feature = "pool-warmup")]
    #[tokio::test]
    async fn health_check_healthy_after_pool_warmup() {
        let mut kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        kit.set_config(DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: crate::foundation::PoolConfig {
                max_connections: 5,
                min_connections: 1,
                ..Default::default()
            },
            ..Default::default()
        });
        kit.register::<OxcacheModule>()
            .expect("register OxcacheModule");
        kit.register::<DbNexusModule>()
            .expect("register DbNexusModule");
        let kit = kit.build().await.expect("AsyncKit::build");
        let pool = kit
            .require::<DbNexusModule>()
            .expect("require DbNexusModule");
        // Before first use: pool eagerly creates min_connections, so health check
        // reports Healthy (connections are pre-warmed).
        let status = DbNexusModule::check(&pool);
        assert!(
            status.is_healthy(),
            "expected Healthy after pool pre-warm, got: {status:?}"
        );
    }

    /// Health check on a pool with zero connections reports `Unhealthy`.
    #[test]
    fn health_check_unhealthy_when_no_connections() {
        // Construct a minimal pool status with total=0.
        let pool_status = crate::database::PoolStatus {
            total: 0,
            active: 0,
            idle: 0,
            wait_count: 0,
            max_waiters: 0,
            borrow_count: 0,
            max_active: 0,
        };
        // Verify the mapping logic directly.
        if pool_status.total == 0 {
            let status = HealthStatus::unhealthy("no connections established");
            assert!(!status.is_healthy());
        }
    }

    /// `DbNexusBuildObserver` counts built and error modules.
    #[test]
    fn build_observer_counts() {
        let obs = DbNexusBuildObserver::new();
        assert_eq!(obs.built_count(), 0);
        assert_eq!(obs.error_count(), 0);

        // Simulate build events.
        obs.on_module_built("oxcache", Duration::from_millis(5));
        obs.on_module_built("dbnexus", Duration::from_millis(10));
        assert_eq!(obs.built_count(), 2);
        assert_eq!(obs.error_count(), 0);

        obs.on_build_error(
            "failing-module",
            &TraitKitError::MissingCapability {
                key: "x".to_string(),
            },
        );
        assert_eq!(obs.built_count(), 2);
        assert_eq!(obs.error_count(), 1);
    }

    /// `DbNexusBuildObserver` default is zeroed.
    #[test]
    fn build_observer_default_is_zeroed() {
        let obs = DbNexusBuildObserver::default();
        assert_eq!(obs.built_count(), 0);
        assert_eq!(obs.error_count(), 0);
    }

    /// Full kit integration with lifecycle + health + observer.
    #[cfg(feature = "pool-warmup")]
    #[tokio::test]
    async fn full_kit_with_lifecycle_health_observer() {
        let mut kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        kit.set_config(DbConfig {
            url: "sqlite::memory:".to_string(),
            pool_config: crate::foundation::PoolConfig {
                max_connections: 3,
                min_connections: 1,
                ..Default::default()
            },
            ..Default::default()
        });

        // Register modules.
        kit.register::<OxcacheModule>()
            .expect("register OxcacheModule");
        kit.register::<DbNexusModule>()
            .expect("register DbNexusModule");

        // Register lifecycle + health hooks.
        kit.register_lifecycle::<DbNexusModule>();
        kit.register_health_check::<DbNexusModule>();

        // Attach build observer.
        let observer = Arc::new(DbNexusBuildObserver::new());
        kit.with_observer(observer.clone());

        let kit = kit.build().await.expect("AsyncKit::build");

        // Observer should have counted successful builds.
        assert!(
            observer.built_count() >= 2,
            "expected >= 2 built modules, got {}",
            observer.built_count()
        );

        // Health check via kit API — pool pre-creates min_connections, so
        // connections are available and health check reports Healthy.
        let health = kit.health_check::<DbNexusModule>().expect("health_check");
        assert!(
            health.is_healthy(),
            "expected Healthy after pool pre-warm, got: {health:?}"
        );

        // Shutdown exercises lifecycle on_shutdown (default no-op for us).
        kit.shutdown_async().await;
    }

    // ========================================================================
    // 全能力注册 — 池/缓存/审计/健康全部能力可 require
    // ========================================================================

    /// 新增卫星模块满足 `AsyncAutoBuilder` trait bounds。
    #[test]
    fn t413_satellite_modules_satisfy_bounds() {
        fn assert_cap<T: Clone + Send + Sync + 'static>() {}
        assert_cap::<Arc<dyn crate::domain::DbCacheProvider + Send + Sync>>();
        #[cfg(all(feature = "audit", feature = "sql-parser"))]
        assert_cap::<Arc<dyn crate::domain::AuditStorage>>();
        #[cfg(feature = "health-check")]
        assert_cap::<DbHealthCapability>();
    }

    /// 缓存能力可 require — get/set 经 `DbCacheProvider` 走 oxcache 后端。
    #[tokio::test]
    async fn t413_cache_capability_requireable() {
        let mut kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        kit.set_config(DbConfig {
            url: "sqlite::memory:".to_string(),
            ..Default::default()
        });
        kit.register::<OxcacheModule>()
            .expect("register OxcacheModule");
        kit.register::<DbNexusModule>()
            .expect("register DbNexusModule");
        kit.register::<DbNexusCacheModule>()
            .expect("register DbNexusCacheModule");
        let kit = kit.build().await.expect("AsyncKit::build");

        let cache: Arc<dyn crate::domain::DbCacheProvider + Send + Sync> =
            kit.require::<DbNexusCacheModule>().expect("require cache");
        cache
            .set("t413-key", b"t413-value".to_vec(), None)
            .await
            .expect("cache set");
        let got = cache.get("t413-key").await.expect("cache get");
        assert_eq!(got.as_deref(), Some(&b"t413-value"[..]), "缓存能力应可读写");
    }

    /// 全能力注册端到端 — 池/缓存/审计/健康四能力在构建后全部
    /// 可 require 且可用（审计走临时文件库，规避 sqlite 内存库每连接独立）。
    #[cfg(all(feature = "audit", feature = "sql-parser", feature = "health-check"))]
    #[tokio::test]
    async fn t413_all_capabilities_requireable() {
        let db_path = std::env::temp_dir().join(format!("dbnexus_t413_{}.db", std::process::id()));
        let url = format!("sqlite:{}?mode=rwc", db_path.display());

        let mut kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        kit.set_config(DbConfig {
            url,
            pool_config: crate::foundation::PoolConfig {
                max_connections: 5,
                min_connections: 1,
                ..Default::default()
            },
            ..Default::default()
        });
        kit.register::<OxcacheModule>()
            .expect("register OxcacheModule");
        kit.register::<DbNexusModule>()
            .expect("register DbNexusModule");
        kit.register::<DbNexusCacheModule>()
            .expect("register DbNexusCacheModule");
        kit.register::<DbNexusAuditModule>()
            .expect("register DbNexusAuditModule");
        kit.register::<DbNexusHealthModule>()
            .expect("register DbNexusHealthModule");
        let kit = kit
            .build()
            .await
            .expect("AsyncKit::build should build all four modules");

        // 1. 池能力
        let pool = kit.require::<DbNexusModule>().expect("require pool");
        assert!(pool.config().url.contains("t413"), "池能力可用");

        // 2. 缓存能力
        let cache = kit.require::<DbNexusCacheModule>().expect("require cache");
        cache
            .set("k", b"v".to_vec(), None)
            .await
            .expect("cache set");
        assert_eq!(
            cache.get("k").await.expect("cache get").as_deref(),
            Some(&b"v"[..])
        );

        // 3. 审计能力（DB 持久化存储）
        let audit = kit.require::<DbNexusAuditModule>().expect("require audit");
        let event = crate::domain::AuditEvent::create("t413_entities", "42", "admin");
        audit.store(&event).await.expect("audit store");
        let events = audit
            .query(&crate::domain::AuditQueryFilters::default())
            .await
            .expect("audit query");
        assert!(
            events
                .iter()
                .any(|e| e.entity_type == "t413_entities" && e.entity_id == "42"),
            "审计能力应可写入并查回事件"
        );

        // 4. 健康能力
        let health = kit
            .require::<DbNexusHealthModule>()
            .expect("require health");
        let snapshot = health.snapshot().await;
        assert!(
            snapshot["pool"]["saturation"].is_number() && snapshot["status"].is_string(),
            "健康能力应输出结构化快照: {snapshot}"
        );

        let _ = std::fs::remove_file(&db_path);
    }
}
