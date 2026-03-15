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
//! ```

#![doc(html_root_url = "https://docs.rs/dbnexus/0.1")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![allow(dead_code)] // 允许未使用的公共 API 和预留功能

// ============================================================================
// 编译期数据库特性互斥检查
// ============================================================================

#[cfg(all(not(clippy), feature = "sqlite", feature = "postgres"))]
compile_error!("Cannot enable both 'sqlite' and 'postgres' features");

#[cfg(all(not(clippy), feature = "sqlite", feature = "mysql"))]
compile_error!("Cannot enable both 'sqlite' and 'mysql' features");

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

/// 审计日志模块
#[cfg(feature = "audit")]
pub mod audit;

/// 缓存模块（基于 moka）
#[cfg(feature = "cache")]
pub mod cache;

/// 缓存配置和类型导出
#[cfg(feature = "cache")]
pub use cache::{AsyncCache, CacheBuilder, CacheKey, create_cache, create_cache_with_ttl};
/// 配置管理模块
pub mod config;

/// 错误类型定义
pub use crate::config::DbResult;
pub use config::{
    ConfigError, DatabaseType, DbConfig, DbError, PoolConfig,
};

/// 统一错误模块（渐进式重构）
pub mod error;
pub use error::{AuditError, MigrationError};
pub use error::{ConfigResult as ConfigResultNew, DbResult as DbResultNew, PermissionResult, PoolResult};
pub use error::{DbError as DbErrorNew, PermissionError, PoolError};

/// 实体转换模块
pub mod entity;
// 生成的权限角色模块（由 build.rs 自动生成，内部使用）
mod generated_roles;
/// 全局索引模块
#[cfg(all(feature = "global-index", feature = "with-json"))]
pub mod global_index;

/// 全局索引类型导出
#[cfg(all(feature = "global-index", feature = "with-json"))]
pub use global_index::{GlobalIndex, IndexEntry, SyncEvent};
/// 健康检查和可观测性模块
///
/// 提供连接池健康检查、熔断器模式和自动恢复机制
#[cfg(feature = "health-check")]
pub mod health;
/// Metrics 收集模块
#[cfg(feature = "metrics")]
pub mod metrics;

/// MetricsCollector trait 接口
#[cfg(feature = "metrics")]
pub use metrics::MetricsCollectorTrait;
/// Migration 模块
#[cfg(feature = "migration")]
pub mod migration;
/// 权限控制模块
#[cfg(feature = "permission")]
pub mod permission;

/// 权限提供者 trait 接口和实现
#[cfg(feature = "permission")]
pub use permission::{MemoryPermissionProvider, PermissionProvider, PermissionProviderError, YamlPermissionProvider};
/// SQL 解析模块
///
/// 提供 SQL 语句解析和操作类型分类功能。
/// 该模块中的类型主要用于内部权限检查逻辑。
#[cfg(feature = "sql-parser")]
pub mod sql_parser;

/// PermissionAction 类型导出（当 permission 特性未启用时，从 sql_parser 导出）
#[cfg(all(feature = "sql-parser", not(feature = "permission")))]
pub use sql_parser::PermissionAction;

/// 基础权限类型（简单 RBAC）
///
/// 提供基于角色的表级权限控制，适合基本使用场景。
///
/// # 类型说明
///
/// - [`PermissionAction`] - 权限操作类型（Select, Insert, Update, Delete）
/// - [`TablePermission`] - 表级权限配置
/// - [`RolePolicy`] - 角色策略
/// - [`PermissionConfig`] - 权限配置
/// - [`PermissionContext`] - 权限检查上下文
#[cfg(feature = "permission")]
pub use permission::{PermissionAction, PermissionConfig, PermissionContext, RolePolicy, TablePermission};

/// 可插拔权限引擎模块
#[cfg(feature = "permission-engine")]
pub mod permission_engine;

/// 权限引擎类型导出
///
/// 提供高级权限引擎，支持多种权限提供者实现。
/// 当启用 `permission-engine` 特性时可用。
///
/// # 与基础权限类型的区别
///
/// - [`EnginePermissionAction`] 包含额外的 `All` 变体
/// - [`PermissionEngineContext`] 包含更丰富的上下文信息
/// - 提供 [`PolicyDecisionPoint`] 策略决策点
/// - 支持 [`YamlPermissionProvider`] 和 [`RbacPermissionProvider`]
///
/// # 使用建议
///
/// - 简单场景：使用基础权限类型（无需启用特性）
/// - 复杂场景：启用 `permission-engine` 特性并使用权限引擎
#[cfg(feature = "permission-engine")]
pub use permission_engine::{
    PermissionAction as EnginePermissionAction, PermissionContext as PermissionEngineContext, PermissionDecision,
    PermissionEngine, PermissionEngineConfig, PermissionProvider as EnginePermissionProvider, PermissionResource,
    PermissionRule, PermissionSubject, PolicyDecisionPoint, RbacPermissionProvider, Role,
    YamlPermissionProvider as EngineYamlPermissionProvider,
};
/// 连接池管理模块
pub mod pool;
/// 分片管理模块
#[cfg(feature = "sharding")]
pub mod sharding;
/// 分布式追踪模块
#[cfg(feature = "tracing")]
pub mod tracing;

// Sea-ORM 类型重导出（通过 entity 子模块访问）
// 注意：不再直接导出 sea_orm，用户应使用 dbnexus 提供的抽象层

pub use crate::pool::DbPool;
pub use crate::pool::Session;
pub use crate::pool::{ConnectionPool, DatabaseSession};

// DI-related exports
pub use crate::pool::DbPoolBuilder;
pub use crate::pool::DbPoolDependencies;

/// 过程宏重新导出
#[cfg(feature = "macros")]
pub mod macros;

/// 过程宏重新导出（兼容旧用法）
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
