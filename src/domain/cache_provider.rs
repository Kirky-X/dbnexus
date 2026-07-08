// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Cache provider abstraction for dbnexus.
//!
//! Defines the [`DbCacheProvider`] trait — the minimal cache interface
//! dbnexus consumes internally. The trait uses explicit
//! `Pin<Box<dyn Future + Send>>` return types (rather than `async fn` in
//! trait) to remain object-safe (`dyn DbCacheProvider` compiles) without
//! pulling in the `async-trait` crate. This mirrors the pattern used by
//! `trait_kit::AsyncAutoBuilder` (see `trait-kit/src/core/meta.rs`).
//!
//! # Design (Rule 7: expose, don't paper over)
//!
//! `design.md` Decision 2 (lines 196-207) and `spec.md` R-dbnexus-module-001
//! specify `DbError::cache(message)` for error mapping. The actual
//! `DbError` enum (`src/foundation/error/mod.rs`) has **no `cache` variant**
//! — the closest semantic match is `DbError::Config(String)`, which we use
//! for cache-related errors surfaced by `DbCacheProvider` implementations.
//! This divergence is documented here rather than silently papered over;
//! a future change can add a dedicated `DbError::Cache(String)` variant if
//! the codebase grows enough cache-related error paths to warrant it.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::foundation::error::DbError;

