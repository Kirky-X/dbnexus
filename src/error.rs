// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 统一错误类型

use std::fmt;
use thiserror::Error;

/// DBNexus 顶层统一错误类型
#[derive(Debug, Error)]
pub enum DbNexusError {
    /// 权限错误
    #[cfg(feature = "permission")]
    #[error(transparent)]
    Permission(#[from] crate::domain::PermissionError),

    /// 权限配置错误
    #[cfg(feature = "permission")]
    #[error(transparent)]
    PermissionConfig(#[from] crate::domain::PermissionConfigError),

    /// 不支持的数据库协议
    #[error("Unsupported database scheme in URL: {0}")]
    UnsupportedDatabaseScheme(String),
}

impl crate::i18n::error_ext::LocalizedMsg for DbNexusError {
    fn message_key(&self) -> &'static str {
        match self {
            #[cfg(feature = "permission")]
            Self::Permission(err) => err.message_key(),
            #[cfg(feature = "permission")]
            Self::PermissionConfig(err) => err.message_key(),
            Self::UnsupportedDatabaseScheme(_) => "nexus-unsupported-database",
        }
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        match self {
            #[cfg(feature = "permission")]
            Self::Permission(err) => err.message_args(),
            #[cfg(feature = "permission")]
            Self::PermissionConfig(err) => err.message_args(),
            Self::UnsupportedDatabaseScheme(scheme) => vec![("scheme", scheme.clone())],
        }
    }
}

/// DBNexus 统一结果类型
pub type DbNexusResult<T> = Result<T, DbNexusError>;

// ============================================================================
// 统一错误码表 + 顶层错误结构
// ============================================================================

/// 统一错误码表
///
/// 稳定数值码（供日志/监控/跨语言协议引用）与机器可读名；分段约定：
/// - `0` 未知；`1xxx` 连接/驱动；`2xxx` 权限；`3xxx` SQL 安全校验；
/// - `4xxx` 配置；`5xxx` 迁移；`6xxx` 查询/事务；`7xxx` 缓存/审计。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ErrorCode {
    /// 未知/未分类错误
    Unknown = 0,
    /// 连接或驱动层错误（1000）
    Connection = 1000,
    /// 权限被拒绝（2000）
    PermissionDenied = 2000,
    /// 权限配置错误（2001）
    PermissionConfig = 2001,
    /// 速率限制拒绝（2002；HTTP 429 语义）
    ///
    /// 错误分类（如 [`QueryErrorReport`]）将其归入 Permission 类：类别
    /// 粒度仅表达「访问控制域」，HTTP 层区分 429（限流）与 403（策略
    /// 拒绝）依赖本错误码——2002 为限流，2000/2001 为策略/配置拒绝。
    RateLimited = 2002,
    /// SQL 注入风险（3000）
    InjectionRisk = 3000,
    /// SQL 语法/解析错误（3001）
    SqlSyntax = 3001,
    /// 配置错误（4000）
    Config = 4000,
    /// 迁移错误（5000）
    Migration = 5000,
    /// 查询执行错误（6000）
    Query = 6000,
    /// 事务错误（6001）
    Transaction = 6001,
    /// 缓存操作错误（7000）
    Cache = 7000,
    /// 数据验证错误（7001）
    Validation = 7001,
}

impl ErrorCode {
    /// 稳定数值码
    pub fn code(self) -> i32 {
        self as i32
    }

    /// 机器可读名（与变体名一致）
    pub fn name(self) -> &'static str {
        match self {
            ErrorCode::Unknown => "Unknown",
            ErrorCode::Connection => "Connection",
            ErrorCode::PermissionDenied => "PermissionDenied",
            ErrorCode::PermissionConfig => "PermissionConfig",
            ErrorCode::RateLimited => "RateLimited",
            ErrorCode::InjectionRisk => "InjectionRisk",
            ErrorCode::SqlSyntax => "SqlSyntax",
            ErrorCode::Config => "Config",
            ErrorCode::Migration => "Migration",
            ErrorCode::Query => "Query",
            ErrorCode::Transaction => "Transaction",
            ErrorCode::Cache => "Cache",
            ErrorCode::Validation => "Validation",
        }
    }
}

/// 顶层统一错误结构
///
/// 携带 [`ErrorCode`]、消息与底层错误文本；保留与既有错误类型的
/// `From` 兼容层——`foundation::DbError` / `DbNexusError` 可无损转换
/// 进入本结构（反向转换为有损映射，见 `From<UnifiedDbError> for DbError`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedDbError {
    code: ErrorCode,
    message: String,
    /// 底层错误文本（兼容层无法持有非 'static 源，故保存文本）
    source_text: Option<String>,
}

