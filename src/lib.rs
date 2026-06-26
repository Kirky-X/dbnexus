// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DBNexus - 企业级数据库抽象层
//!
//! 基于积木架构的分层模块设计：
//! - **Foundation 层**: 零依赖基础模块 (pool)
//! - **Domain 层**: 领域模块 (permission, migration, audit, auth)
//! - **Observability 层**: 可观测模块 (metrics, health)

#![deny(missing_docs)]
#![forbid(unsafe_code)]

// ============================================================================
// 编译期数据库特性互斥检查
// ============================================================================

#[cfg(all(not(clippy), feature = "postgres", feature = "mysql", not(feature = "sqlite")))]
compile_error!("Cannot enable both 'postgres' and 'mysql' features");

#[cfg(all(not(clippy), not(any(feature = "sqlite", feature = "postgres", feature = "mysql"))))]
compile_error!("Must enable exactly one database feature: 'sqlite', 'postgres', or 'mysql'");

// 检查 feature 依赖关系
// Task 21：移除 permission-with-cache 检查（该聚合 feature 已移除，用户改用 permission + cache）
#[cfg(all(not(clippy), feature = "permission-engine", not(feature = "cache")))]
compile_error!("The 'permission-engine' feature requires the 'cache' feature to be enabled");

#[cfg(all(not(clippy), feature = "sql-parser", not(feature = "cache")))]
compile_error!("The 'sql-parser' feature requires the 'cache' feature to be enabled");

// ============================================================================
// 模块声明
// ============================================================================

/// 公共类型模块
pub mod common;

/// Foundation 层 - 基础模块 (旧版，保持兼容)
pub mod foundation;

/// Domain 层 - 领域模块
pub mod domain;

/// Database 模块 - 连接池、迁移、分片
pub mod database;

/// Access 模块 - 安全、权限、认证
pub mod access;

/// Observability 层 - 可观测模块
pub mod observability;

/// Storage 模块 (保留用于 global-index)
pub mod storage;

/// Kit 模块 — 基于 `trait-kit` 的统一能力管理
pub mod kit;

// 生成的权限角色模块
mod generated_roles;

// ============================================================================
// 公共 API 导出
// ============================================================================

// Common 导出 (新架构)
pub use crate::common::error::{DbNexusError, DbNexusResult};

// DatabaseType 统一定一在 foundation::config（Task 15：合并 common::types::DatabaseType）
pub use crate::foundation::config::DatabaseType;

// Foundation 导出 (旧版，保持兼容)
pub use crate::foundation::config::{CacheConfig, ConfigError, DbConfig, PoolConfig};
pub use crate::foundation::{ActiveModelTrait, Condition, EntityTrait, Set};
pub use crate::foundation::error::DbError;
pub use crate::foundation::error::DbResult;
pub use crate::foundation::error::{AuditError, MigrationError, MigrationResult};
pub use crate::foundation::error::{ConfigResult, PermissionResult, PoolResult};

// Domain Permission 导出 (新架构)
#[cfg(feature = "permission")]
pub use crate::domain::permission::{
    PermissionAction as DomainPermissionAction, PermissionConfig as NewPermissionConfig, PermissionConfigError,
    PermissionError as NewPermissionError, PermissionProvider, RolePolicy as DomainRolePolicy,
    TablePermission as DomainTablePermission,
};

// Database 导出
#[cfg(feature = "migration")]
pub use crate::database::migration::{
    Column, ColumnType, Index, Migration, MigrationExecutor, MigrationFile, MigrationFileParser, MigrationHistory,
    MigrationVersion, Schema, SchemaDiffer, SqlGenerator, Table, TableChange,
};
pub use crate::database::pool::DbPool;
pub use crate::database::pool::DbPoolBuilder;
pub use crate::database::pool::Session;
pub use crate::database::pool::{ConnectionPool, DatabaseSession};
#[cfg(feature = "sharding")]
pub use crate::database::sharding::{ShardConfig, ShardRouter, ShardingStrategy};

