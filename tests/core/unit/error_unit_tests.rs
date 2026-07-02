// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 错误类型单元测试
//!
//! 测试 DbError, PoolError, PermissionError, ConfigError, MigrationError, AuditError

use dbnexus::foundation::error::{PermissionError, PoolError};
use dbnexus::{AuditError, ConfigError, DbError, MigrationError};

// ============================================================================
// DbError 测试
// ============================================================================

#[test]
fn test_db_error_creation() {
    // 测试从 sea_orm::DbErr 创建 DbError
    let io_error = sea_orm::DbErr::Custom("Test error".to_string());
    let db_error = DbError::new(io_error);

    assert!(format!("{db_error}").contains("Test error"));
}

#[test]
fn test_db_error_message() {
    let io_error = sea_orm::DbErr::Custom("Connection failed".to_string());
    let db_error = DbError::new(io_error.clone());

    // DbError 使用 message() 方法获取错误消息
    assert!(db_error.message().contains("Connection failed"));
}

#[test]
fn test_db_error_display() {
    let error = sea_orm::DbErr::Custom("Database error".to_string());
    let db_error = DbError::new(error);

    assert!(format!("{db_error}").contains("Database error"));
}

// ============================================================================
// PoolError 测试
// ============================================================================

#[test]
fn test_pool_error_acquire_timeout() {
    let error = PoolError::AcquireTimeout;
    assert_eq!(format!("{error}"), "Failed to acquire connection within timeout");
}

#[test]
fn test_pool_error_pool_exhausted() {
    let error = PoolError::PoolExhausted;
    assert_eq!(format!("{error}"), "Connection pool exhausted");
}

#[test]
fn test_pool_error_connection_failed() {
    let error = PoolError::ConnectionFailed("Database unreachable".to_string());
    assert_eq!(format!("{error}"), "Failed to create connection: Database unreachable");
}

#[test]
fn test_pool_error_health_check_failed() {
    let error = PoolError::HealthCheckFailed("Connection unhealthy".to_string());
    assert_eq!(format!("{error}"), "Health check failed: Connection unhealthy");
}

#[test]
fn test_pool_error_debug() {
    let error = PoolError::AcquireTimeout;
    assert!(!format!("{error:?}").is_empty());
}

// ============================================================================
// PermissionError 测试
// ============================================================================

#[test]
fn test_permission_error_denied() {
    let error = PermissionError::Denied {
        resource: "users".to_string(),
        operation: "DELETE".to_string(),
    };
    assert_eq!(format!("{error}"), "permission denied for DELETE on users");
}

#[test]
fn test_permission_error_role_not_found() {
    let error = PermissionError::RoleNotFound("admin".to_string());
    assert_eq!(format!("{error}"), "role not found: admin");
}

#[test]
fn test_permission_error_invalid_policy() {
    let error = PermissionError::InvalidPolicy("Missing policy".to_string());
    assert_eq!(format!("{error}"), "invalid policy configuration: Missing policy");
}

#[test]
fn test_permission_error_rate_limited() {
    let error = PermissionError::RateLimited;
    assert_eq!(format!("{error}"), "rate limit exceeded");
}

#[test]
fn test_permission_error_display_variants() {
    let denied = PermissionError::Denied {
        resource: "orders".to_string(),
        operation: "UPDATE".to_string(),
    };
    let not_found = PermissionError::RoleNotFound("guest".to_string());
    let invalid = PermissionError::InvalidPolicy("Bad YAML".to_string());
    let rate_limited = PermissionError::RateLimited;

    assert!(format!("{denied}").contains("UPDATE"));
    assert!(format!("{not_found}").contains("guest"));
    assert!(format!("{invalid}").contains("YAML"));
    assert!(format!("{rate_limited}").contains("rate"));
}

// ============================================================================
// ConfigError 测试
// ============================================================================

#[test]
fn test_config_error_missing_field() {
    let error = ConfigError::MissingField("db_url".to_string());
    assert!(format!("{error}").contains("Missing"));
}

#[test]
fn test_config_error_invalid_format() {
    let error = ConfigError::InvalidFormat("invalid yaml".to_string());
    assert!(format!("{error}").contains("Invalid"));
}

