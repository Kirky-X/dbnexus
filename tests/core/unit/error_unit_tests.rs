// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 错误类型单元测试
//!
//! 测试 DbError, PoolError, PermissionError, ConfigError, MigrationError, AuditError

use dbnexus::ConfigError;
use dbnexus::error::{AuditError, DbError, MigrationError, PermissionError, PoolError};

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
fn test_db_error_inner() {
    let io_error = sea_orm::DbErr::Custom("Connection failed".to_string());
    let db_error = DbError::new(io_error.clone());

    assert_eq!(format!("{}", db_error.inner()), format!("{}", io_error));
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
    assert_eq!(format!("{error}"), "Permission denied for DELETE on users");
}

#[test]
fn test_permission_error_role_not_found() {
    let error = PermissionError::RoleNotFound("admin".to_string());
    assert_eq!(format!("{error}"), "Role not found: admin");
}

#[test]
fn test_permission_error_invalid_config() {
    let error = PermissionError::InvalidConfig("Missing policy".to_string());
    assert_eq!(format!("{error}"), "Invalid permission configuration: Missing policy");
}

#[test]
fn test_permission_error_rate_limited() {
    let error = PermissionError::RateLimited;
    assert_eq!(format!("{error}"), "Rate limit exceeded");
}

#[test]
fn test_permission_error_display_variants() {
    let denied = PermissionError::Denied {
        resource: "orders".to_string(),
        operation: "UPDATE".to_string(),
    };
    let not_found = PermissionError::RoleNotFound("guest".to_string());
    let invalid = PermissionError::InvalidConfig("Bad YAML".to_string());
    let rate_limited = PermissionError::RateLimited;

    assert!(format!("{denied}").contains("UPDATE"));
    assert!(format!("{not_found}").contains("guest"));
    assert!(format!("{invalid}").contains("YAML"));
    assert!(format!("{rate_limited}").contains("Rate"));
}

// ============================================================================
// ConfigError 测试
// ============================================================================

#[test]
fn test_config_error_missing_field() {
    let error = ConfigError::MissingField;
    assert_eq!(format!("{error}"), "Missing required configuration field");
}

#[test]
fn test_config_error_invalid_format() {
    let error = ConfigError::InvalidFormat;
    assert_eq!(format!("{error}"), "Invalid configuration format");
}

#[test]
fn test_config_error_file_not_found() {
    let error = ConfigError::FileNotFound;
    assert_eq!(format!("{error}"), "Configuration file not found");
}

#[test]
fn test_config_error_file_read_error() {
    let error = ConfigError::IoError;
    assert_eq!(format!("{error}"), "Configuration file I/O error");
}

#[test]
fn test_config_error_invalid_url() {
    let error = ConfigError::InvalidUrl("invalid://url".to_string());
    assert_eq!(format!("{error}"), "Invalid database URL format: invalid://url");
}

#[test]
fn test_config_error_unsupported_protocol() {
    let error = ConfigError::UnsupportedProtocol;
    assert_eq!(format!("{error}"), "Unsupported database protocol");
}

#[test]
fn test_config_error_variants() {
    let missing = ConfigError::MissingField;
    let invalid = ConfigError::InvalidFormat;
    let not_found = ConfigError::FileNotFound;
    let read_error = ConfigError::IoError;
    let invalid_url = ConfigError::InvalidUrl("ftp://localhost".to_string());
    let unsupported = ConfigError::UnsupportedProtocol;

    assert!(format!("{missing}").contains("Missing"));
    assert!(format!("{invalid}").contains("Invalid"));
    assert!(format!("{not_found}").contains("file"));
    assert!(format!("{read_error}").contains("I/O"));
    assert!(format!("{invalid_url}").contains("ftp://localhost"));
    assert!(format!("{unsupported}").contains("protocol"));
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
    let success: dbnexus::error::DbResult<i32> = Ok(42);
    let error = sea_orm::DbErr::Custom("Error".to_string());
    let failure: dbnexus::error::DbResult<i32> = Err(DbError::new(error));

    assert_eq!(success.unwrap(), 42);
    assert!(failure.is_err());
}

#[test]
fn test_pool_result_type_alias() {
    // 测试 PoolResult 类型别名
    let success: dbnexus::error::PoolResult<String> = Ok("connection".to_string());
    let failure: dbnexus::error::PoolResult<String> = Err(PoolError::PoolExhausted);

    assert_eq!(success.unwrap(), "connection");
    assert!(failure.is_err());
}

#[test]
fn test_permission_result_type_alias() {
    // 测试 PermissionResult 类型别名
    let success: dbnexus::error::PermissionResult<bool> = Ok(true);
    let failure: dbnexus::error::PermissionResult<bool> = Err(PermissionError::RateLimited);

    assert_eq!(success.unwrap(), true);
    assert!(failure.is_err());
}

#[test]
fn test_config_result_type_alias() {
    // 测试 ConfigResult 类型别名
    let success: dbnexus::error::ConfigResult<String> = Ok("config".to_string());
    let failure: dbnexus::error::ConfigResult<String> = Err(ConfigError::FileNotFound);

    assert_eq!(success.unwrap(), "config");
    assert!(failure.is_err());
}

#[test]
fn test_migration_result_type_alias() {
    // 测试 MigrationResult 类型别名
    let success: dbnexus::error::MigrationResult<u32> = Ok(5);
    let failure: dbnexus::error::MigrationResult<u32> = Err(MigrationError::FileNotFound("001.sql".to_string()));

    assert_eq!(success.unwrap(), 5);
    assert!(failure.is_err());
}

#[test]
fn test_audit_result_type_alias() {
    // 测试 AuditResult 类型别名
    let success: dbnexus::error::AuditResult<String> = Ok("logged".to_string());
    let failure: dbnexus::error::AuditResult<String> = Err(AuditError::WriteError("IO".to_string()));

    assert_eq!(success.unwrap(), "logged");
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
        ("PermissionError::RateLimited", "Rate limit exceeded"),
        ("ConfigError::FileNotFound", "Configuration file not found"),
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
                let error = ConfigError::FileNotFound;
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
    assert!(display.contains("Permission denied"));
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
        &ConfigError::FileNotFound,
        &MigrationError::FileNotFound("test".to_string()),
        &AuditError::WriteError("test".to_string()),
    ];

    for error in errors {
        let debug = format!("{error:?}");
        assert!(!debug.is_empty());
    }
}
