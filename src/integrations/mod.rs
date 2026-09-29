// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Integration adapters for external crates.
//!
//! Each submodule adapts a third-party library's API to a dbnexus domain
//! trait. Adapters live in dbnexus (the consumer crate), not in the
//! provider crate — this avoids circular dependencies (see `design.md`
//! Decision 2).

#[cfg(feature = "oxcache-integration")]
pub mod oxcache_adapter;

/// 查询缓存装饰器（oxcache 后端 × DbPool 参数化查询，命中/失效/穿透）
#[cfg(feature = "oxcache-integration")]
pub mod oxcache_query_cache;

#[cfg(feature = "kit")]
pub mod kit;

#[cfg(feature = "inklog")]
pub mod inklog;

/// HTTP 健康端点生成器（axum Router 三端点：liveness/readiness/Prometheus）
#[cfg(feature = "http-health")]
pub mod http_health;

// Re-exports
#[cfg(feature = "oxcache-integration")]
pub use oxcache_adapter::OxcacheDbCacheAdapter;

#[cfg(feature = "oxcache-integration")]
pub use oxcache_query_cache::{CachedQuery, OxcacheQueryCache};

#[cfg(feature = "kit")]
pub use kit::{DbNexusBuildObserver, DbNexusModule};

#[cfg(feature = "inklog")]
pub use inklog::{InklogInit, init_inklog_logger, init_inklog_logger_with_config};

#[cfg(feature = "http-health")]
pub use http_health::HealthRouterBuilder;