#[test]
fn test_config_error_file_not_found() {
    let error = ConfigError::FileNotFound("config.yaml".to_string());
    assert!(format!("{error}").contains("file"));
}

#[test]
fn test_config_error_file_read_error() {
    let error = ConfigError::IoError("permission denied".to_string());
    assert!(format!("{error}").contains("IO"));
}

#[test]
fn test_config_error_invalid_url() {
    let error = ConfigError::InvalidUrl("invalid://url".to_string());
    assert!(format!("{error}").contains("Invalid URL"));
}

#[test]
fn test_config_error_unsupported_protocol() {
    let error = ConfigError::UnsupportedProtocol("ftp".to_string());
    assert!(format!("{error}").contains("Unsupported"));
}

#[test]
fn test_config_error_variants() {
    let missing = ConfigError::MissingField("field".to_string());
    let invalid = ConfigError::InvalidFormat("format".to_string());
    let not_found = ConfigError::FileNotFound("file".to_string());
    let read_error = ConfigError::IoError("error".to_string());
    let invalid_url = ConfigError::InvalidUrl("ftp://localhost".to_string());
    let unsupported = ConfigError::UnsupportedProtocol("ftp".to_string());

    assert!(format!("{missing}").contains("Missing"));
    assert!(format!("{invalid}").contains("Invalid"));
    assert!(format!("{not_found}").contains("file"));
    assert!(format!("{read_error}").contains("IO"));
    assert!(format!("{invalid_url}").contains("ftp://localhost"));
    assert!(format!("{unsupported}").contains("Unsupported"));
}

// ============================================================================
// MigrationError 测试
// ============================================================================

#[test]
fn test_migration_error_file_not_found() {
    let error = MigrationError::FileNotFound("001_init.sql".to_string());
    assert_eq!(format!("{error}"), "Migration file not found: 001_init.sql");
}

#[test]
fn test_migration_error_parse_error() {
    let error = MigrationError::ParseError("Invalid SQL syntax".to_string());
    assert_eq!(format!("{error}"), "Failed to parse migration file: Invalid SQL syntax");
}

#[test]
fn test_migration_error_execution_error() {
    let error = MigrationError::ExecutionError("Duplicate key".to_string());
    assert_eq!(format!("{error}"), "Migration execution failed: Duplicate key");
}

#[test]
fn test_migration_error_version_conflict() {
    let error = MigrationError::VersionConflict("Version 2 already exists".to_string());
    assert_eq!(
        format!("{error}"),
        "Migration version conflict: Version 2 already exists"
    );
}

#[test]
fn test_migration_error_rollback_error() {
    let error = MigrationError::RollbackError("Cannot drop table".to_string());
    assert_eq!(format!("{error}"), "Migration rollback failed: Cannot drop table");
}

#[test]
fn test_migration_error_all_variants() {
    let not_found = MigrationError::FileNotFound("001.sql".to_string());
    let parse = MigrationError::ParseError("syntax error".to_string());
    let exec = MigrationError::ExecutionError("constraint".to_string());
    let version = MigrationError::VersionConflict("duplicate".to_string());
    let rollback = MigrationError::RollbackError("failed".to_string());

    assert!(format!("{not_found}").contains("001.sql"));
    assert!(format!("{parse}").contains("syntax"));
    assert!(format!("{exec}").contains("constraint"));
    assert!(format!("{version}").contains("duplicate"));
    assert!(format!("{rollback}").contains("failed"));
}

// ============================================================================
// AuditError 测试
// ============================================================================

#[test]
fn test_audit_error_write_error() {
    let error = AuditError::WriteError("Disk full".to_string());
    assert_eq!(format!("{error}"), "Failed to write audit log: Disk full");
}

#[test]
fn test_audit_error_serialization_error() {
    let error = AuditError::SerializationError("JSON error".to_string());
    assert_eq!(format!("{error}"), "Failed to serialize audit data: JSON error");
}

#[test]
fn test_audit_error_config_error() {
    let error = AuditError::ConfigError("Missing field".to_string());
    assert_eq!(format!("{error}"), "Invalid audit configuration: Missing field");
}

