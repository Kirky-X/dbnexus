// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Business 模块
//!
//! 提供业务相关功能

// 单文件模块
#[cfg(feature = "audit")]
pub mod audit;

// Re-exports
#[cfg(feature = "audit")]
pub use audit::{AuditConfig, AuditContext, AuditEvent, AuditLogger, AuditLoggerBuilder};