impl UnifiedDbError {
    /// 构造统一错误
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source_text: None,
        }
    }

    /// 附加底层错误文本
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source_text = Some(source.into());
        self
    }

    /// 错误码
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// 错误消息
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 底层错误文本
    pub fn source_text(&self) -> Option<&str> {
        self.source_text.as_deref()
    }

    /// 机器可读 JSON 形态（供日志/监控/跨语言协议输出）
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.code.code(),
            "name": self.code.name(),
            "message": self.message,
            "source": self.source_text,
        })
    }
}

impl fmt::Display for UnifiedDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}:{}] {}",
            self.code.code(),
            self.code.name(),
            self.message
        )?;
        if let Some(source) = &self.source_text {
            write!(f, " (source: {source})")?;
        }
        Ok(())
    }
}

impl std::error::Error for UnifiedDbError {}

/// 兼容层：既有 `foundation::DbError` → 统一错误（码按变体分段映射）
impl From<crate::foundation::DbError> for UnifiedDbError {
    fn from(err: crate::foundation::DbError) -> Self {
        let code = match &err {
            crate::foundation::DbError::Connection(_) => ErrorCode::Connection,
            crate::foundation::DbError::Permission(_) => ErrorCode::PermissionDenied,
            crate::foundation::DbError::RateLimited { .. } => ErrorCode::RateLimited,
            crate::foundation::DbError::Transaction(_) => ErrorCode::Transaction,
            crate::foundation::DbError::Migration(_) => ErrorCode::Migration,
            crate::foundation::DbError::Cache(_) => ErrorCode::Cache,
            crate::foundation::DbError::Query(_) => ErrorCode::Query,
            #[cfg(feature = "validation")]
            crate::foundation::DbError::Validation(_) => ErrorCode::Validation,
            crate::foundation::DbError::Config(_) => ErrorCode::Config,
            crate::foundation::DbError::Unsupported(_) => ErrorCode::Query,
            crate::foundation::DbError::VersionConflict { .. } => ErrorCode::Query,
        };
        Self::new(code, err.to_string()).with_source(err.to_string())
    }
}

/// 兼容层：`DbNexusError` → 统一错误
impl From<DbNexusError> for UnifiedDbError {
    fn from(err: DbNexusError) -> Self {
        let code = match &err {
            #[cfg(feature = "permission")]
            // 权限拒绝（2000）与权限配置错误（2001）分段映射，与错误码表契约一致
            DbNexusError::Permission(_) => ErrorCode::PermissionDenied,
            #[cfg(feature = "permission")]
            DbNexusError::PermissionConfig(_) => ErrorCode::PermissionConfig,
            DbNexusError::UnsupportedDatabaseScheme(_) => ErrorCode::SqlSyntax,
        };
        Self::new(code, err.to_string())
    }
}

/// 兼容层：统一错误 → 既有 `foundation::DbError`（有损：码段映射回变体，
/// 原始细节保留在消息文本中）
impl From<UnifiedDbError> for crate::foundation::DbError {
    fn from(err: UnifiedDbError) -> Self {
        let text = match err.source_text {
            Some(source) => format!("{} ({source})", err.message),
            None => err.message,
        };
        match err.code {
            // Connection 变体持有 DbErr；文本经 Custom 包装保持类别不丢失
            ErrorCode::Connection => {
                crate::foundation::DbError::Connection(sea_orm::DbErr::Custom(text))
            }
            ErrorCode::PermissionDenied | ErrorCode::PermissionConfig => {
                crate::foundation::DbError::Permission(text)
            }
            // 反向为有损映射：Retry-After 细节保留在消息文本中
            ErrorCode::RateLimited => crate::foundation::DbError::RateLimited {
                retry_after_secs: None,
            },
            ErrorCode::Migration => crate::foundation::DbError::Migration(text),
            ErrorCode::Transaction => crate::foundation::DbError::Transaction(text),
            ErrorCode::Cache => crate::foundation::DbError::Cache(text),
            ErrorCode::InjectionRisk | ErrorCode::SqlSyntax | ErrorCode::Query => {
                crate::foundation::DbError::Query(text)
            }
            // validation feature 启用时映射回 Validation 变体；未启用时退化为 Query
            #[cfg(feature = "validation")]
            ErrorCode::Validation => crate::foundation::DbError::Validation(text),
            #[cfg(not(feature = "validation"))]
            ErrorCode::Validation => crate::foundation::DbError::Query(text),
            ErrorCode::Unknown | ErrorCode::Config => crate::foundation::DbError::Config(text),
        }
    }
}