#[test]
fn test_audit_error_all_variants() {
    let write = AuditError::WriteError("IO error".to_string());
    let serialize = AuditError::SerializationError("JSON".to_string());
    let config = AuditError::ConfigError("field missing".to_string());

    assert!(format!("{write}").contains("IO"));
    assert!(format!("{serialize}").contains("JSON"));
    assert!(format!("{config}").contains("field"));
}

// ============================================================================
// 结果类型别名测试
// ============================================================================

#[test]
fn test_db_result_type_alias() {
    // 测试 DbResult 类型别名
    let success: dbnexus::DbResult<i32> = Ok(42);
    let error = sea_orm::DbErr::Custom("Error".to_string());
    let failure: dbnexus::DbResult<i32> = Err(DbError::new(error));

    assert!(success.is_ok());
    assert_eq!(success.ok(), Some(42));
    assert!(failure.is_err());
}

#[test]
fn test_pool_result_type_alias() {
    // 测试 PoolResult 类型别名
    let success: dbnexus::PoolResult<String> = Ok("connection".to_string());
    let failure: dbnexus::PoolResult<String> = Err(PoolError::PoolExhausted);

    assert!(success.is_ok());
    assert_eq!(success.ok(), Some("connection".to_string()));
    assert!(failure.is_err());
}

#[test]
fn test_permission_result_type_alias() {
    // 测试 PermissionResult 类型别名
    let success: dbnexus::PermissionResult<bool> = Ok(true);
    let failure: dbnexus::PermissionResult<bool> = Err(PermissionError::RateLimited);

    assert!(success.is_ok());
    assert_eq!(success.ok(), Some(true));
    assert!(failure.is_err());
}

#[test]
fn test_config_result_type_alias() {
    // 测试 ConfigResult 类型别名
    let success: dbnexus::ConfigResult<String> = Ok("config".to_string());
    let failure: dbnexus::ConfigResult<String> = Err(ConfigError::FileNotFound("config.yaml".to_string()));

    assert!(success.is_ok());
    assert_eq!(success.ok(), Some("config".to_string()));
    assert!(failure.is_err());
}

#[test]
fn test_migration_result_type_alias() {
    // 测试 MigrationResult 类型别名
    let success: dbnexus::MigrationResult<u32> = Ok(5);
    let failure: dbnexus::MigrationResult<u32> = Err(MigrationError::FileNotFound("001.sql".to_string()));

    assert!(success.is_ok());
    assert_eq!(success.ok(), Some(5));
    assert!(failure.is_err());
}

#[cfg(feature = "audit")]
#[test]
fn test_audit_result_type_alias() {
    // 测试 AuditResult 类型别名
    let success: dbnexus::AuditResult<String> = Ok("logged".to_string());
    let failure: dbnexus::AuditResult<String> = Err(AuditError::WriteError("IO".to_string()));

    assert!(success.is_ok());
    assert_eq!(success.ok(), Some("logged".to_string()));
    assert!(failure.is_err());
}

// ============================================================================
// 错误转换测试
// ============================================================================

#[test]
fn test_error_source_chain() {
    let error = PoolError::ConnectionFailed("Database down".to_string());
    let debug_output = format!("{error:?}");

    assert!(!debug_output.is_empty());
}

#[test]
fn test_error_display_consistency() {
    // 验证所有错误类型的 Display 实现都正常工作
    let errors: Vec<(&str, &str)> = vec![
        (
            "PoolError::AcquireTimeout",
            "Failed to acquire connection within timeout",
        ),
        ("PermissionError::RateLimited", "rate limit exceeded"),
        ("ConfigError::FileNotFound", "file not found"),
        ("MigrationError::FileNotFound", "Migration file not found:"),
        ("AuditError::WriteError", "Failed to write audit log:"),
    ];

    for (variant, expected) in errors {
        match variant {
            "PoolError::AcquireTimeout" => {
                let error = PoolError::AcquireTimeout;
                assert!(format!("{error}").contains(expected.split(':').next().unwrap()));
            }
            "PermissionError::RateLimited" => {
                let error = PermissionError::RateLimited;
                assert!(format!("{error}").contains(expected));
            }
            "ConfigError::FileNotFound" => {
                let error = ConfigError::FileNotFound("config.yaml".to_string());
                assert!(format!("{error}").contains(expected));
            }
            "MigrationError::FileNotFound" => {
                let error = MigrationError::FileNotFound("test".to_string());
                assert!(format!("{error}").contains("Migration file not found"));
            }
            "AuditError::WriteError" => {
                let error = AuditError::WriteError("test".to_string());
                assert!(format!("{error}").contains("Failed to write audit log"));
            }
            _ => {}
        }
    }
}