// Access 导出
#[cfg(feature = "sql-parser")]
pub use crate::access::security::{DdlGuard, DdlValidationResult};
pub use crate::access::security::{MaskType, SensitiveError, SensitiveMasker, SensitiveResult};

#[cfg(feature = "permission")]
#[cfg(not(feature = "permission-engine"))]
pub use crate::access::permission::{
    MemoryPermissionProvider, PermissionAction as AccessPermissionAction, PermissionConfig, PermissionContext,
    PermissionProvider as AccessPermissionProvider, PermissionProviderError, RolePolicy as AccessRolePolicy,
    TablePermission as AccessTablePermission, YamlPermissionProvider,
};

#[cfg(all(feature = "permission", feature = "permission-engine"))]
pub use crate::access::permission::{
    MemoryPermissionProvider, PermissionAction as AccessPermissionAction, PermissionConfig,
    PermissionProvider as AccessPermissionProvider, PermissionProviderError, RolePolicy as AccessRolePolicy,
    TablePermission as AccessTablePermission, YamlPermissionProvider,
};

#[cfg(feature = "authentication")]
pub use crate::access::authentication::{
    AuthCredentials, AuthError, AuthResult, AuthenticationManager, JwtClaims, JwtManager, PasswordHasher, TokenType,
    User,
};

#[cfg(feature = "sql-parser")]
pub use crate::access::sql_parser::SqlParser;

#[cfg(feature = "sql-parser")]
pub use crate::access::sql_parser::SqlOperationType;

#[cfg(feature = "sql-parser")]
pub use crate::access::sql_parser::is_ddl_operation;

#[cfg(feature = "sql-parser")]
pub use crate::access::sql_parser::contains_sql_injection;

#[cfg(feature = "permission-engine")]
#[cfg(feature = "permission")]
pub use crate::access::permission_engine::{
    PermissionAction as EnginePermissionAction, PermissionContext as PermissionEngineContext, PermissionDecision,
    PermissionProvider as EnginePermissionProvider, PermissionResource,
    PermissionRule, PermissionSubject, PolicyDecisionPoint, PolicyDecisionPointConfig, RbacPermissionProvider, Role,
    YamlPermissionProvider as EngineYamlPermissionProvider,
};

// Observability 导出
#[cfg(feature = "health-check")]
pub use crate::observability::health::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerState, HealthChecker, HealthStatus,
    PoolHealthMetrics,
};
#[cfg(feature = "metrics")]
pub use crate::observability::metrics::{
    ConnectionAcquireStats, HistogramBucket, HistogramStats, LatencyHistogram, LatencyPercentiles, MetricsCollector,
    MetricsCollectorTrait, MetricsError, PoolMetrics, QueryStats, SlowQueryConfig, SlowQueryRecord,
    ThroughputStats, TransactionStats,
};

// MockMetrics 仅在测试或启用 `test-utils` feature 时导出（BREAKING: 从默认公共 API 移除）
#[cfg(all(feature = "metrics", any(test, feature = "test-utils")))]
pub use crate::observability::metrics::MockMetrics;

// Storage 导出
#[cfg(feature = "global-index")]
pub use crate::storage::global_index::{
    GlobalIndex, IndexEntry, SYNC_STATUS_FAILED, SYNC_STATUS_PENDING, SYNC_STATUS_SYNCED, SyncEvent, SyncResult,
};

// Business 导出（直接从 domain::audit 导出，移除 business 中间层）
#[cfg(feature = "audit")]
pub use crate::domain::audit::{
    AuditConfig, AuditContext, AuditEvent, AuditEventBuilder, AuditLogger, AuditOperation,
    AuditQueryFilters, AuditSeverity, AuditStatus, AuditStorage, MemoryAuditStorage,
};
#[cfg(feature = "audit")]
pub use crate::foundation::error::AuditResult;

// 过程宏 — 仅导出统一属性宏 `db_entity`（替代旧版 DbEntity/db_crud/db_permission/db_cache/db_audit）
#[cfg(feature = "macros")]
pub use dbnexus_macros::db_entity;

// Kit 导出
pub use crate::kit::{DbNexusKit, keys::*};