/// 统一错误 → 查询错误报告（码段 → 类别）
impl From<UnifiedDbError> for QueryErrorReport {
    fn from(err: UnifiedDbError) -> Self {
        let category = match err.code {
            // RateLimited（2002）归 Permission 类：429/403 的区分靠错误码
            // 本身（见 ErrorCode::RateLimited 文档），类别仅表达访问控制域
            ErrorCode::PermissionDenied | ErrorCode::PermissionConfig | ErrorCode::RateLimited => {
                ErrorCategory::Permission
            }
            ErrorCode::InjectionRisk => ErrorCategory::InjectionRisk,
            ErrorCode::SqlSyntax => ErrorCategory::SyntaxError,
            _ => ErrorCategory::SyntaxError,
        };
        QueryErrorReport::new(
            category,
            err.to_string(),
            crate::i18n::t_simple("query-error-suggestion-code-table"),
        )
    }
}

impl crate::i18n::error_ext::LocalizedMsg for ErrorCode {
    fn message_key(&self) -> &'static str {
        "error-code"
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        vec![
            ("code", self.code().to_string()),
            ("name", self.name().to_string()),
        ]
    }
}

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
///
/// 结构化查询错误报告
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

impl crate::i18n::error_ext::LocalizedMsg for ErrorCategory {
    fn message_key(&self) -> &'static str {
        match self {
            Self::Permission => "error-category-permission",
            Self::InjectionRisk => "error-category-injection-risk",
            Self::SyntaxError => "error-category-syntax-error",
            Self::ShardConflict => "error-category-shard-conflict",
        }
    }
}

impl crate::i18n::error_ext::LocalizedMsg for QueryErrorReport {
    fn message_key(&self) -> &'static str {
        "query-error-report"
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        let mut args = vec![
            ("category", self.category.to_string()),
            ("message", self.message.clone()),
            ("suggestion", self.suggestion.clone()),
        ];
        if let Some(table) = &self.table {
            args.push(("table", table.clone()));
        }
        if let Some(operation) = &self.operation {
            args.push(("operation", operation.clone()));
        }
        args
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

#[cfg(test)]
mod error_code_tests {
    use super::*;

    /// 错误码表：数值分段与机器可读名稳定
    #[test]
    fn test_error_code_table() {
        assert_eq!(ErrorCode::Unknown.code(), 0);
        assert_eq!(ErrorCode::Connection.code(), 1000);
        assert_eq!(ErrorCode::PermissionDenied.code(), 2000);
        assert_eq!(ErrorCode::InjectionRisk.code(), 3000);
        assert_eq!(ErrorCode::Config.code(), 4000);
        assert_eq!(ErrorCode::Migration.code(), 5000);
        assert_eq!(ErrorCode::Query.code(), 6000);
        assert_eq!(ErrorCode::Cache.code(), 7000);
        assert_eq!(ErrorCode::PermissionDenied.name(), "PermissionDenied");
    }

    /// 顶层结构：构造 + JSON 形态
    #[test]
    fn test_unified_error_json_shape() {
        let err = UnifiedDbError::new(ErrorCode::PermissionDenied, "select denied on orders")
            .with_source("role policy missing");
        let json = err.to_json();
        assert_eq!(json["code"], 2000);
        assert_eq!(json["name"], "PermissionDenied");
        assert_eq!(json["message"], "select denied on orders");
        assert_eq!(json["source"], "role policy missing");
        let display = err.to_string();
        assert!(
            display.contains("[2000:PermissionDenied]"),
            "display: {display}"
        );
        assert!(display.contains("select denied on orders"));
    }

    /// From 兼容层：foundation::DbError → 统一错误（码映射正确）
    #[test]
    fn test_from_legacy_db_error() {
        let unified: UnifiedDbError =
            crate::foundation::DbError::Permission("denied on users".to_string()).into();
        assert_eq!(unified.code(), ErrorCode::PermissionDenied);
        assert!(unified.message().contains("denied on users"));
        assert!(unified.source_text().is_some(), "应保留底层错误文本");

        let unified: UnifiedDbError =
            crate::foundation::DbError::Connection(sea_orm::DbErr::RecordNotFound("x".into()))
                .into();
        assert_eq!(unified.code(), ErrorCode::Connection);
    }

