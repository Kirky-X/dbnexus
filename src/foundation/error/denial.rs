// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 权限拒绝的结构化原因（机器可读）
//!
//! 与 [`DbError::Permission`](crate::foundation::DbError::Permission) 的人类文案并存：
//! 本模块提供稳定原因码，供 HTTP/MCP、审计与调用方程序在不解析本地化文案的
//! 情况下区分拒绝类别。`#[non_exhaustive]` 保证后续 change 可无损追加变体。

use crate::i18n::error_ext::LocalizedMsg;

/// 权限拒绝的结构化原因（机器可读）。
///
/// 字段值来自用户 SQL 解析结果或调用方传入；在拒绝点构造本类型时，须经
/// `sanitize_log_field` 同口径清理（剔除控制字符与 Unicode 格式字符），
/// 防止敏感信息与日志注入进入错误与审计通道。
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DenialReason {
    /// DDL 在 Session 出口被拒绝
    DdlForbidden,
    /// 语句类型解析成功但不在受支持集合（SELECT/INSERT/UPDATE/DELETE 之外）
    UnsupportedStatement,
    /// 表名提取失败：语句无表名，或含非法表名标识符
    TableExtractFailed,
    /// SQL 解析失败（不携带原始 SQL 片段，防敏感信息外泄）
    SqlParseFailed,
    /// 角色策略中未授权该表（含 JOIN/子查询中被逐表检查命中的表）
    TableNotInPolicy {
        /// 表名（已脱敏）
        table: String,
        /// 操作类型（已脱敏）
        action: String,
    },
    /// 角色未定义（策略加载结果中不存在该角色的 RolePolicy）
    RoleNotDefined {
        /// 角色名（已脱敏）
        role: String,
    },
    /// RLS：语句解析失败，无法安全判定谓词注入位置
    RlsParseFailed,
    /// RLS：多表语句超出单表谓词覆盖能力（fail-closed）
    RlsMultiTableUnsupported {
        /// 语句涉及的表名清单（已脱敏）
        tables: Vec<String>,
    },
    /// RLS：谓词注入不适用（列白名单不通过 / 策略列非法等）
    RlsPredicateRejected {
        /// 表名（已脱敏）
        table: String,
    },
}

impl DenialReason {
    /// 本地化后的用户可见文案（Display 与错误 message 共享）
    pub fn message(&self) -> String {
        crate::i18n::t(self.message_key(), &self.message_args())
    }

    /// 机器可读稳定原因码（审计/HTTP/MCP 层映射用；永不本地化）
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::DdlForbidden => "DDL_FORBIDDEN",
            Self::UnsupportedStatement => "UNSUPPORTED_STATEMENT",
            Self::TableExtractFailed => "TABLE_EXTRACT_FAILED",
            Self::SqlParseFailed => "SQL_PARSE_FAILED",
            Self::TableNotInPolicy { .. } => "TABLE_NOT_IN_POLICY",
            Self::RoleNotDefined { .. } => "ROLE_NOT_DEFINED",
            Self::RlsParseFailed => "RLS_PARSE_FAILED",
            Self::RlsMultiTableUnsupported { .. } => "RLS_MULTI_TABLE_UNSUPPORTED",
            Self::RlsPredicateRejected { .. } => "RLS_PREDICATE_REJECTED",
        }
    }

    /// 携带的上下文字段（键名，值），供审计与日志统一脱敏。
    ///
    /// `tables` 变体以逗号拼接为一个值返回，键名为 `tables`。
    pub fn context_fields(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::DdlForbidden
            | Self::UnsupportedStatement
            | Self::TableExtractFailed
            | Self::SqlParseFailed
            | Self::RlsParseFailed => vec![],
            Self::TableNotInPolicy { table, action } => {
                vec![("table", table.clone()), ("action", action.clone())]
            }
            Self::RoleNotDefined { role } => vec![("role", role.clone())],
            Self::RlsMultiTableUnsupported { tables } => {
                vec![("tables", tables.join(","))]
            }
            Self::RlsPredicateRejected { table } => vec![("table", table.clone())],
        }
    }
}

impl LocalizedMsg for DenialReason {
    fn message_key(&self) -> &'static str {
        match self {
            Self::DdlForbidden => "denial-reason-ddl-forbidden",
            Self::UnsupportedStatement => "denial-reason-unsupported-statement",
            Self::TableExtractFailed => "denial-reason-table-extract-failed",
            Self::SqlParseFailed => "denial-reason-sql-parse-failed",
            Self::TableNotInPolicy { .. } => "denial-reason-table-not-in-policy",
            Self::RoleNotDefined { .. } => "denial-reason-role-not-defined",
            Self::RlsParseFailed => "denial-reason-rls-parse-failed",
            Self::RlsMultiTableUnsupported { .. } => "denial-reason-rls-multi-table-unsupported",
            Self::RlsPredicateRejected { .. } => "denial-reason-rls-predicate-rejected",
        }
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        match self {
            Self::TableNotInPolicy { table, action } => {
                vec![("table", table.clone()), ("action", action.clone())]
            }
            Self::RoleNotDefined { role } => vec![("role", role.clone())],
            Self::RlsMultiTableUnsupported { tables } => {
                vec![("tables", tables.join(", "))]
            }
            Self::RlsPredicateRejected { table } => vec![("table", table.clone())],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_codes_match_catalog() {
        let all = vec![
            DenialReason::DdlForbidden,
            DenialReason::UnsupportedStatement,
            DenialReason::TableExtractFailed,
            DenialReason::SqlParseFailed,
            DenialReason::TableNotInPolicy {
                table: "t".into(),
                action: "a".into(),
            },
            DenialReason::RoleNotDefined { role: "r".into() },
            DenialReason::RlsParseFailed,
            DenialReason::RlsMultiTableUnsupported {
                tables: vec!["t".into()],
            },
            DenialReason::RlsPredicateRejected { table: "t".into() },
        ];
        for r in all {
            let key = r.message_key();
            assert!(key.starts_with("denial-reason-"));
            assert!(!r.reason_code().is_empty());
        }
    }
}
