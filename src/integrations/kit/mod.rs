// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! trait-kit 0.2.2 `AsyncKit` integration for dbnexus.
//!
//! Contains [`DbNexusModule`] — a module that constructs a `DbPool` via the
//! `AsyncKit` dependency injection framework, depending on `OxcacheModule`
//! for cache capability.

pub mod module;

pub use module::DbNexusModule;
