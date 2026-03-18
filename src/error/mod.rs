// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 统一错误类型模块
//!
//! 定义 DBNexus 项目中所有错误类型的统一接口。
//!
//! # 错误处理策略
//!
//! DBNexus 采用分层错误处理架构：
//!
//! ## 错误层次结构
//!
//! ```text
//! DbNexusError (顶层统一错误)
//! ├── Database(DbError)      - 数据库操作错误
//! ├── Pool(PoolError)        - 连接池错误
//! ├── Permission(PermissionError) - 权限错误
//! ├── Config(ConfigError)    - 配置错误
//! ├── Migration(MigrationError) - 迁移错误
//! └── Audit(AuditError)      - 审计错误
//! ```
//!
//! ## 设计原则
//!
//! 1. **统一入口**: 所有公开 API 使用 `DbNexusResult<T>` 作为返回类型
//! 2. **错误转换**: 通过 `From` trait 实现子错误到顶层错误的自动转换
//! 3. **错误链**: 使用 `#[error(transparent)]` 保留原始错误信息
//! 4. **类型安全**: 每种错误类型都有明确的语义和上下文信息
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use dbnexus::error::{DbNexusError, DbNexusResult};
//!
//! async fn example() -> DbNexusResult<()> {
//!     // 子错误会自动转换为 DbNexusError
//!     // 无需手动 map_err
//!     Ok(())
//! }
//! ```

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
// 顶层统一错误类型
// ============================================================================

