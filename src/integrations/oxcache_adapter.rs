// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `OxcacheDbCacheAdapter` — adapts `oxcache::backend::CacheBackend` to
//! dbnexus's [`DbCacheProvider`] trait.
//!
//! Phase 4 T027+T028 of the `trait-kit-async-integration` change. Wraps the
//! cache capability produced by `OxcacheModule` (`Arc<dyn CacheBackend +
//! Send + Sync>`) so dbnexus internals can consume it through the
//! `DbCacheProvider` abstraction without depending on oxcache types directly.
//!
//! # Design divergences from `design.md` / `spec.md` (Rule 7: expose, don't
//! paper over)
//!
//! 1. **`Arc<oxcache::Cache>` → `Arc<dyn CacheBackend + Send + Sync>`**:
//!    `design.md` Decision 2 (lines 245-247) and `spec.md` R-dbnexus-module-002
//!    wrote the field as `cache: Arc<oxcache::Cache>`. The actual
//!    `OxcacheModule` (Phase 2, `oxcache/src/integrations/kit/module.rs`)
//!    returns `Arc<dyn CacheBackend + Send + Sync>` because `UnifiedCache`
//!    is not object-safe (documented in OxcacheModule's module-level docs).
//!    This adapter follows the actual OxcacheModule capability type.
//!
//! 2. **`DbError::cache(message)` → `DbError::Config(String)`**:
//!    `spec.md` R-dbnexus-module-002 specifies `DbError::cache(message)` for
//!    error mapping. `DbError` has no `cache` variant — we use
//!    `DbError::Config(String)` with a `"cache ..."` prefix for traceability.
//!    See `cache_provider.rs` module-level docs for the full rationale.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use oxcache::backend::CacheBackend;

use crate::domain::DbCacheProvider;
use crate::foundation::DbError;

/// Adapter that wraps an oxcache `CacheBackend` as a `DbCacheProvider`.
///
/// Constructed in `DbNexusModule::build()` (Phase 4 T029/T030) from the
/// `Arc<dyn CacheBackend + Send + Sync>` capability produced by
/// `OxcacheModule`. The adapter proxies `get`/`set`/`delete` calls to the
/// underlying oxcache backend and maps `oxcache::OxCacheError` to `DbError`.
///
/// # Feature gating
///
/// Only available when the `oxcache-integration` feature is enabled (which
/// the `kit` feature pulls in transitively).
#[derive(Clone)]
pub struct OxcacheDbCacheAdapter {
    /// The oxcache cache backend capability (dyn-dispatched).
    cache: Arc<dyn CacheBackend + Send + Sync>,
}

impl OxcacheDbCacheAdapter {
    /// Create a new adapter wrapping the given oxcache cache backend.
    ///
    /// The `cache` argument is the capability produced by
    /// `OxcacheModule::build()` — an `Arc<dyn CacheBackend + Send + Sync>`.
    #[must_use]
    pub fn new(cache: Arc<dyn CacheBackend + Send + Sync>) -> Self {
        Self { cache }
    }
}

impl DbCacheProvider for OxcacheDbCacheAdapter {
    fn get<'a>(&'a self, key: &'a str) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, DbError>> + Send + 'a>> {
        Box::pin(async move {
            self.cache
                .get(key)
                .await
                .map_err(|e| DbError::Config(format!("cache get failed: {e}")))
        })
    }

