// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DB Nexus - 企业级数据库抽象层
//!
//! 基于 Sea-ORM 的高性能、高安全性 Rust 数据库访问层
//!
//! # 功能特性
//!
//! - **Session 机制**: RAII 自动管理数据库连接生命周期
//! - **权限控制**: 声明式宏自动生成权限检查代码
//! - **连接池管理**: 动态配置修正与健康检查
//! - **监控指标**: Prometheus 指标导出
//!
//! # 快速开始
//!
//! ```rust,no_run,ignore
//! use dbnexus::DbPool;
//!
//! #[derive(dbnexus::DbEntity)]
//! #[table_name = "users"]
//! struct User {
//!     #[primary_key]
//!     id: i64,
//!     name: String,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let pool = DbPool::new("postgresql://user:pass@localhost/db").await?;
//!     let session = pool.get_session("admin").await?;
//!     Ok(())
//! }
//! # 监控指标
//!
//! DBNexus 提供 Prometheus 指标导出功能。

#![deny(missing_docs)]
#![forbid(unsafe_code)]
// 注意: 移除了 `#![allow(dead_code)]` 以恢复编译时警告
// 未使用的代码应该显式标记或在具体位置添加 `#[allow(dead_code)]`

// ============================================================================
// 编译期数据库特性互斥检查
// ============================================================================

// 临时禁用以支持多数据库测试环境
// #[cfg(all(not(clippy), feature = "sqlite", feature = "postgres"))]
// compile_error!("Cannot enable both 'sqlite' and 'postgres' features");

// #[cfg(all(not(clippy), feature = "sqlite", feature = "mysql"))]
// compile_error!("Cannot enable both 'sqlite' and 'mysql' features");

#[cfg(all(not(clippy), feature = "postgres", feature = "mysql"))]
compile_error!("Cannot enable both 'postgres' and 'mysql' features");

#[cfg(all(not(clippy), not(any(feature = "sqlite", feature = "postgres", feature = "mysql"))))]
compile_error!("Must enable exactly one database feature: 'sqlite', 'postgres', or 'mysql'");

// 检查 feature 依赖关系
#[cfg(all(not(clippy), feature = "permission-with-cache", not(feature = "cache")))]
compile_error!("The 'permission-with-cache' feature requires the 'cache' feature to be enabled");

#[cfg(all(not(clippy), feature = "permission-engine", not(feature = "cache")))]
compile_error!("The 'permission-engine' feature requires the 'cache' feature to be enabled");

#[cfg(all(not(clippy), feature = "sql-parser", not(feature = "cache")))]
compile_error!("The 'sql-parser' feature requires the 'cache' feature to be enabled");

// ============================================================================
// 模块声明
// ============================================================================

/// Foundation 模块 - 错误类型、配置、类型定义
pub mod foundation;

/// Database 模块 - 连接池、迁移、分片
pub mod database;

/// Access 模块 - 安全、权限、认证
pub mod access;

/// Observability 模块 - 健康检查、指标、追踪
pub mod observability;

/// Storage 模块 - 缓存、全局索引
pub mod storage;

/// Business 模块 - 审计等业务功能
pub mod business;

/// Tools 模块 - 过程宏、CLI
pub mod tools;

// 生成的权限角色模块（由 build.rs 自动生成，内部使用）
mod generated_roles;

// ============================================================================
// 公共 API 导出（保持向后兼容）
// ============================================================================

// Foundation 导出
pub use crate::foundation::config::{CacheConfig, ConfigError, DatabaseType, DbConfig, PoolConfig};
pub use crate::foundation::entity::{ActiveModelTrait, Condition, EntityTrait, Set};
pub use crate::foundation::error::DbError;
pub use crate::foundation::error::DbResult;
pub use crate::foundation::error::{AuditError, MigrationError, MigrationResult};
pub use crate::foundation::error::{ConfigResult, PermissionResult, PoolResult};
pub use crate::foundation::error::{DbError as DbErrorNew, PermissionError, PoolError};

// Database 导出
#[cfg(feature = "migration")]
pub use crate::database::migration::{
    Column, ColumnType, Index, Migration, MigrationExecutor, MigrationFile, MigrationFileParser, MigrationHistory,
    MigrationVersion, Schema, SchemaDiffer, SqlGenerator, Table, TableChange,
};
pub use crate::database::pool::DbPool;
pub use crate::database::pool::DbPoolBuilder;
pub use crate::database::pool::DbPoolDependencies;
pub use crate::database::pool::Session;
pub use crate::database::pool::{ConnectionPool, DatabaseSession};
#[cfg(feature = "sharding")]
pub use crate::database::sharding::{ShardConfig, ShardRouter, ShardingStrategy};

// Access 导出
pub use crate::access::security::{DdlGuard, DdlValidationResult};
pub use crate::access::security::{MaskType, SensitiveError, SensitiveMasker, SensitiveResult};

#[cfg(feature = "permission")]
#[cfg(not(feature = "permission-engine"))]
pub use crate::access::permission::{
    MemoryPermissionProvider, PermissionAction, PermissionConfig, PermissionContext, PermissionProvider,
    PermissionProviderError, RolePolicy, TablePermission, YamlPermissionProvider,
};

#[cfg(all(feature = "permission", feature = "permission-engine"))]
pub use crate::access::permission::{
    MemoryPermissionProvider, PermissionAction, PermissionConfig, PermissionProvider, PermissionProviderError,
    RolePolicy, TablePermission, YamlPermissionProvider,
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
// Access Control 导出
#[cfg(feature = "permission")]
pub use crate::access::permission_engine::{
    PermissionAction as EnginePermissionAction, PermissionContext as PermissionEngineContext, PermissionDecision,
    PermissionEngine, PermissionEngineConfig, PermissionProvider as EnginePermissionProvider, PermissionResource,
    PermissionRule, PermissionSubject, PolicyDecisionPoint, RbacPermissionProvider, Role,
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
    MetricsCollectorTrait, MetricsError, PoolMetrics, QueryStats, SlowQueryConfig, SlowQueryRecord, ThroughputStats,
    TransactionStats,
};

// Storage 导出
#[cfg(feature = "cache")]
pub use crate::storage::cache::{CacheBackend, CacheError, CacheKey, CacheResult, OxcacheBackend};

#[cfg(all(feature = "global-index", feature = "with-json"))]
pub use crate::storage::global_index::{
    GlobalIndex, IndexEntry, SYNC_STATUS_FAILED, SYNC_STATUS_PENDING, SYNC_STATUS_SYNCED, SyncEvent, SyncResult,
};

// Business 导出
#[cfg(feature = "audit")]
pub use crate::business::audit::{
    AuditConfig, AuditContext, AuditEvent, AuditEventBuilder, AuditLogger, AuditLoggerBuilder, AuditOperation,
    AuditQueryFilters, AuditSeverity, AuditStatus, AuditStorage, MemoryAuditStorage,
};
// AuditResult 类型别名
#[cfg(feature = "audit")]
pub use crate::foundation::error::AuditResult;

// Tools 导出
#[cfg(feature = "macros")]
pub use dbnexus_macros::DbEntity;
#[cfg(feature = "macros")]
pub use dbnexus_macros::db_audit;
#[cfg(feature = "macros")]
pub use dbnexus_macros::db_cache;
#[cfg(feature = "macros")]
pub use dbnexus_macros::db_crud;
#[cfg(feature = "macros")]
pub use dbnexus_macros::db_permission;
