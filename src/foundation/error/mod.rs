// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 子错误类型模块
//!
//! 定义 DBNexus 项目中各功能模块的子错误类型。
//! 顶层统一错误类型 `DbNexusError` 定义在 [`crate::common::error`] 模块。
//!
//! # 错误类型
//!
//! - [`DbError`] - 数据库操作错误
//! - [`PoolError`] - 连接池错误
//! - [`PermissionError`] - 权限错误
//! - [`MigrationError`] - 迁移错误
//! - [`AuditError`] - 审计错误
//!
//! 每个子错误类型都有对应的结果类型别名（如 [`DbResult`]、[`PoolResult`] 等）。

// ============================================================================
// 子错误类型定义
// ============================================================================

/// 数据库操作错误
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// 数据库连接错误
    #[error(transparent)]
    Connection(#[from] sea_orm::DbErr),

    /// 配置错误
    #[error("Configuration error: {0}")]
    Config(String),

    /// 权限错误
    #[error("Permission denied: {0}")]
    Permission(String),

    /// 事务错误
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// 迁移错误
    #[error("Migration error: {0}")]
    Migration(String),

    /// 数据验证错误（feature-gated: validation）
    #[cfg(feature = "validation")]
    #[error("Validation error: {0}")]
    Validation(String),
}

impl DbError {
    /// 从 sea_orm::DbErr 创建数据库连接错误
    pub fn new(error: sea_orm::DbErr) -> Self {
        Self::Connection(error)
    }

    /// 获取错误消息
    pub fn message(&self) -> String {
        match self {
            DbError::Connection(e) => e.to_string(),
            DbError::Config(msg) => msg.clone(),
            DbError::Permission(msg) => msg.clone(),
            DbError::Transaction(msg) => msg.clone(),
            DbError::Migration(msg) => msg.clone(),
            #[cfg(feature = "validation")]
            DbError::Validation(msg) => msg.clone(),
        }
    }
}

/// 从字符串创建 DbError::Config
impl From<String> for DbError {
    fn from(msg: String) -> Self {
        Self::Config(msg)
    }
}

/// 从 &str 创建 DbError::Config
impl From<&str> for DbError {
    fn from(msg: &str) -> Self {
        Self::Config(msg.to_string())
    }
}

/// 连接池错误
#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    /// 连接获取超时
    #[error("Failed to acquire connection within timeout")]
    AcquireTimeout,

    /// 连接池已耗尽
    #[error("Connection pool exhausted")]
    PoolExhausted,

    /// 连接创建失败
    #[error("Failed to create connection: {0}")]
    ConnectionFailed(String),

    /// 健康检查失败
    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),
}

// 权限错误类型已统一到领域层。
// 此处 re-export `domain::permission::error::PermissionError`，保持
// `foundation::error::PermissionError` 路径可用，避免破坏既有引用路径。
//
// BREAKING（Task 13）：旧版 foundation 定义被删除，统一到 domain 版本：
// - `InvalidConfig` 变体重命名为 `InvalidPolicy`
// - 新增独有变体 `ParseError`
// - 错误消息大小写变化（如 "Role not found" → "role not found"）
// access::permission::types::PermissionError 保留不动，待 Phase 4 Task 18 整体删除。
#[cfg(feature = "permission")]
pub use crate::domain::permission::PermissionError;

/// 迁移错误
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// 迁移文件未找到
    #[error("Migration file not found: {0}")]
    FileNotFound(String),

    /// 迁移文件解析错误
    #[error("Failed to parse migration file: {0}")]
    ParseError(String),

    /// 迁移执行失败
    #[error("Migration execution failed: {0}")]
    ExecutionError(String),

    /// 迁移版本冲突
    #[error("Migration version conflict: {0}")]
    VersionConflict(String),

    /// 迁移回滚失败
    #[error("Migration rollback failed: {0}")]
    RollbackError(String),
}

/// 审计错误
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// 审计日志写入失败
    #[error("Failed to write audit log: {0}")]
    WriteError(String),

    /// 审计日志序列化失败
    #[error("Failed to serialize audit data: {0}")]
    SerializationError(String),

    /// 审计配置错误
    #[error("Invalid audit configuration: {0}")]
    ConfigError(String),
}

// ============================================================================
// 结果类型别名
// ============================================================================

/// 数据库操作结果
pub type DbResult<T> = Result<T, DbError>;
/// 权限检查结果
#[cfg(feature = "permission")]
pub type PermissionResult<T> = Result<T, PermissionError>;
/// 连接池操作结果
pub type PoolResult<T> = Result<T, PoolError>;
/// 配置操作结果
pub type ConfigResult<T> = Result<T, crate::foundation::config::ConfigError>;
/// 迁移操作结果
pub type MigrationResult<T> = Result<T, MigrationError>;
/// 审计操作结果
pub type AuditResult<T> = Result<T, AuditError>;

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 DbError 创建和转换
    #[test]
    fn test_db_error_creation() {
        let db_err = sea_orm::DbErr::Custom("test error".to_string());
        let error = DbError::new(db_err);
        assert!(matches!(error, DbError::Connection(_)));
    }

    /// 测试从 String 创建 DbError
    #[test]
    fn test_db_error_from_string() {
        let error: DbError = "custom error message".into();
        assert!(matches!(error, DbError::Config(msg) if msg == "custom error message"));
    }

    /// 测试从 &str 创建 DbError
    #[test]
    fn test_db_error_from_str() {
        let error: DbError = "str error".into();
        assert!(matches!(error, DbError::Config(msg) if msg == "str error"));
    }

    /// 测试 DbError 各变体
    #[test]
    fn test_db_error_variants() {
        let config_err = DbError::Config("config issue".to_string());
        assert!(matches!(config_err, DbError::Config(_)));

        let perm_err = DbError::Permission("access denied".to_string());
        assert!(matches!(perm_err, DbError::Permission(_)));

        let tx_err = DbError::Transaction("tx failed".to_string());
        assert!(matches!(tx_err, DbError::Transaction(_)));

        let mig_err = DbError::Migration("migration failed".to_string());
        assert!(matches!(mig_err, DbError::Migration(_)));
    }

    /// 测试 PoolError 显示
    #[test]
    fn test_pool_error_display() {
        let error = PoolError::AcquireTimeout;
        assert_eq!(error.to_string(), "Failed to acquire connection within timeout");

        let error = PoolError::ConnectionFailed("network issue".to_string());
        assert!(error.to_string().contains("network issue"));
    }

    /// 测试 PermissionError 显示
    #[test]
    #[cfg(feature = "permission")]
    fn test_permission_error_display() {
        let error = PermissionError::Denied {
            resource: "users".to_string(),
            operation: "delete".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("users"));
        assert!(msg.contains("delete"));

        let error = PermissionError::RoleNotFound("admin".to_string());
        assert!(error.to_string().contains("admin"));
    }

    /// 测试 MigrationError 显示
    #[test]
    fn test_migration_error_display() {
        let error = MigrationError::FileNotFound("v001.sql".to_string());
        assert!(error.to_string().contains("v001.sql"));

        let error = MigrationError::VersionConflict("v002".to_string());
        assert!(error.to_string().contains("v002"));
    }

    /// 测试 AuditError 显示
    #[test]
    fn test_audit_error_display() {
        let error = AuditError::WriteError("disk full".to_string());
        assert!(error.to_string().contains("disk full"));

        let error = AuditError::SerializationError("invalid JSON".to_string());
        assert!(error.to_string().contains("invalid JSON"));
    }
}
