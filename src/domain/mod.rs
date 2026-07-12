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

// Re-exports
pub use cache_provider::DbCacheProvider;

#[cfg(all(feature = "permission", feature = "cache"))]
pub use permission::with_cache;
#[cfg(feature = "permission")]
pub use permission::{
    DefaultPolicy, PermissionAction, PermissionChecker, PermissionConfig, PermissionConfigError, PermissionError,
    PermissionLifecycle, PermissionProvider, PolicyManager, PolicySet, RolePolicy, TablePermission, new, new_in_memory,
};

#[cfg(feature = "audit")]
pub use audit::{
    AuditConfig, AuditContext, AuditEvent, AuditEventBuilder, AuditLogger, AuditOperation, AuditQueryFilters,
    AuditSeverity, AuditStatus, AuditStorage, BuildError, MemoryAuditStorage,
};

#[cfg(feature = "migration")]
pub use migration::{
    Column, ColumnType, Index, Migration, MigrationExecutor, MigrationFile, MigrationFileParser, MigrationHistory,
    MigrationVersion, Schema, SchemaDiffer, SqlGenerator, SqlReverser, Table, TableChange,
};
pub use migration::{ColumnChange, ColumnChangeType, ColumnDefinition, MigrationMetadata, TableSnapshot};
