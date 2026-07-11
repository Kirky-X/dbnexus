// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Domain 层 - 领域模块
//!
//! 提供业务领域相关的能力，可依赖 foundation 层和第三方库

pub mod cache_provider;

#[cfg(feature = "permission")]
pub mod permission;

pub mod migration;

#[cfg(feature = "audit")]
pub mod audit;
