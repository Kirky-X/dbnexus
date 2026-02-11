// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DBNexus procedural macros re-export module
//!
//! This module provides a convenient way to access all DBNexus procedural macros
//! under a single namespace.

#[cfg(feature = "macros")]
pub use dbnexus_macros::DbEntity;

#[cfg(feature = "macros")]
pub use dbnexus_macros::db_audit;

#[cfg(feature = "macros")]
pub use dbnexus_macros::db_cache;

#[cfg(feature = "macros")]
pub use dbnexus_macros::db_crud;

#[cfg(feature = "macros")]
pub use dbnexus_macros::db_permission;