// ============================================================================
// 错误消息边界测试
// ============================================================================

#[test]
fn test_error_with_empty_message() {
    let error = PermissionError::Denied {
        resource: "".to_string(),
        operation: "".to_string(),
    };
    let display = format!("{error}");
    assert!(display.contains("permission denied"));
}

#[test]
fn test_error_with_long_message() {
    let long_message = "a".repeat(1000);
    let error = ConfigError::InvalidUrl(long_message.clone());
    let display = format!("{error}");

    assert!(display.contains(&long_message));
}

#[test]
fn test_error_with_special_characters() {
    let error = MigrationError::ParseError("Error with 'quotes' and \"double quotes\"".to_string());
    let display = format!("{error}");

    assert!(display.contains("Error with"));
}

#[test]
fn test_error_with_unicode() {
    let error = ConfigError::InvalidUrl("错误消息".to_string());
    let display = format!("{error}");

    assert!(display.contains("错误消息"));
}

// ============================================================================
// 错误 Debug 实现测试
// ============================================================================

#[test]
fn test_all_errors_debug_impl() {
    // 确保所有错误类型都可以通过 Debug 格式化
    let errors: &[&dyn std::fmt::Debug] = &[
        &DbError::new(sea_orm::DbErr::Custom("test".to_string())),
        &PoolError::AcquireTimeout,
        &PermissionError::RateLimited,
        &ConfigError::FileNotFound("config.yaml".to_string()),
        &MigrationError::FileNotFound("test".to_string()),
        &AuditError::WriteError("test".to_string()),
    ];

    for error in errors {
        let debug = format!("{error:?}");
        assert!(!debug.is_empty());
    }
}

// ============================================================================
// QueryErrorReport 测试（v0.3.0 新增）
// ============================================================================

use dbnexus::{ErrorCategory, QueryErrorReport};

/// TEST-U-ERR-001: ErrorCategory Display 应输出 PascalCase 类别名
#[test]
fn test_error_category_display() {
    assert_eq!(ErrorCategory::Permission.to_string(), "Permission");
    assert_eq!(ErrorCategory::InjectionRisk.to_string(), "InjectionRisk");
    assert_eq!(ErrorCategory::SyntaxError.to_string(), "SyntaxError");
    assert_eq!(ErrorCategory::ShardConflict.to_string(), "ShardConflict");
}

/// TEST-U-ERR-002: ErrorCategory 应支持 Clone/Copy/PartialEq/Eq
#[test]
fn test_error_category_traits() {
    let a = ErrorCategory::Permission;
    let b = a; // Copy
    assert_eq!(a, b); // PartialEq + Eq
    let c = b.clone(); // Clone
    assert_eq!(a, c);
}

/// TEST-U-ERR-003: QueryErrorReport::new 应构造带空 table/operation 的报告
#[test]
fn test_query_error_report_new_basic() {
    let report = QueryErrorReport::new(
        ErrorCategory::InjectionRisk,
        "UNION-based injection detected",
        "Use parameterized queries",
    );
    assert_eq!(report.category, ErrorCategory::InjectionRisk);
    assert_eq!(report.message, "UNION-based injection detected");
    assert_eq!(report.suggestion, "Use parameterized queries");
    assert!(report.table.is_none());
    assert!(report.operation.is_none());
}

/// TEST-U-ERR-004: with_table/with_operation 链式构造应正确设置字段
#[test]
fn test_query_error_report_builder_chaining() {
    let report = QueryErrorReport::new(
        ErrorCategory::Permission,
        "role lacks DELETE permission",
        "Grant DELETE on the table to the role",
    )
    .with_table("orders")
    .with_operation("DELETE");

    assert_eq!(report.table.as_deref(), Some("orders"));
    assert_eq!(report.operation.as_deref(), Some("DELETE"));
}

