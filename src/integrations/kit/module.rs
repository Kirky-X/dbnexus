// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `DbNexusModule` — trait-kit 0.2.2 `AsyncKit` integration for dbnexus.
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
        // OnceLock lazy init — `TypeId::of::<T>()` became `const fn` in Rust
        // 1.91, but the project MSRV is 1.85, so we can't use a `static`
        // initializer. OnceLock (stable since 1.70) gives us a `&'static`
        // reference to a runtime-constructed `Vec`.
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
}
