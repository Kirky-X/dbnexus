// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! SQL 标识符白名单校验
//!
//! 标识符以文本直拼进 SQL 的场景（表名/列名等无法参数化的位置），
//! 必须先经本模块白名单校验；独立于各仓储/迁移模块的 feature 门控，
//! 保证全库共用同一套口径。

/// 校验 SQL 标识符（表名/列名）白名单：字母或下划线开头，仅含
/// 字母/数字/下划线，长度 1-64
pub(crate) fn is_safe_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && name.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