/// Cache provider abstraction consumed by dbnexus internals.
///
/// Defines the minimal get/set/delete interface for a byte-oriented cache.
/// Implementations include [`OxcacheDbCacheAdapter`](crate::integrations::oxcache_adapter::OxcacheDbCacheAdapter)
/// (feature-gated behind `oxcache-integration`) and any custom adapter the
/// user supplies.
///
/// # Object safety
///
/// The trait is dyn-compatible: `Arc<dyn DbCacheProvider + Send + Sync>`
/// compiles. Methods return `Pin<Box<dyn Future + Send + 'a>>` (lifetime-tied
/// to `&self`) rather than using `async fn` in trait, because `dyn`-compatible
/// dispatch requires the explicit `Pin<Box>` indirection (Rust 1.91 supports
/// `async fn` in trait but not `dyn` with native async methods).
///
/// # Rule 7 divergence from `design.md`
///
/// `design.md` wrote `DbError::cache(e.to_string())` for error mapping.
/// `DbError` has no `cache` variant — we use `DbError::Config(String)` as
/// the closest semantic match for cache-related configuration/operational
/// errors. See the module-level docs for the full rationale.
pub trait DbCacheProvider: Send + Sync {
    /// Retrieve a byte value from the cache by key.
    ///
    /// Returns `Ok(None)` if the key is absent (not an error condition).
    /// Returns `Err(DbError::Config(..))` if the underlying cache backend
    /// reports an operational failure.
    #[allow(
        clippy::type_complexity,
        reason = "Pin<Box<dyn Future + Send>> is the canonical dyn-compatible async trait dispatch type"
    )]
    fn get<'a>(&'a self, key: &'a str) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, DbError>> + Send + 'a>>;

    /// Store a byte value in the cache with an optional TTL.
    ///
    /// If `ttl` is `None`, the implementation applies its default expiry
    /// policy. Returns `Err(DbError::Config(..))` on backend failure.
    #[allow(
        clippy::type_complexity,
        reason = "Pin<Box<dyn Future + Send>> is the canonical dyn-compatible async trait dispatch type"
    )]
    fn set<'a>(
        &'a self,
        key: &'a str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>>;

    /// Delete a key from the cache. Idempotent — deleting an absent key
    /// returns `Ok(())`.
    #[allow(
        clippy::type_complexity,
        reason = "Pin<Box<dyn Future + Send>> is the canonical dyn-compatible async trait dispatch type"
    )]
    fn delete<'a>(&'a self, key: &'a str) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// In-memory `DbCacheProvider` mock for verifying the trait contract.
    ///
    /// Uses `Mutex<HashMap<String, Vec<u8>>>` so the mock is `Send + Sync`
    /// (the trait requires both). A real adapter like
    /// `OxcacheDbCacheAdapter` would wrap an `Arc<dyn CacheBackend>` and
    /// forward calls to the oxcache backend.
    struct MockCacheProvider {
        inner: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl MockCacheProvider {
        fn new() -> Self {
            Self {
                inner: Mutex::new(HashMap::new()),
            }
        }
    }

    impl DbCacheProvider for MockCacheProvider {
        fn get<'a>(
            &'a self,
            key: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, DbError>> + Send + 'a>> {
            Box::pin(async move {
                let map = self.inner.lock().expect("mock cache lock poisoned");
                Ok(map.get(key).cloned())
            })
        }

        fn set<'a>(
            &'a self,
            key: &'a str,
            value: Vec<u8>,
            _ttl: Option<Duration>,
        ) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
            Box::pin(async move {
                let mut map = self.inner.lock().expect("mock cache lock poisoned");
                map.insert(key.to_string(), value);
                Ok(())
            })
        }

        fn delete<'a>(
            &'a self,
            key: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), DbError>> + Send + 'a>> {
            Box::pin(async move {
                let mut map = self.inner.lock().expect("mock cache lock poisoned");
                map.remove(key);
                Ok(())
            })
        }
    }

    /// R-dbnexus-module-001 #1: trait is object-safe —
    /// `Arc<dyn DbCacheProvider + Send + Sync>` compiles and dispatches.
    #[test]
    fn db_cache_provider_is_object_safe() {
        fn assert_dyn_compatible(_: Arc<dyn DbCacheProvider + Send + Sync>) {}
        let mock = Arc::new(MockCacheProvider::new());
        assert_dyn_compatible(mock);
    }

    /// R-dbnexus-module-001 #2: `get` returns `Pin<Box<dyn Future + Send>>`
    /// — verified by calling `.await` on the returned future inside a
    /// tokio runtime.
    #[tokio::test]
    async fn db_cache_provider_get_returns_pin_box_future() {
        let cache = MockCacheProvider::new();
        // set a value first
        cache.set("k", b"v".to_vec(), None).await.expect("set");
        // get returns Pin<Box<dyn Future + Send>> — awaiting it yields the value
        let got = cache.get("k").await.expect("get should succeed");
        assert_eq!(got, Some(b"v".to_vec()));
    }

    /// R-dbnexus-module-001 #3: `get` on absent key returns `Ok(None)`.
    #[tokio::test]
    async fn db_cache_provider_get_absent_key_returns_none() {
        let cache = MockCacheProvider::new();
        let got = cache.get("missing").await.expect("get should succeed");
        assert!(got.is_none(), "absent key must return Ok(None)");
    }

    /// R-dbnexus-module-001 #4: `set` stores the value and it's
    /// retrievable by `get`.
    #[tokio::test]
    async fn db_cache_provider_set_stores_value() {
        let cache = MockCacheProvider::new();
        cache
            .set("key1", vec![1, 2, 3], Some(Duration::from_secs(60)))
            .await
            .expect("set");
        let got = cache.get("key1").await.expect("get");
        assert_eq!(got, Some(vec![1, 2, 3]));
    }

    /// R-dbnexus-module-001 #5: `delete` removes the value — subsequent
    /// `get` returns `Ok(None)`. Delete is idempotent.
    #[tokio::test]
    async fn db_cache_provider_delete_removes_value() {
        let cache = MockCacheProvider::new();
        cache.set("doomed", b"x".to_vec(), None).await.expect("set");
        cache.delete("doomed").await.expect("delete");
        let got = cache.get("doomed").await.expect("get after delete");
        assert!(got.is_none(), "deleted key must return Ok(None)");

        // Idempotent: deleting an absent key does not error.
        cache.delete("doomed").await.expect("idempotent delete");
    }

    /// R-dbnexus-module-001 #6: `set` overwrites a prior value for the
    /// same key.
    #[tokio::test]
    async fn db_cache_provider_set_overwrites() {
        let cache = MockCacheProvider::new();
        cache.set("k", b"old".to_vec(), None).await.expect("set old");
        cache.set("k", b"new".to_vec(), None).await.expect("set new");
        let got = cache.get("k").await.expect("get");
        assert_eq!(got, Some(b"new".to_vec()));
    }

    /// R-dbnexus-module-001 #7: trait can be used through a trait object
    /// (`Arc<dyn DbCacheProvider + Send + Sync>`) — the dyn dispatch path
    /// that `DbNexusModule::build` will use in T029/T030.
    #[tokio::test]
    async fn db_cache_provider_dyn_dispatch_works() {
        let cache: Arc<dyn DbCacheProvider + Send + Sync> =
            Arc::new(MockCacheProvider::new());
        cache.set("dyn", b"works".to_vec(), None).await.expect("set");
        let got = cache.get("dyn").await.expect("get");
        assert_eq!(got, Some(b"works".to_vec()));
        cache.delete("dyn").await.expect("delete");
        let gone = cache.get("dyn").await.expect("get after delete");
        assert!(gone.is_none());
    }
}
