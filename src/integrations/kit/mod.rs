// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! trait-kit 0.2.2 `AsyncKit` integration for dbnexus.
//!
//! Contains [`DbNexusModule`] — a module that constructs a `DbPool` via the
//! `AsyncKit` dependency injection framework, depending on `OxcacheModule`
//! for cache capability.

pub mod module;

pub use module::DbNexusModule;
