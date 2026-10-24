// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! SQL 文本直拼安全口径
//!
//! 字符串字面量以文本直拼进 SQL 的场景（无法参数化的位置：RLS 租户谓词、
//! 实体字面量等），转义口径必须按后端区分——反斜杠在 MySQL 默认模式与
//! SQL 标准（PostgreSQL/SQLite）下语义不同。独立于各仓储/访问模块的
//! feature 门控，保证全库共用同一套口径。

/// 字符串字面量转义的数据库后端口径
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlBackend {
    /// MySQL：先加倍反斜杠，再加倍单引号
    MySql,
    /// SQL 标准口径（PostgreSQL / SQLite / DuckDB）：仅加倍单引号
    Standard,
}

/// 按后端转义字符串字面量内容（不含外层引号）
///
/// 反斜杠语义按后端分化：MySQL 默认模式（`NO_BACKSLASH_ESCAPES` 关闭）下
/// `\` 是转义符，仅加倍单引号可被 `\'` 序列击穿（注入向量）；PostgreSQL
/// （standard_conforming_strings=on）、SQLite、DuckDB 遵循 SQL 标准，
/// `\` 是字面量，加倍会引入多余字符。
pub fn escape_sql_literal_content(s: &str, backend: SqlBackend) -> String {
    match backend {
        SqlBackend::MySql => s.replace('\\', "\\\\").replace('\'', "''"),
        SqlBackend::Standard => s.replace('\'', "''"),
    }
}