    /// 反向兼容层：统一错误 → foundation::DbError（有损但可用）
    #[test]
    fn test_into_legacy_db_error() {
        let unified = UnifiedDbError::new(ErrorCode::PermissionDenied, "denied");
        let legacy: crate::foundation::DbError = unified.into();
        assert!(matches!(legacy, crate::foundation::DbError::Permission(_)));

        let unified = UnifiedDbError::new(ErrorCode::Migration, "bad migration");
        let legacy: crate::foundation::DbError = unified.into();
        assert!(matches!(legacy, crate::foundation::DbError::Migration(_)));
    }

    /// 反向映射保持错误类别：Connection/Query/Config 不被吞成其他变体
    #[test]
    fn test_reverse_mapping_preserves_variant() {
        let unified = UnifiedDbError::new(ErrorCode::Connection, "conn refused");
        let legacy: crate::foundation::DbError = unified.into();
        assert!(
            matches!(legacy, crate::foundation::DbError::Connection(_)),
            "Connection 码应映射回 Connection 变体"
        );

        let unified = UnifiedDbError::new(ErrorCode::Query, "bad query");
        let legacy: crate::foundation::DbError = unified.into();
        assert!(matches!(legacy, crate::foundation::DbError::Query(_)));
    }

    /// 正向映射分段：权限拒绝 → 2000，权限配置错误 → 2001
    #[cfg(feature = "permission")]
    #[test]
    fn test_forward_mapping_permission_segments() {
        let denied: UnifiedDbError =
            DbNexusError::Permission(crate::domain::PermissionError::Denied {
                resource: "users".to_string(),
                operation: "DELETE".to_string(),
            })
            .into();
        assert_eq!(denied.code(), ErrorCode::PermissionDenied);

        let config_err: UnifiedDbError = DbNexusError::PermissionConfig(
            crate::domain::PermissionConfigError::MissingField("roles".to_string()),
        )
        .into();
        assert_eq!(config_err.code(), ErrorCode::PermissionConfig);
    }

