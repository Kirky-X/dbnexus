// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in project root for full license information.

//! 统一错误类型

use std::fmt;
use thiserror::Error;

/// DBNexus 顶层统一错误类型
#[derive(Debug, Error)]
pub enum DbNexusError {
    /// 权限错误
    #[cfg(feature = "permission")]
    #[error(transparent)]
    Permission(#[from] crate::domain::permission::PermissionError),

    /// 权限配置错误
    #[cfg(feature = "permission")]
    #[error(transparent)]
    PermissionConfig(#[from] crate::domain::permission::PermissionConfigError),

    /// 不支持的数据库协议
    #[error("Unsupported database scheme in URL: {0}")]
    UnsupportedDatabaseScheme(String),
}

/// DBNexus 统一结果类型
pub type DbNexusResult<T> = Result<T, DbNexusError>;

// ============================================================================
// 结构化查询错误报告（v0.3.0 新增）
// ============================================================================

/// 查询错误类别
///
/// 对数据库查询执行期间可能出现的错误进行分类，
/// 便于上层应用根据类别采取不同的处理策略（如重试、降级、上报）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// 权限被拒绝（角色缺少对资源的操作权限）
    Permission,
    /// SQL 注入风险（检测到可疑的注入模式）
    InjectionRisk,
    /// SQL 语法错误或解析失败
    SyntaxError,
    /// 分片冲突（跨分片查询未通过路由约束）
    ShardConflict,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Permission => write!(f, "Permission"),
            Self::InjectionRisk => write!(f, "InjectionRisk"),
            Self::SyntaxError => write!(f, "SyntaxError"),
            Self::ShardConflict => write!(f, "ShardConflict"),
        }
    }
}

/// 结构化查询错误报告
///
/// 提供比裸错误更丰富的上下文：类别、消息、修复建议、涉及的表与操作。
/// 可由上层（如 `Session::execute`）在检测到注入、权限拒绝或分片冲突时直接构造，
/// 也可通过 `From<DbNexusError>` 从顶层错误自动转换。
///
/// # 示例
///
/// ```rust,no_run
/// use dbnexus::{ErrorCategory, QueryErrorReport};
///
/// let report = QueryErrorReport::new(
///     ErrorCategory::InjectionRisk,
///     "SQL contains UNION-based injection pattern",
///     "Use parameterized queries instead of string concatenation",
/// )
/// .with_table("users")
/// .with_operation("SELECT");
///
/// assert_eq!(report.to_string(), "[InjectionRisk] SQL contains UNION-based injection pattern\nSuggestion: Use parameterized queries instead of string concatenation\nTable: users\nOperation: SELECT");
/// ```
#[derive(Debug, Clone)]
pub struct QueryErrorReport {
    /// 错误类别
    pub category: ErrorCategory,
    /// 错误消息
    pub message: String,
    /// 修复建议
    pub suggestion: String,
    /// 涉及的表名（可选）
    pub table: Option<String>,
    /// 涉及的 SQL 操作（可选）
    pub operation: Option<String>,
}

impl QueryErrorReport {
    /// 创建新的查询错误报告
    pub fn new(
        category: ErrorCategory,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            category,
            message: message.into(),
            suggestion: suggestion.into(),
            table: None,
            operation: None,
        }
    }

    /// 链式设置涉及的表名
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// 链式设置涉及的 SQL 操作
    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }
}

impl fmt::Display for QueryErrorReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}\nSuggestion: {}",
            self.category, self.message, self.suggestion
        )?;
        if let Some(table) = &self.table {
            write!(f, "\nTable: {}", table)?;
        }
        if let Some(operation) = &self.operation {
            write!(f, "\nOperation: {}", operation)?;
        }
        Ok(())
    }
}

impl std::error::Error for QueryErrorReport {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

/// 从 `DbNexusError` 自动推断 `ErrorCategory` 并构造报告
impl From<DbNexusError> for QueryErrorReport {
    fn from(err: DbNexusError) -> Self {
        match err {
            #[cfg(feature = "permission")]
            DbNexusError::Permission(_) => QueryErrorReport::new(
                ErrorCategory::Permission,
                err.to_string(),
                "Verify the role has the required permissions on the target table",
            ),
            #[cfg(feature = "permission")]
            DbNexusError::PermissionConfig(_) => QueryErrorReport::new(
                ErrorCategory::Permission,
                err.to_string(),
                "Check the permission policy configuration file for syntax or schema errors",
            ),
            DbNexusError::UnsupportedDatabaseScheme(_) => QueryErrorReport::new(
                ErrorCategory::SyntaxError,
                err.to_string(),
                "Use a supported database scheme: sqlite, postgres, mysql, or duckdb",
            ),
        }
    }
}
