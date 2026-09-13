// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Kit feature gate compilation + behavior test.
//!
//! Verifies that `--features kit` pulls in `trait-kit/observer` and the
//! `DbNexusBuildObserver` is usable alongside `AsyncKit::with_observer`.

use std::sync::Arc;

use dbnexus::foundation::{DbConfig, PoolConfig};
use dbnexus::integrations::kit::{DbNexusBuildObserver, DbNexusModule};
use oxcache::integrations::kit::OxcacheConfig;
use trait_kit::{AsyncKit, BuildObserver};

/// `DbNexusBuildObserver` implements `BuildObserver` (pulled in via
/// `trait-kit/observer` in the `kit` feature).
#[test]
fn kit_feature_observer_is_available() {
    fn assert_trait<T: BuildObserver>() {}
    assert_trait::<DbNexusBuildObserver>();
}

/// Full build pipeline with observer counts module builds.
#[tokio::test]
async fn kit_feature_full_build_with_observer() {
    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig::default());
    kit.set_config(DbConfig {
        url: "sqlite::memory:".to_string(),
        pool_config: PoolConfig {
            max_connections: 2,
            min_connections: 1,
            ..Default::default()
        },
        ..Default::default()
    });

    kit.register::<oxcache::integrations::kit::OxcacheModule>()
        .expect("register OxcacheModule");
    kit.register::<DbNexusModule>()
        .expect("register DbNexusModule");

    let observer = Arc::new(DbNexusBuildObserver::new());
    kit.with_observer(observer.clone());

    let kit = kit.build().await.expect("build should succeed");

    // Observer should have counted at least 2 successful builds.
    assert!(
        observer.built_count() >= 2,
        "expected >= 2 built modules, got {}",
        observer.built_count()
    );

    kit.shutdown_async().await;
}
