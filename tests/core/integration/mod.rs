// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Core 模块集成测试

pub mod config_integration;
pub mod dbnexus_integration;
pub mod entity_integration;

#[cfg(feature = "health-check")]
pub mod health_integration;

#[cfg(feature = "permission")]
pub mod permission_integration;
pub mod session_transaction;
#[cfg(feature = "sql-parser")]
pub mod sql_parser_integration;