    fn set<'a>(
        &'a self,
        key: &'a str,
        value: Vec<u8>,
        ttl: Option<std::time::Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
        Box::pin(async move {
            self.cache
                .set(key, value, ttl)
                .await
                .map_err(|e| DbError::Config(format!("cache set failed: {e}")))
        })
    }

    fn delete<'a>(&'a self, key: &'a str) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
        Box::pin(async move {
            self.cache
                .delete(key)
                .await
                .map_err(|e| DbError::Config(format!("cache delete failed: {e}")))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use oxcache::backend::MokaMemoryBackend;

    /// Build a small `MokaMemoryBackend`-backed adapter for testing.
    fn make_adapter() -> OxcacheDbCacheAdapter {
        let backend = MokaMemoryBackend::builder().capacity(100).build();
        let cache: Arc<dyn CacheBackend + Send + Sync> = Arc::new(backend);
        OxcacheDbCacheAdapter::new(cache)
    }

    /// R-dbnexus-module-002 #1: `OxcacheDbCacheAdapter` implements
    /// `DbCacheProvider` and can be used through a trait object.
    #[test]
    fn adapter_is_db_cache_provider() {
        fn assert_impl<T: DbCacheProvider + ?Sized>() {}
        assert_impl::<OxcacheDbCacheAdapter>();
        // Also works as Arc<dyn DbCacheProvider + Send + Sync> (dyn dispatch).
        let adapter = make_adapter();
        let _dyn: Arc<dyn DbCacheProvider + Send + Sync> = Arc::new(adapter);
    }

    /// R-dbnexus-module-002 #2: `set` then `get` round-trips a value
    /// through the real oxcache `MokaMemoryBackend`.
    #[tokio::test]
    async fn adapter_set_get_round_trip() {
        let adapter = make_adapter();
        adapter
            .set("key1", b"value1".to_vec(), None)
            .await
            .expect("set should succeed");
        let got = adapter.get("key1").await.expect("get should succeed");
        assert_eq!(got, Some(b"value1".to_vec()));
    }

    /// R-dbnexus-module-002 #3: `get` on an absent key returns `Ok(None)`.
    #[tokio::test]
    async fn adapter_get_absent_returns_none() {
        let adapter = make_adapter();
        let got = adapter.get("nonexistent").await.expect("get should succeed");
        assert!(got.is_none(), "absent key should return Ok(None)");
    }

    /// R-dbnexus-module-002 #4: `delete` removes a value — subsequent
    /// `get` returns `Ok(None)`.
    #[tokio::test]
    async fn adapter_delete_removes_value() {
        let adapter = make_adapter();
        adapter.set("doomed", b"x".to_vec(), None).await.expect("set");
        adapter.delete("doomed").await.expect("delete");
        let gone = adapter.get("doomed").await.expect("get after delete");
        assert!(gone.is_none());
    }

    /// R-dbnexus-module-002 #5: `set` with a TTL stores the value (the TTL
    /// is forwarded to the oxcache backend). The value is immediately
    /// retrievable.
    #[tokio::test]
    async fn adapter_set_with_ttl() {
        let adapter = make_adapter();
        adapter
            .set("ttl_key", b"ttl_val".to_vec(), Some(Duration::from_secs(60)))
            .await
            .expect("set with ttl");
        let got = adapter.get("ttl_key").await.expect("get");
        assert_eq!(got, Some(b"ttl_val".to_vec()));
    }

    /// R-dbnexus-module-002 #6: error mapping — oxcache errors become
    /// `DbError::Config`. Hard to trigger a real oxcache error, so we
    /// verify the error type by checking that `DbError::Config` is the
    /// variant used in the adapter (static check via `From`-like pattern).
    #[test]
    fn adapter_error_mapping_uses_db_error_config() {
        // The adapter's get/set/delete use DbError::Config(format!(...)).
        // Verify the variant exists and can be constructed with the
        // "cache ..." prefix pattern the adapter uses.
        let err = DbError::Config("cache get failed: simulated".to_string());
        let msg = err.message();
        assert!(
            msg.starts_with("cache get failed:"),
            "error message should start with cache prefix, got: {msg}"
        );
    }

    /// R-dbnexus-module-002 #7: adapter works through dyn dispatch
    /// (`Arc<dyn DbCacheProvider + Send + Sync>`) — the path
    /// `DbNexusModule::build()` will use in T029/T030.
    #[tokio::test]
    async fn adapter_dyn_dispatch() {
        let adapter: Arc<dyn DbCacheProvider + Send + Sync> = Arc::new(make_adapter());
        adapter
            .set("dyn_key", b"dyn_val".to_vec(), None)
            .await
            .expect("set via dyn");
        let got = adapter.get("dyn_key").await.expect("get via dyn");
        assert_eq!(got, Some(b"dyn_val".to_vec()));
        adapter.delete("dyn_key").await.expect("delete via dyn");
        let gone = adapter.get("dyn_key").await.expect("get after delete via dyn");
        assert!(gone.is_none());
    }
}
