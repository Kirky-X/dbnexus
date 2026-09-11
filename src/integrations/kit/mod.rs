// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! trait-kit 0.4 `AsyncKit` integration for dbnexus.
//!
//! Contains [`DbNexusModule`] — a module that constructs a `DbPool` via the
//! `AsyncKit` dependency injection framework, depending on `OxcacheModule`
//! for cache capability.
//!
//! Also provides [`DbNexusBuildObserver`] — a `BuildObserver` implementation
//! for monitoring kit build pipeline events.

pub mod module;

pub use module::{DbNexusBuildObserver, DbNexusCacheModule, DbNexusModule};
// T413：全能力注册 — 审计/健康能力模块（按各自 feature 门控）
#[cfg(all(feature = "audit", feature = "sql-parser"))]
pub use module::DbNexusAuditModule;
#[cfg(feature = "health-check")]
pub use module::{DbHealthCapability, DbNexusHealthModule};