/// TEST-U-ERR-005: Display 应输出简化格式（无可选字段）
#[test]
fn test_query_error_report_display_minimal() {
    let report = QueryErrorReport::new(
        ErrorCategory::ShardConflict,
        "cross-shard query detected",
        "Route the query to a single shard",
    );
    let display = format!("{report}");
    assert_eq!(
        display,
        "[ShardConflict] cross-shard query detected\nSuggestion: Route the query to a single shard"
    );
}

/// TEST-U-ERR-006: Display 应输出完整格式（含 table 和 operation）
#[test]
fn test_query_error_report_display_full() {
    let report = QueryErrorReport::new(
        ErrorCategory::SyntaxError,
        "near \"FROM\": syntax error",
        "Check the SQL syntax near the FROM clause",
    )
    .with_table("users")
    .with_operation("SELECT");
    let display = format!("{report}");
    assert_eq!(
        display,
        "[SyntaxError] near \"FROM\": syntax error\nSuggestion: Check the SQL syntax near the FROM clause\nTable: users\nOperation: SELECT"
    );
}

/// TEST-U-ERR-007: Display 仅含 table 时不应输出 Operation 行
#[test]
fn test_query_error_report_display_table_only() {
    let report = QueryErrorReport::new(
        ErrorCategory::Permission,
        "denied",
        "grant access",
    )
    .with_table("accounts");
    let display = format!("{report}");
    assert!(display.contains("Table: accounts"));
    assert!(!display.contains("Operation:"));
}

/// TEST-U-ERR-008: From<DbNexusError> 应将 UnsupportedDatabaseScheme 映射为 SyntaxError
#[test]
fn test_query_error_report_from_unsupported_scheme() {
    use dbnexus::DbNexusError;
    let err = DbNexusError::UnsupportedDatabaseScheme("ftp://localhost".to_string());
    let report = QueryErrorReport::from(err);
    assert_eq!(report.category, ErrorCategory::SyntaxError);
    assert!(report.message.contains("ftp://localhost"));
    assert!(report.suggestion.contains("sqlite"));
    assert!(report.suggestion.contains("duckdb"));
}

/// TEST-U-ERR-009: From<DbNexusError> 应将 Permission 错误映射为 Permission 类别
#[cfg(feature = "permission")]
#[test]
fn test_query_error_report_from_permission_error() {
    use dbnexus::DbNexusError;
    let perm_err = dbnexus::foundation::error::PermissionError::Denied {
        resource: "users".to_string(),
        operation: "DELETE".to_string(),
    };
    let err: DbNexusError = perm_err.into();
    let report = QueryErrorReport::from(err);
    assert_eq!(report.category, ErrorCategory::Permission);
    assert!(report.message.contains("users"));
    assert!(report.message.contains("DELETE"));
}

/// TEST-U-ERR-010: QueryErrorReport 应实现 std::error::Error
#[test]
fn test_query_error_report_implements_error_trait() {
    let report = QueryErrorReport::new(
        ErrorCategory::InjectionRisk,
        "test",
        "suggestion",
    );
    // 可作为 trait object 使用
    let err: &dyn std::error::Error = &report;
    assert!(err.source().is_none());
    assert!(!err.to_string().is_empty());
}

/// TEST-U-ERR-011: Debug 实现应输出非空字符串
#[test]
fn test_query_error_report_debug() {
    let report = QueryErrorReport::new(
        ErrorCategory::ShardConflict,
        "conflict",
        "reroute",
    )
    .with_table("t")
    .with_operation("INSERT");
    let debug = format!("{report:?}");
    assert!(debug.contains("ShardConflict"));
    assert!(debug.contains("conflict"));
}

/// TEST-U-ERR-012: Clone 实现应产生相等副本
#[test]
fn test_query_error_report_clone() {
    let original = QueryErrorReport::new(
        ErrorCategory::SyntaxError,
        "msg",
        "sug",
    )
    .with_table("t")
    .with_operation("op");
    let cloned = original.clone();
    assert_eq!(original.category, cloned.category);
    assert_eq!(original.message, cloned.message);
    assert_eq!(original.suggestion, cloned.suggestion);
    assert_eq!(original.table, cloned.table);
    assert_eq!(original.operation, cloned.operation);
}
