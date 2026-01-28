// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 统一错误类型模块
//!
//! 定义 DBNexus 项目中所有错误类型的统一接口。

/// 数据库操作错误
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct DbError(#[from] sea_orm::DbErr);

impl DbError {
    /// 创建新的数据库错误
    pub fn new(error: sea_orm::DbErr) -> Self {
        Self(error)
    }

    /// 获取内部错误引用
    pub fn inner(&self) -> &sea_orm::DbErr {
        &self.0
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

/// 权限错误
#[derive(Debug, thiserror::Error)]
pub enum PermissionError {
    /// 权限被拒绝
    #[error("Permission denied for {operation} on {resource}")]
    Denied {
        /// 目标资源
        resource: String,
        /// 操作类型
        operation: String,
    },

    /// 角色未找到
    #[error("Role not found: {0}")]
    RoleNotFound(String),

    /// 无效的权限配置
    #[error("Invalid permission configuration: {0}")]
    InvalidConfig(String),

    /// 速率限制
    #[error("Rate limit exceeded")]
    RateLimited,
}

/// 结果类型别名
pub type DbResult<T> = Result<T, DbError>;
/// 权限检查结果
pub type PermissionResult<T> = Result<T, PermissionError>;
/// 连接池操作结果
pub type PoolResult<T> = Result<T, PoolError>;
/// 配置操作结果
pub type ConfigResult<T> = Result<T, crate::config::ConfigError>;

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

/// 结果类型别名
pub type MigrationResult<T> = Result<T, MigrationError>;
/// 审计操作结果
pub type AuditResult<T> = Result<T, AuditError>;
