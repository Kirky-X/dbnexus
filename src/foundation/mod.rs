// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Foundation 模块
//!
//! 提供错误类型、配置定义、类型重导出等基础设施

pub mod config;
pub mod entity;
pub mod error;

// Re-exports for convenience
pub use config::{ConfigError, DatabaseType, DbConfig, PoolConfig};
pub use entity::{ActiveModelTrait, Condition, EntityTrait, Set};
pub use error::{AuditError, DbError, DbResult, MigrationError};
pub use error::{ConfigResult, PermissionResult, PoolResult};
pub use error::{DbError as DbErrorNew, PermissionError, PoolError};
