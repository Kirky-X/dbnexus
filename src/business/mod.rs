// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Business 模块（重导出）
//!
//! 实际实现在 [`crate::domain::audit`]

// 重导出 domain 层的审计模块
#[cfg(feature = "audit")]
pub use crate::domain::audit::{
    AuditConfig, AuditContext, AuditEvent, AuditEventBuilder, AuditLogger, AuditLoggerBuilder, AuditOperation,
    AuditQueryFilters, AuditSeverity, AuditStatus, AuditStorage, MemoryAuditStorage,
};
