// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Core 模块集成测试

pub mod config_integration;
pub mod entity_integration;

#[cfg(feature = "health-check")]
pub mod health_integration;

pub mod permission_integration;
pub mod session_transaction;
pub mod sql_parser_integration;
