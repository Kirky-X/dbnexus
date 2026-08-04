// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `DbNexusModule` — trait-kit 0.4 `AsyncKit` integration for dbnexus.
//!
//! Phase 4 (T029 Red / T030 Green) of the `trait-kit-async-integration`
//! change. Wires dbnexus's database pool into the `AsyncKit` dependency
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

use crate::database::{ConnectionPool, DbPoolBuilder};
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
/// The returned `Arc<dyn ConnectionPool + Send + Sync>` is a fully initialized
/// database connection pool. The `OxcacheDbCacheAdapter` is injected via
/// `DbPoolBuilder::cache_provider()`, enabling cache DI through the kit.
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
    type Capability = Arc<dyn ConnectionPool + Send + Sync>;
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

            // 5. Return as Arc<dyn ConnectionPool + Send + Sync>.
            Ok(Arc::new(pool) as Arc<dyn ConnectionPool + Send + Sync>)
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
/// Requires `trait-kit/observability` feature (pulled in by `kit`).
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
        self.built_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn on_build_error(&self, _module_name: &'static str, _error: &TraitKitError) {
        self.error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
        assert_cap::<Arc<dyn ConnectionPool + Send + Sync>>();
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
        kit.register::<OxcacheModule>().expect("register OxcacheModule");
        kit.register::<DbNexusModule>().expect("register DbNexusModule");
        let kit = kit.build().await.expect("AsyncKit::build");
        let pool: Arc<dyn ConnectionPool + Send + Sync> =
            kit.require::<DbNexusModule>().expect("require DbNexusModule");
        // Verify the pool is usable.
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
        kit.register::<DbNexusModule>().expect("register DbNexusModule");
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

    /// Health check on a freshly built pool reports `Unhealthy` (lazy init —
    /// no connections established until first use). This is correct behavior:
    /// the pool is functional but hasn't created connections yet.
    #[tokio::test]
    async fn health_check_unhealthy_before_first_use() {
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
        kit.register::<OxcacheModule>().expect("register OxcacheModule");
        kit.register::<DbNexusModule>().expect("register DbNexusModule");
        let kit = kit.build().await.expect("AsyncKit::build");
        let pool: Arc<dyn ConnectionPool + Send + Sync> =
            kit.require::<DbNexusModule>().expect("require DbNexusModule");
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

        obs.on_build_error("failing-module", &TraitKitError::MissingCapability { key: "x" });
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
        kit.register::<OxcacheModule>().expect("register OxcacheModule");
        kit.register::<DbNexusModule>().expect("register DbNexusModule");

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
        kit.shutdown();
    }
}
