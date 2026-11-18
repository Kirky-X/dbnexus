// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 结构化权限拒绝原因码单元测试
//!
//! 覆盖：
//! - DenialReason 各变体的稳定 reason_code
//! - context_fields 键名与脱敏后的上下文值
//! - DbError::PermissionDenied 向后兼容（Permission(String) 输出不变）

use dbnexus::DbError;
use dbnexus::foundation::error::denial::DenialReason;

#[test]
fn test_reason_codes_are_stable() {
    let cases: Vec<(DenialReason, &str)> = vec![
        (DenialReason::DdlForbidden, "DDL_FORBIDDEN"),
        (DenialReason::UnsupportedStatement, "UNSUPPORTED_STATEMENT"),
        (DenialReason::TableExtractFailed, "TABLE_EXTRACT_FAILED"),
        (DenialReason::SqlParseFailed, "SQL_PARSE_FAILED"),
        (
            DenialReason::TableNotInPolicy {
                table: "orders".into(),
                action: "Select".into(),
            },
            "TABLE_NOT_IN_POLICY",
        ),
        (
            DenialReason::RoleNotDefined {
                role: "guest".into(),
            },
            "ROLE_NOT_DEFINED",
        ),
        (DenialReason::RlsParseFailed, "RLS_PARSE_FAILED"),
        (
            DenialReason::RlsMultiTableUnsupported {
                tables: vec!["orders".into(), "users".into()],
            },
            "RLS_MULTI_TABLE_UNSUPPORTED",
        ),
        (
            DenialReason::RlsPredicateRejected {
                table: "orders".into(),
            },
            "RLS_PREDICATE_REJECTED",
        ),
    ];
    for (reason, code) in cases {
        assert_eq!(reason.reason_code(), code, "reason: {reason:?}");
    }
}

#[test]
fn test_context_fields_keys() {
    let cases: Vec<(DenialReason, Vec<&str>)> = vec![
        (DenialReason::DdlForbidden, vec![]),
        (DenialReason::UnsupportedStatement, vec![]),
        (DenialReason::TableExtractFailed, vec![]),
        (DenialReason::SqlParseFailed, vec![]),
        (
            DenialReason::TableNotInPolicy {
                table: "orders".into(),
                action: "Select".into(),
            },
            vec!["table", "action"],
        ),
        (
            DenialReason::RoleNotDefined {
                role: "guest".into(),
            },
            vec!["role"],
        ),
        (DenialReason::RlsParseFailed, vec![]),
        (
            DenialReason::RlsMultiTableUnsupported {
                tables: vec!["orders".into()],
            },
            vec!["tables"],
        ),
        (
            DenialReason::RlsPredicateRejected {
                table: "orders".into(),
            },
            vec!["table"],
        ),
    ];
    for (reason, keys) in cases {
        let fields = reason.context_fields();
        let actual: Vec<&str> = fields.iter().map(|(k, _)| *k).collect();
        assert_eq!(actual, keys, "reason: {reason:?}");
    }
}

#[test]
fn test_permission_legacy_display_unchanged() {
    let err = DbError::Permission("x".into());
    assert_eq!(format!("{err}"), "Permission denied: x");
    assert_eq!(err.message(), "x");
}

#[test]
fn test_permission_denied_from_reason() {
    let reason = DenialReason::TableNotInPolicy {
        table: "orders".into(),
        action: "Select".into(),
    };
    let err: DbError = reason.clone().into();
    match err {
        DbError::PermissionDenied { reason: r } => assert_eq!(r, reason),
        other => panic!("expected PermissionDenied, got {other:?}"),
    }
}
