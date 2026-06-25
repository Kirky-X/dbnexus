// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Migration 模块集成测试

#[cfg(feature = "migration")]
pub mod auto_migrate;

#[cfg(feature = "migration")]
#[allow(clippy::module_inception)]
pub mod integration;
