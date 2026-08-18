// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DBNexus - 企业级数据库抽象层
//!
//! 基于积木架构的分层模块设计：
//! - **Foundation 层**: 零依赖基础模块 (pool)
//! - **Domain 层**: 领域模块 (permission, migration, audit, auth)
//! - **Observability 层**: 可观测模块 (metrics, health)

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![doc(html_root_url = "https://docs.rs/dbnexus/0.4.3")]

// ============================================================================
// 编译期数据库特性互斥检查
// ============================================================================

// 规则 1：postgres 与 mysql 互斥（当未启用嵌入式数据库时，服务器端数据库不能同时启用）
#[cfg(all(
    not(clippy),
    feature = "postgres",
    feature = "mysql",
    not(any(feature = "sqlite", feature = "duckdb"))
))]
compile_error!("Cannot enable both 'postgres' and 'mysql' features");

// 规则 2：嵌入式（sqlite/duckdb）与服务器端（postgres/mysql）严格互斥，无逃生门。
// 任何混合（含 --all-features 全开场景）一律编译失败——多后端 crate 不适用
// --all-features 验证（Cargo 无 feature 互斥语法），消费方按驱动分组启用 features。
#[cfg(all(
    not(clippy),
    any(feature = "sqlite", feature = "duckdb"),
    any(feature = "postgres", feature = "mysql")
))]
compile_error!("Cannot mix embedded (sqlite/duckdb) and server-side (postgres/mysql) database features");

// 规则 3：至少一个数据库后端（关系型或图 DB）
// 注意：不使用 compile_error! 以便 cargo publish 能验证 default = [] 的包。
// 用户启用任一数据库 feature 后，下方模块才会提供实际功能。
// 图 DB feature（ladybug/neo4j）与关系型 feature 不互斥，允许混合使用

// 检查 feature 依赖关系
#[cfg(all(not(clippy), feature = "permission-engine", not(feature = "cache")))]
compile_error!("The 'permission-engine' feature requires the 'cache' feature to be enabled");

#[cfg(all(not(clippy), feature = "sql-parser", not(feature = "cache")))]
compile_error!("The 'sql-parser' feature requires the 'cache' feature to be enabled");

// permission feature 需要 cache（Cache::builder 在 db_pool 中使用）
#[cfg(all(not(clippy), feature = "permission", not(feature = "cache")))]
compile_error!("The 'permission' feature requires the 'cache' feature to be enabled");

// ============================================================================
// 模块声明
// ============================================================================

/// 公共类型模块
mod common;

/// 统一错误类型模块
mod error;

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

/// Reliability 层 - 运行时容错模块（重试、故障转移等）
#[cfg(feature = "retry")]
pub mod reliability;

/// Storage 模块 (保留用于 global-index)
pub mod storage;

/// Internationalization 模块 - ICU4X + Fluent locale 感知格式化（核心特性，始终可用）
pub mod i18n;

/// Integration adapters for external crates (oxcache, etc.)
#[cfg(feature = "oxcache-integration")]
pub mod integrations;

// 生成的权限角色模块
mod generated_roles;

// ============================================================================
// 公共 API 导出
// ============================================================================

// Common 导出 (新架构)
pub use crate::error::{DbNexusError, DbNexusResult, ErrorCategory, QueryErrorReport};

// DatabaseType 统一在 foundation::config
pub use crate::foundation::DatabaseType;

// Foundation 导出 (旧版，保持兼容)
pub use crate::foundation::DbError;
pub use crate::foundation::DbResult;
#[cfg(feature = "failover")]
pub use crate::foundation::FailoverConfig;
#[cfg(feature = "permission")]
pub use crate::foundation::PermissionResult;
#[cfg(feature = "replica-routing")]
pub use crate::foundation::ReplicaConfig;
pub use crate::foundation::{ActiveModelTrait, Condition, EntityTrait, Set};
pub use crate::foundation::{AuditError, MigrationError, MigrationResult};
pub use crate::foundation::{CacheConfig, ConfigError, DbConfig, PoolConfig};
pub use crate::foundation::{ConfigResult, PoolResult};

// Domain cache provider 抽象导出（无 feature gate，domain 层核心 trait）
pub use crate::domain::DbCacheProvider;

// Domain Permission 导出 (新架构)
#[cfg(feature = "permission")]
pub use crate::domain::{
    PermissionAction as DomainPermissionAction, PermissionConfig as NewPermissionConfig, PermissionConfigError,
    PermissionError as NewPermissionError, PermissionProvider, RolePolicy as DomainRolePolicy,
    TablePermission as DomainTablePermission,
};

