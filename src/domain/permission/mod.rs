// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 权限领域模块
//!
//! 提供基于角色的权限控制能力

mod config;
mod error;
mod impl_;
mod interface;
mod types;

mod permission_impl;

pub use config::{DefaultPolicy, PermissionConfig};
pub use error::{PermissionConfigError, PermissionError};
pub use interface::{PermissionChecker, PermissionLifecycle, PermissionProvider, PolicyManager};
pub use types::{PermissionAction, PolicySet, RolePolicy, TablePermission};

#[cfg(feature = "cache")]
pub use permission_impl::with_cache;
pub use permission_impl::{new, new_in_memory};