    /// 统一错误 → 查询错误报告（码段 → 类别）
    #[test]
    fn test_unified_error_to_report() {
        let unified = UnifiedDbError::new(ErrorCode::PermissionDenied, "denied on orders");
        let report = QueryErrorReport::from(unified);
        assert_eq!(report.category, ErrorCategory::Permission);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_category_display() {
        assert_eq!(ErrorCategory::Permission.to_string(), "Permission");
        assert_eq!(ErrorCategory::InjectionRisk.to_string(), "InjectionRisk");
        assert_eq!(ErrorCategory::SyntaxError.to_string(), "SyntaxError");
        assert_eq!(ErrorCategory::ShardConflict.to_string(), "ShardConflict");
    }

    #[test]
    fn test_query_error_report_new() {
        let report = QueryErrorReport::new(ErrorCategory::SyntaxError, "bad sql", "fix it");
        assert_eq!(report.category, ErrorCategory::SyntaxError);
        assert_eq!(report.message, "bad sql");
        assert_eq!(report.suggestion, "fix it");
        assert!(report.table.is_none());
        assert!(report.operation.is_none());
    }

    #[test]
    fn test_query_error_report_with_table() {
        let report = QueryErrorReport::new(ErrorCategory::Permission, "denied", "check perms")
            .with_table("users");
        assert_eq!(report.table.as_deref(), Some("users"));
    }

    #[test]
    fn test_query_error_report_with_operation() {
        let report = QueryErrorReport::new(ErrorCategory::InjectionRisk, "injection", "use params")
            .with_operation("SELECT");
        assert_eq!(report.operation.as_deref(), Some("SELECT"));
    }

    #[test]
    fn test_query_error_report_display_full() {
        let report = QueryErrorReport::new(
            ErrorCategory::InjectionRisk,
            "SQL contains UNION",
            "Use parameterized queries",
        )
        .with_table("users")
        .with_operation("SELECT");
        let display = report.to_string();
        assert!(display.contains("[InjectionRisk]"));
        assert!(display.contains("SQL contains UNION"));
        assert!(display.contains("Suggestion: Use parameterized queries"));
        assert!(display.contains("Table: users"));
        assert!(display.contains("Operation: SELECT"));
    }

    #[test]
    fn test_query_error_report_display_minimal() {
        let report = QueryErrorReport::new(ErrorCategory::SyntaxError, "bad sql", "fix it");
        let display = report.to_string();
        assert!(display.contains("[SyntaxError]"));
        assert!(display.contains("bad sql"));
        assert!(!display.contains("Table:"));
        assert!(!display.contains("Operation:"));
    }

    #[test]
    fn test_query_error_report_error_trait() {
        let report = QueryErrorReport::new(ErrorCategory::SyntaxError, "bad sql", "fix it");
        // Error trait: source() returns None
        assert!(std::error::Error::source(&report).is_none());
    }

    #[test]
    fn test_from_db_nexus_error_unsupported_scheme() {
        let err = DbNexusError::UnsupportedDatabaseScheme("oracle://localhost".to_string());
        let report = QueryErrorReport::from(err);
        assert_eq!(report.category, ErrorCategory::SyntaxError);
        assert!(report.message.contains("oracle"));
    }

    #[cfg(feature = "permission")]
    #[test]
    fn test_from_db_nexus_error_permission() {
        let err = DbNexusError::Permission(crate::domain::PermissionError::Denied {
            resource: "users".to_string(),
            operation: "DELETE".to_string(),
        });
        let report = QueryErrorReport::from(err);
        assert_eq!(report.category, ErrorCategory::Permission);
    }

    #[test]
    fn test_error_code_name_table() {
        let cases = [
            (ErrorCode::Unknown, "Unknown"),
            (ErrorCode::Connection, "Connection"),
            (ErrorCode::PermissionDenied, "PermissionDenied"),
            (ErrorCode::PermissionConfig, "PermissionConfig"),
            (ErrorCode::RateLimited, "RateLimited"),
            (ErrorCode::InjectionRisk, "InjectionRisk"),
            (ErrorCode::SqlSyntax, "SqlSyntax"),
            (ErrorCode::Config, "Config"),
            (ErrorCode::Migration, "Migration"),
            (ErrorCode::Query, "Query"),
            (ErrorCode::Transaction, "Transaction"),
            (ErrorCode::Cache, "Cache"),
            (ErrorCode::Validation, "Validation"),
        ];
        for (code, name) in cases {
            assert_eq!(code.name(), name);
        }
    }

    #[test]
    fn test_error_code_localized_msg() {
        use crate::i18n::error_ext::LocalizedMsg;
        let code = ErrorCode::Connection;
        assert_eq!(code.message_key(), "error-code");
        let args = code.message_args();
        assert!(args.contains(&("code", "1000".to_string())));
        assert!(args.contains(&("name", "Connection".to_string())));
    }

    #[test]
    fn test_error_category_localized_msg_keys() {
        use crate::i18n::error_ext::LocalizedMsg;
        assert_eq!(
            ErrorCategory::Permission.message_key(),
            "error-category-permission"
        );
        assert_eq!(
            ErrorCategory::InjectionRisk.message_key(),
            "error-category-injection-risk"
        );
        assert_eq!(
            ErrorCategory::SyntaxError.message_key(),
            "error-category-syntax-error"
        );
        assert_eq!(
            ErrorCategory::ShardConflict.message_key(),
            "error-category-shard-conflict"
        );
    }

    #[test]
    fn test_query_error_report_localized_msg_args() {
        use crate::i18n::error_ext::LocalizedMsg;
        let minimal = QueryErrorReport::new(ErrorCategory::SyntaxError, "bad sql", "fix it");
        assert_eq!(minimal.message_key(), "query-error-report");
        let args = minimal.message_args();
        assert_eq!(args.len(), 3);

        let full = minimal.clone().with_table("users").with_operation("SELECT");
        let args = full.message_args();
        assert_eq!(args.len(), 5);
        assert!(args.contains(&("table", "users".to_string())));
        assert!(args.contains(&("operation", "SELECT".to_string())));
    }

    #[test]
    fn test_db_nexus_error_localized_msg() {
        use crate::i18n::error_ext::LocalizedMsg;
        let err = DbNexusError::UnsupportedDatabaseScheme("oracle".to_string());
        assert_eq!(err.message_key(), "nexus-unsupported-database");
        assert_eq!(err.message_args(), vec![("scheme", "oracle".to_string())]);
    }

    #[test]
    fn test_unified_error_display_with_source() {
        let err = UnifiedDbError::new(ErrorCode::Cache, "cache miss").with_source("redis down");
        let display = err.to_string();
        assert!(display.contains("[7000:Cache]"));
        assert!(display.contains("cache miss"));
        assert!(display.contains("(source: redis down)"));
    }

    #[test]
    fn test_from_legacy_db_error_all_segments() {
        let cases = [
            (
                crate::foundation::DbError::Permission("p".to_string()),
                ErrorCode::PermissionDenied,
            ),
            (
                crate::foundation::DbError::RateLimited {
                    retry_after_secs: Some(3),
                },
                ErrorCode::RateLimited,
            ),
            (
                crate::foundation::DbError::Transaction("t".to_string()),
                ErrorCode::Transaction,
            ),
            (
                crate::foundation::DbError::Migration("m".to_string()),
                ErrorCode::Migration,
            ),
            (
                crate::foundation::DbError::Cache("c".to_string()),
                ErrorCode::Cache,
            ),
            (
                crate::foundation::DbError::Query("q".to_string()),
                ErrorCode::Query,
            ),
            (
                crate::foundation::DbError::Config("cfg".to_string()),
                ErrorCode::Config,
            ),
            (
                crate::foundation::DbError::Unsupported("u".to_string()),
                ErrorCode::Query,
            ),
            (
                crate::foundation::DbError::VersionConflict {
                    table: "t9".to_string(),
                    id: 1,
                },
                ErrorCode::Query,
            ),
        ];
        for (legacy, expected) in cases {
            let unified: UnifiedDbError = legacy.into();
            assert_eq!(unified.code(), expected);
        }

        #[cfg(feature = "validation")]
        {
            let unified: UnifiedDbError =
                crate::foundation::DbError::Validation("v".to_string()).into();
            assert_eq!(unified.code(), ErrorCode::Validation);
        }
    }

    #[test]
    fn test_from_db_nexus_error_segments() {
        let scheme: UnifiedDbError =
            DbNexusError::UnsupportedDatabaseScheme("oracle".to_string()).into();
        assert_eq!(scheme.code(), ErrorCode::SqlSyntax);

        #[cfg(feature = "permission")]
        {
            let denied: UnifiedDbError = DbNexusError::Permission(
                crate::domain::PermissionError::RoleNotFound("r1".to_string()),
            )
            .into();
            assert_eq!(denied.code(), ErrorCode::PermissionDenied);

            let config_err: UnifiedDbError = DbNexusError::PermissionConfig(
                crate::domain::PermissionConfigError::PolicyFileNotFound("p.json".to_string()),
            )
            .into();
            assert_eq!(config_err.code(), ErrorCode::PermissionConfig);
        }
    }

    #[test]
    fn test_into_legacy_db_error_all_segments() {
        // RateLimited 反向映射有损：Retry-After 与消息不保留（见 From impl 注释），
        // 只断言变体；其余断言消息文本保留
        let rate: crate::foundation::DbError =
            UnifiedDbError::new(ErrorCode::RateLimited, "rate").into();
        assert!(matches!(
            rate,
            crate::foundation::DbError::RateLimited {
                retry_after_secs: None
            }
        ));

        let cases = [
            (ErrorCode::Transaction, "txn"),
            (ErrorCode::Cache, "cache"),
            (ErrorCode::SqlSyntax, "sql"),
            (ErrorCode::InjectionRisk, "inj"),
            (ErrorCode::Query, "query"),
            (ErrorCode::Unknown, "unknown"),
            (ErrorCode::Config, "config"),
            (ErrorCode::Validation, "val"),
            (ErrorCode::PermissionConfig, "perm-cfg"),
        ];
        for (code, msg) in cases {
            let unified = UnifiedDbError::new(code, msg).with_source("detail");
            let legacy: crate::foundation::DbError = unified.into();
            let text = legacy.to_string();
            assert!(text.contains(msg), "code {code:?} text: {text}");
            assert!(text.contains("detail"), "source 文本应保留: {text}");
        }
    }

    #[test]
    fn test_unified_error_to_report_categories() {
        let injection = QueryErrorReport::from(UnifiedDbError::new(ErrorCode::InjectionRisk, "x"));
        assert_eq!(injection.category, ErrorCategory::InjectionRisk);

        let syntax = QueryErrorReport::from(UnifiedDbError::new(ErrorCode::SqlSyntax, "x"));
        assert_eq!(syntax.category, ErrorCategory::SyntaxError);

        let fallback = QueryErrorReport::from(UnifiedDbError::new(ErrorCode::Unknown, "x"));
        assert_eq!(fallback.category, ErrorCategory::SyntaxError);

        #[cfg(feature = "permission")]
        {
            let config_report = QueryErrorReport::from(DbNexusError::PermissionConfig(
                crate::domain::PermissionConfigError::MissingField("roles".to_string()),
            ));
            assert_eq!(config_report.category, ErrorCategory::Permission);
        }
    }
}
