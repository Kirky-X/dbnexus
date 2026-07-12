// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Foundation 模块
//!
//! 提供错误类型、配置定义、类型重导出等基础设施

// 旧版模块（保持兼容）
pub mod config;
pub mod error;

// Re-exports for convenience (旧版，保持兼容)
pub use config::{CacheConfig, ConfigError, DatabaseType, DbConfig, PoolConfig};
#[cfg(feature = "permission")]
pub use error::PermissionError;
#[cfg(feature = "permission")]
pub use error::PermissionResult;
pub use error::{AuditError, AuditResult, DbError, DbResult, MigrationError, MigrationResult};
pub use error::{ConfigResult, PoolError, PoolResult};
pub use sea_orm::entity::prelude::{ActiveModelTrait, EntityTrait};
pub use sea_orm::{Condition, Set};
