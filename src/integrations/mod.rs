// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Integration adapters for external crates.
//!
//! Each submodule adapts a third-party library's API to a dbnexus domain
//! trait. Adapters live in dbnexus (the consumer crate), not in the
//! provider crate — this avoids circular dependencies (see `design.md`
//! Decision 2).

#[cfg(feature = "oxcache-integration")]
pub mod oxcache_adapter;

#[cfg(feature = "kit")]
pub mod kit;

// Re-exports
#[cfg(feature = "oxcache-integration")]
pub use oxcache_adapter::OxcacheDbCacheAdapter;

#[cfg(feature = "kit")]
pub use kit::{DbNexusBuildObserver, DbNexusModule};
