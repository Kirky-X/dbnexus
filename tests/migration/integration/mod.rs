// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Migration 模块集成测试

#[cfg(feature = "migration")]
pub mod auto_migrate;

#[cfg(feature = "migration")]
#[allow(clippy::module_inception)]
pub mod integration;
