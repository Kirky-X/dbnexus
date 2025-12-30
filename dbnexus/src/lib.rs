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
//! ```rust,ignore
//! use dbnexus::DbPool;
//!
//! #[derive(dbnexus::DbEntity)]
//! #[db_entity]
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

// ============================================================================
// 编译期数据库特性互斥检查
// ============================================================================

#[cfg(all(feature = "sqlite", feature = "postgres"))]
compile_error!("Cannot enable both 'sqlite' and 'postgres' features");

#[cfg(all(feature = "sqlite", feature = "mysql"))]
compile_error!("Cannot enable both 'sqlite' and 'mysql' features");

#[cfg(all(feature = "postgres", feature = "mysql"))]
compile_error!("Cannot enable both 'postgres' and 'mysql' features");

#[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
compile_error!("Must enable exactly one database feature: 'sqlite', 'postgres', or 'mysql'");

// ============================================================================
// 模块声明
// ============================================================================

/// 配置管理模块
pub mod config;
/// 实体转换模块
pub mod entity;
/// 生成的权限角色模块（由 build.rs 自动生成）
pub mod generated_roles;
/// Metrics 收集模块
#[cfg(feature = "metrics")]
pub mod metrics;
/// Migration 模块
#[cfg(feature = "migration")]
pub mod migration;
/// 权限控制模块
pub mod permission;
/// 连接池管理模块
pub mod pool;
/// 分布式追踪模块
#[cfg(feature = "tracing")]
pub mod tracing;
/// 分片管理模块
#[cfg(feature = "sharding")]
pub mod sharding;
/// 全局索引模块
#[cfg(feature = "global-index")]
pub mod global_index;
/// 缓存模块
#[cfg(feature = "cache")]
pub mod cache;
/// 审计日志模块
#[cfg(feature = "audit")]
pub mod audit;

/// 错误类型定义
pub use crate::config::DbResult;

/// Sea-ORM 类型重导出
pub use sea_orm as orm;

pub use crate::pool::DbPool;
pub use crate::pool::Session;

/// 过程宏重新导出
pub use dbnexus_macros::DbEntity;
pub use dbnexus_macros::db_crud;
pub use dbnexus_macros::db_entity;
pub use dbnexus_macros::db_permission;
pub use dbnexus_macros::db_cache;
pub use dbnexus_macros::db_audit;
pub use dbnexus_macros::primary_key;
pub use dbnexus_macros::table_name;