/// DBNexus 顶层统一错误类型
///
/// 这是 DBNexus 项目中所有公开 API 的统一错误返回类型。
/// 通过 `#[error(transparent)]` 透明地传递子错误，保留完整的错误链。
///
/// # 错误转换
///
/// 所有子错误类型都实现了 `From<SubError> for DbNexusError`，
/// 因此可以使用 `?` 运算符进行自动转换。
///
/// # 示例
///
/// ```rust,ignore
/// use dbnexus::error::{DbNexusError, DbNexusResult, DbError};
///
/// fn might_fail() -> DbNexusResult<()> {
///     // DbError 自动转换为 DbNexusError::Database
///     let result: Result<(), DbError> = Err(DbError::from("something went wrong"));
///     result?;
///     Ok(())
/// }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum DbNexusError {
    /// 数据库操作错误
    #[error(transparent)]
    Database(#[from] DbError),

    /// 连接池错误
    #[error(transparent)]
    Pool(#[from] PoolError),

    /// 权限错误
    #[error(transparent)]
    Permission(#[from] PermissionError),

    /// 配置错误
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),

    /// 迁移错误
    #[error(transparent)]
    Migration(#[from] MigrationError),

    /// 审计错误
    #[error(transparent)]
    Audit(#[from] AuditError),
}

// ============================================================================
// 额外的 From 实现
// ============================================================================

/// 从 sea_orm::DbErr 转换到 DbNexusError
impl From<sea_orm::DbErr> for DbNexusError {
    fn from(err: sea_orm::DbErr) -> Self {
        DbNexusError::Database(DbError::new(err))
    }
}

/// 从字符串创建 DbNexusError
impl From<String> for DbNexusError {
    fn from(msg: String) -> Self {
        DbNexusError::Database(DbError::from(msg))
    }
}

/// 从 &str 创建 DbNexusError
impl From<&str> for DbNexusError {
    fn from(msg: &str) -> Self {
        DbNexusError::Database(DbError::from(msg))
    }
}

// ============================================================================
// 结果类型别名
// ============================================================================

/// DBNexus 统一结果类型
///
/// 这是所有 DBNexus 公开 API 的标准返回类型。
/// 使用此类型别名可以：
/// - 简化函数签名
/// - 统一错误处理
/// - 支持 `?` 运算符的自动错误转换
///
/// # 示例
///
/// ```rust,ignore
/// use dbnexus::error::DbNexusResult;
///
/// async fn connect() -> DbNexusResult<Connection> {
///     // 任何子错误都会自动转换为 DbNexusError
///     Ok(connection)
/// }
/// ```
pub type DbNexusResult<T> = Result<T, DbNexusError>;

/// 数据库操作结果
pub type DbResult<T> = Result<T, DbError>;
/// 权限检查结果
pub type PermissionResult<T> = Result<T, PermissionError>;
/// 连接池操作结果
pub type PoolResult<T> = Result<T, PoolError>;
/// 配置操作结果
pub type ConfigResult<T> = Result<T, crate::config::ConfigError>;
/// 迁移操作结果
pub type MigrationResult<T> = Result<T, MigrationError>;
/// 审计操作结果
pub type AuditResult<T> = Result<T, AuditError>;

// ============================================================================
// 错误辅助函数
// ============================================================================

/// 安全地格式化错误消息
///
/// 避免在错误消息中暴露敏感信息
#[cfg(feature = "regex")]
pub fn safe_error_message(error: &str) -> String {
    // 使用 regex 移除可能的敏感信息
    let sensitive_patterns = [
        r"(?i)(password|passwd|pwd)[=:]\S+",
        r"(?i)(api_key|apikey|secret|token)[=:]\S+",
        r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b",
    ];

    let mut result = error.to_string();
    for pattern in &sensitive_patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            result = re.replace_all(&result, "[REDACTED]").to_string();
        }
    }

    result
}

/// 安全地格式化错误消息（无 regex 版本）
#[cfg(not(feature = "regex"))]
pub fn safe_error_message(error: &str) -> String {
    // 简单处理：检查常见敏感关键词
    let lower = error.to_lowercase();
    if lower.contains("password") || lower.contains("api_key") || lower.contains("secret") {
        "[REDACTED]".to_string()
    } else {
        error.to_string()
    }
}

/// 获取错误的根本原因
pub fn root_cause<E: std::error::Error>(error: &E) -> &dyn std::error::Error {
    let mut current = error as &dyn std::error::Error;
    while let Some(source) = current.source() {
        current = source;
    }
    current
}

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

    /// 测试 DbNexusError 从 DbError 转换
    #[test]
    fn test_dbnexus_error_from_db_error() {
        let db_error: DbError = "database error".into();
        let nexus_error: DbNexusError = db_error.into();

        match nexus_error {
            DbNexusError::Database(_) => (),
            _ => panic!("Expected Database variant"),
        }
    }

    /// 测试 DbNexusError 从 PoolError 转换
    #[test]
    fn test_dbnexus_error_from_pool_error() {
        let pool_error = PoolError::PoolExhausted;
        let nexus_error: DbNexusError = pool_error.into();

        match nexus_error {
            DbNexusError::Pool(_) => (),
            _ => panic!("Expected Pool variant"),
        }
    }

    /// 测试 DbNexusError 从 PermissionError 转换
    #[test]
    fn test_dbnexus_error_from_permission_error() {
        let perm_error = PermissionError::RateLimited;
        let nexus_error: DbNexusError = perm_error.into();

        match nexus_error {
            DbNexusError::Permission(_) => (),
            _ => panic!("Expected Permission variant"),
        }
    }

    /// 测试 DbNexusError 从 MigrationError 转换
    #[test]
    fn test_dbnexus_error_from_migration_error() {
        let mig_error = MigrationError::ExecutionError("failed".to_string());
        let nexus_error: DbNexusError = mig_error.into();

        match nexus_error {
            DbNexusError::Migration(_) => (),
            _ => panic!("Expected Migration variant"),
        }
    }

    /// 测试 DbNexusError 从 AuditError 转换
    #[test]
    fn test_dbnexus_error_from_audit_error() {
        let audit_error = AuditError::ConfigError("invalid config".to_string());
        let nexus_error: DbNexusError = audit_error.into();

        match nexus_error {
            DbNexusError::Audit(_) => (),
            _ => panic!("Expected Audit variant"),
        }
    }

    /// 测试 DbNexusError 从 sea_orm::DbErr 直接转换
    #[test]
    fn test_dbnexus_error_from_db_err() {
        let db_err = sea_orm::DbErr::Custom("direct error".to_string());
        let nexus_error: DbNexusError = db_err.into();

        match nexus_error {
            DbNexusError::Database(_) => (),
            _ => panic!("Expected Database variant"),
        }
    }

    /// 测试 DbNexusError 从 String 转换
    #[test]
    fn test_dbnexus_error_from_string() {
        let nexus_error: DbNexusError = "string error".to_string().into();

        match nexus_error {
            DbNexusError::Database(_) => (),
            _ => panic!("Expected Database variant"),
        }
    }

    /// 测试 DbNexusError 从 &str 转换
    #[test]
    fn test_dbnexus_error_from_str() {
        let nexus_error: DbNexusError = "str error".into();

        match nexus_error {
            DbNexusError::Database(_) => (),
            _ => panic!("Expected Database variant"),
        }
    }

    /// 测试 DbNexusResult 类型别名
    #[test]
    fn test_dbnexus_result_ok() {
        fn returns_ok() -> DbNexusResult<i32> {
            Ok(42)
        }

        assert_eq!(returns_ok().unwrap(), 42);
    }

    /// 测试 DbNexusResult 错误转换
    #[test]
    fn test_dbnexus_result_error_conversion() {
        fn returns_db_error() -> DbNexusResult<()> {
            let err: DbError = "test error".into();
            Err(err)?
        }

        let result = returns_db_error();
        assert!(result.is_err());

        match result.unwrap_err() {
            DbNexusError::Database(_) => (),
            _ => panic!("Expected Database variant"),
        }
    }

    /// 测试错误链传播
    #[test]
    fn test_error_chain_propagation() {
        fn inner() -> Result<(), PoolError> {
            Err(PoolError::AcquireTimeout)
        }

        fn outer() -> DbNexusResult<()> {
            inner()?;
            Ok(())
        }

        let result = outer();
        assert!(result.is_err());

        match result.unwrap_err() {
            DbNexusError::Pool(e) => {
                assert!(matches!(e, PoolError::AcquireTimeout));
            }
            _ => panic!("Expected Pool variant"),
        }
    }

    /// 测试 safe_error_message 函数
    #[test]
    fn test_safe_error_message() {
        let safe = safe_error_message("Connection failed");
        assert_eq!(safe, "Connection failed");

        // 测试敏感信息过滤
        let sensitive = safe_error_message("password=secret123");
        // 根据 regex feature 是否启用，结果可能不同
        #[cfg(feature = "regex")]
        assert!(sensitive.contains("[REDACTED]") || !sensitive.contains("secret123"));
    }

    /// 测试 root_cause 函数
    #[test]
    fn test_root_cause() {
        let error = DbError::from("inner error");
        let cause = root_cause(&error);
        assert!(cause.to_string().contains("inner error"));
    }
}