// Database 导出
pub use crate::database::DbConnection;
pub use crate::database::DbPool;
pub use crate::database::DbPoolBuilder;
pub use crate::database::Session;
#[cfg(feature = "migration")]
pub use crate::database::{
    Column, ColumnType, Index, Migration, MigrationExecutor, MigrationFile, MigrationFileParser, MigrationHistory,
    MigrationVersion, Schema, SchemaDiffer, SqlGenerator, Table, TableChange,
};
pub use crate::database::{ConnectionPool, DatabaseSession};

// DuckDB 连接包装器导出（0.3.0 新增）
#[cfg(feature = "sharding")]
pub use crate::database::{ConsistentHashStrategy, ShardConfig, ShardRouter, ShardingStrategy, create_strategy};
#[cfg(feature = "duckdb")]
pub use crate::database::{DuckDbConnection, DuckDbExecResult, DuckDbRow};

// 图数据库连接导出（0.4.0 新增）
#[cfg(feature = "ladybug")]
pub use crate::database::LadybugConnection;
#[cfg(feature = "neo4j")]
pub use crate::database::Neo4jConnection;
pub use crate::database::{
    GraphConnection, GraphExecResult, GraphNode, GraphQueryResult, GraphRel, GraphRow, GraphTransaction, GraphValue,
};

// Access 导出
#[cfg(feature = "sql-parser")]
pub use crate::access::{DdlGuard, DdlValidationResult};
pub use crate::access::{MaskType, SensitiveError, SensitiveMasker, SensitiveResult};

#[cfg(all(feature = "permission", any(feature = "ladybug", feature = "neo4j")))]
pub use crate::access::GraphPermissionContext;
#[cfg(feature = "permission")]
pub use crate::access::{
    MemoryPermissionProvider, PermissionAction as AccessPermissionAction, PermissionCache, PermissionCacheConfig,
    PermissionConfig, PermissionContext, PermissionProvider as AccessPermissionProvider, PermissionProviderError,
    RolePolicy as AccessRolePolicy, TablePermission as AccessTablePermission, YamlPermissionProvider,
};

#[cfg(feature = "authentication")]
pub use crate::access::{
    AuthCredentials, AuthError, AuthResult, AuthenticationManager, JwtClaims, JwtManager, PasswordHasher, TokenType,
    User,
};

#[cfg(feature = "sql-parser")]
pub use crate::access::SqlParser;

#[cfg(feature = "sql-parser")]
pub use crate::access::SqlOperationType;

#[cfg(feature = "sql-parser")]
pub use crate::access::is_ddl_operation;

#[cfg(feature = "sql-parser")]
pub use crate::access::contains_sql_injection;

#[cfg(feature = "permission-engine")]
#[cfg(feature = "permission")]
pub use crate::access::permission_engine::{
    PermissionAction as EnginePermissionAction, PermissionContext as PermissionEngineContext, PermissionDecision,
    PermissionProvider as EnginePermissionProvider, PermissionResource, PermissionRule, PermissionSubject,
    PolicyDecisionPoint, PolicyDecisionPointConfig, RbacPermissionProvider, Role,
    YamlPermissionProvider as EngineYamlPermissionProvider,
};

// Observability 导出
#[cfg(feature = "health-check")]
pub use crate::observability::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerState, HealthChecker, HealthStatus,
    PoolHealthMetrics,
};
#[cfg(feature = "metrics")]
pub use crate::observability::{
    ConnectionAcquireStats, HistogramBucket, HistogramStats, LatencyHistogram, LatencyPercentiles, MetricsCollector,
    MetricsCollectorTrait, MetricsError, PoolMetrics, QueryStats, SlowQueryConfig, SlowQueryRecord, ThroughputStats,
    TransactionStats,
};

// Reliability 导出（retry feature）
#[cfg(feature = "retry")]
pub use crate::reliability::{RetryError, RetryExecutor, RetryPolicy, is_idempotent_operation};

// Replica routing 导出（replica-routing feature）
#[cfg(feature = "replica-routing")]
pub use crate::database::replica::MySqlLagDetector;
#[cfg(feature = "replica-routing")]
pub use crate::database::replica::PostgresLagDetector;
#[cfg(feature = "replica-routing")]
pub use crate::database::replica::{ReplicaPool, ReplicationLag, ReplicationLagDetector, SqliteLagDetector};

// Scatter-Gather 导出（scatter-gather feature）
#[cfg(feature = "scatter-gather")]
pub use crate::database::{
    AggregateFunction, AggregateValue, PartialFailurePolicy, ScatterGatherExecutor, ScatterResult, ShardError,
};

// Saga 分布式事务导出（saga feature）
#[cfg(feature = "saga")]
pub use crate::database::{
    InMemorySagaLog, SagaAction, SagaError, SagaExecutionResult, SagaLog, SagaOrchestrator, SagaStatus, SagaStep,
    SagaStepLog,
};

// 分片迁移编排导出（shard-migration feature）
#[cfg(feature = "shard-migration")]
pub use crate::domain::migration::{OrchestratedMigrationResult, ShardMigrationOrchestrator, ShardMigrationResult};

// 分布式 ID 生成器导出（distributed-id feature）
#[cfg(feature = "distributed-id")]
pub use crate::common::{DistributedIdGenerator, IdComponents, SnowflakeError, SnowflakeIdGenerator};

// MockMetrics 仅在测试或启用 `test-utils` feature 时导出（BREAKING: 从默认公共 API 移除）
#[cfg(all(feature = "metrics", any(test, feature = "test-utils")))]
pub use crate::observability::MockMetrics;

// Storage 导出
#[cfg(feature = "global-index")]
pub use crate::storage::{
    GlobalIndex, IndexEntry, SYNC_STATUS_FAILED, SYNC_STATUS_PENDING, SYNC_STATUS_SYNCED, SyncEvent, SyncResult,
};

// Business 导出（直接从 domain::audit 导出，移除 business 中间层）
#[cfg(feature = "audit")]
pub use crate::domain::{
    AuditConfig, AuditContext, AuditEvent, AuditEventBuilder, AuditLogger, AuditOperation, AuditQueryFilters,
    AuditSeverity, AuditStatus, AuditStorage, MemoryAuditStorage,
};
#[cfg(feature = "audit")]
pub use crate::foundation::AuditResult;

// 过程宏 — 导出 `db_entity` 和 `db_repository`
#[cfg(feature = "macros")]
pub use dbnexus_macros::db_entity;
#[cfg(feature = "macros")]
pub use dbnexus_macros::db_repository;

// Integration adapter 导出（oxcache-integration feature）
#[cfg(feature = "oxcache-integration")]
pub use crate::integrations::OxcacheDbCacheAdapter;

// DbNexusModule 导出（kit feature — trait-kit 0.4 AsyncKit integration）
#[cfg(feature = "kit")]
pub use crate::integrations::{DbNexusBuildObserver, DbNexusModule};

// I18n 导出（核心特性 — ICU4X 国际化格式化 + 全局 locale 管理）
pub use crate::i18n::catalog::{t, t_simple, translate, translate_en};
pub use crate::i18n::error_ext::{I18nExt, LocalizedMsg};
pub use crate::i18n::locale::{clear_locale_override, current_locale, detected_locale, set_locale};
pub use crate::i18n::{DbI18nFormatter, I18nError};

// ============================================================================
// Re-export underlying dependencies
// ============================================================================
// Scope: only type references (L1) — e.g. `use dbnexus::sea_orm::ActiveValue`,
// `use dbnexus::chrono::Utc`, `use dbnexus::tokio::sync::Mutex`,
// `use dbnexus::async_trait::async_trait`. Macro attributes that expand to
// canonical crate paths (L2, e.g. `#[tokio::main]` expands to
// `tokio::runtime::...`, `#[derive(serde::Serialize)]` expands to
// `impl serde::Serialize`) and macro invocations (L3, e.g.
// `sea_orm::entity::prelude!()`) reference absolute crate paths at expansion
// time and cannot be routed through a re-export alias; downstream crates must
// still declare direct dependencies for those uses. Note: `#[async_trait]` is
// an attribute macro whose expansion does NOT reference `async_trait::` paths,
// so it can be used via the re-export (`use dbnexus::async_trait::async_trait`).
// This re-export narrows the direct-dependency surface to the macro path only;
// it does not eliminate it.
//
// Each re-export is feature-gated to the feature set that actually pulls in
// the underlying dependency.

// sea_orm is a non-optional dependency but is re-exported only under database
// driver features to keep the API surface minimal in non-sea-orm builds.
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql", feature = "duckdb"))]
pub use sea_orm;

// async_trait is a non-optional dependency; always available.
pub use async_trait;

// tokio is a non-optional core dependency; always available.
pub use tokio;

// chrono is an optional dependency; re-export only when a feature that pulls
// it in is enabled (sharding / global-index / audit / with-chrono).
#[cfg(any(
    feature = "sharding",
    feature = "global-index",
    feature = "audit",
    feature = "with-chrono"
))]
pub use chrono;
