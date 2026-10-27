// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 字段级自动脱敏与行级安全（`data-protection` feature）
//!
//! - `MaskingEngine`：按列名的声明式脱敏规则（掩码/SHA-256 哈希/截断），
//!   在行查询出口（`Session::query_rows`）统一应用。
//! - `RlsPolicy`：行级安全谓词注入（租户/角色条件 append 到 WHERE），
//!   admin 角色或显式 bypass 授权可绕过。

use sha2::{Digest, Sha256};

use crate::database::sql_text::{SqlBackend, escape_sql_literal_content};
use crate::i18n;

// 字段级脱敏

/// 脱敏策略
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskStrategy {
    /// 掩码：保留首尾各 keep 个字符，中间以 `*` 替代（字符串列；数值列转字符串后掩码）
    /// 保留首尾明文长度
    Mask {
        /// 保留首尾明文长度
        keep: usize,
    },
    /// SHA-256 十六进制哈希（不可逆）
    Hash,
    /// 截断到 n 字符（尾部追加省略号）
    Truncate(usize),
}

/// 单列脱敏规则
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskingRule {
    /// 列名（对 JSON 行的顶层 key 匹配）
    pub column: String,
    /// 应用策略
    pub strategy: MaskStrategy,
}

/// 脱敏引擎：列名 → 策略映射
#[derive(Debug, Clone, Default)]
pub struct MaskingEngine {
    rules: Vec<MaskingRule>,
}

impl MaskingEngine {
    /// 创建引擎
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// 追加规则（同列后写覆盖先写）
    pub fn rule(mut self, column: impl Into<String>, strategy: MaskStrategy) -> Self {
        let column = column.into();
        self.rules.retain(|r| r.column != column);
        self.rules.push(MaskingRule { column, strategy });
        self
    }

    /// 是否有规则
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// 对单值应用策略
    pub fn apply_value(
        &self,
        strategy: &MaskStrategy,
        value: &serde_json::Value,
    ) -> serde_json::Value {
        let s = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => return serde_json::Value::Null,
            other => other.to_string(),
        };
        let out = match strategy {
            MaskStrategy::Mask { keep } => {
                let chars: Vec<char> = s.chars().collect();
                let total = chars.len();
                let keep = (*keep).min(total / 2);
                if total > keep * 2 {
                    let mut out2: String = chars[..keep].iter().collect();
                    for _ in 0..(total - keep * 2) {
                        out2.push('*');
                    }
                    out2.extend(chars[total - keep..].iter());
                    out2
                } else {
                    "*".repeat(total)
                }
            }
            MaskStrategy::Hash => {
                let mut hasher = Sha256::new();
                hasher.update(s.as_bytes());
                let digest = hasher.finalize();
                hex_encode(&digest)
            }
            MaskStrategy::Truncate(n) => {
                let mut out2: String = s.chars().take(*n).collect();
                if s.chars().count() > *n {
                    out2.push('…');
                }
                out2
            }
        };
        serde_json::Value::String(out)
    }

    /// 对一批查询结果行应用脱敏（命中规则的字符串/数值列被转换）
    pub fn apply(&self, rows: &mut [serde_json::Value]) {
        if self.rules.is_empty() {
            return;
        }
        for row in rows.iter_mut() {
            let Some(obj) = row.as_object_mut() else {
                continue;
            };
            for rule in &self.rules {
                if let Some(v) = obj.get_mut(&rule.column) {
                    let masked = self.apply_value(&rule.strategy, v);
                    *v = masked;
                }
            }
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// 行级安全（RLS）

/// 行级安全策略：对指定表的语句注入 `<column> = <value>` 谓词
///
/// 谓词列名必须通过标识符白名单；谓词值经后端口径转义后作为字面量写入
/// SQL 文本（字面量在 PostgreSQL 中为 unknown 类型，由列类型决定，跨
/// text/uuid/int 等列类型通用；绑定参数会被声明为 TEXT，非文本列无匹配
/// 操作符）。注入范围：读出口（`query_rows`）与 raw 写出口的
/// SELECT/UPDATE/DELETE。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RlsPolicy {
    /// 目标表
    pub table: String,
    /// 谓词列（如 tenant_id）
    pub column: String,
    /// 谓词值（如租户 ID）
    pub value: String,
}

/// RLS 引擎：策略集 + 绕过控制
#[derive(Debug, Clone, Default)]
pub struct RlsEngine {
    policies: Vec<RlsPolicy>,
}

impl RlsEngine {
    /// 创建引擎
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    /// 追加策略
    pub fn policy(
        mut self,
        table: impl Into<String>,
        column: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.policies.push(RlsPolicy {
            table: table.into(),
            column: column.into(),
            value: value.into(),
        });
        self
    }

    /// 是否有策略
    pub fn is_empty(&self) -> bool {
        self.policies.is_empty()
    }

    /// 对 SQL 注入 RLS 谓词（谓词值经后端转义后写为字面量）
    ///
    /// # 安全边界
    ///
    /// - admin 角色**不**注入（显式管理通道，与 `admin_bypass` 语义一致）
    /// - 谓词列名必须通过标识符白名单，否则 fail-closed 报错
    /// - 谓词值经 [`SqlBackend`] 口径转义（MySQL 默认模式先加倍反斜杠），
    ///   写为字符串字面量：字面量在 PostgreSQL 中是 unknown 类型，由列类型
    ///   决定，text/uuid/int 等列类型通用；绑定参数会被声明为 TEXT，非文本
    ///   列无匹配操作符（`uuid = text`）
    /// - WHERE 位置按顶层子句边界判定（GROUP BY/HAVING/ORDER BY/LIMIT 之前），
    ///   不依赖 `contains(" where ")` 之类会把谓词追加到 ORDER BY 之后的启发式
    ///
    /// # Errors
    ///
    /// 谓词列名不合法或语句含顶层集合操作（UNION/INTERSECT/EXCEPT，
    /// 单表谓词无法覆盖全部分支）时返回 [`RlsError`]。
    pub fn inject(
        &self,
        sql: &str,
        primary_table: Option<&str>,
        backend: SqlBackend,
    ) -> Result<String, RlsError> {
        if self.policies.is_empty() {
            return Ok(sql.to_string());
        }
        let Some(table) = primary_table else {
            return Ok(sql.to_string());
        };
        let Some(policy) = self.policies.iter().find(|p| p.table == table) else {
            return Ok(sql.to_string());
        };
        if !crate::database::identifier::is_safe_identifier(&policy.column) {
            return Err(RlsError::UnsafePolicyColumn {
                table: policy.table.clone(),
                column: policy.column.clone(),
            });
        }
        let predicate = format!(
            "{} = '{}'",
            policy.column,
            escape_sql_literal_content(&policy.value, backend)
        );
        insert_predicate(sql, &predicate, backend)
    }
}

/// RLS 注入失败（fail-closed：无法安全判定时拒绝执行，而非静默放行）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RlsError {
    /// 策略列名未通过标识符白名单
    UnsafePolicyColumn {
        /// 策略目标表
        table: String,
        /// 非法列名
        column: String,
    },
    /// 语句含顶层集合操作（UNION/INTERSECT/EXCEPT），单表谓词覆盖不全部分支
    UnsupportedSetOperation,
}

impl std::fmt::Display for RlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsafePolicyColumn { table, column } => write!(
                f,
                "{}",
                i18n::t(
                    "rls-unsafe-predicate-column",
                    &[("table", table.clone()), ("column", column.clone())],
                )
            ),
            Self::UnsupportedSetOperation => {
                write!(f, "{}", i18n::t_simple("rls-set-operations-unsupported"))
            }
        }
    }
}

impl std::error::Error for RlsError {}

/// 把谓词插入语句的 WHERE 位置
///
/// 插入点按顶层子句边界判定：已有 WHERE 时并入该 WHERE 子句末尾，
/// 无 WHERE 时插在首个后续子句（GROUP BY/HAVING/ORDER BY/LIMIT…）之前。
/// 字符串字面量、引用标识符、注释与括号内的子句均不参与判定。
fn insert_predicate(sql: &str, predicate: &str, backend: SqlBackend) -> Result<String, RlsError> {
    let trimmed = sql.trim().trim_end_matches(';').trim_end();
    let scan = scan_top_level(trimmed, backend);
    if scan.set_op.is_some() {
        return Err(RlsError::UnsupportedSetOperation);
    }
    let (insert_at, connector) = match scan.where_pos {
        Some(where_pos) => {
            // WHERE 子句在下一个顶层终结子句处结束
            let end = scan
                .terminators
                .iter()
                .copied()
                .find(|pos| *pos > where_pos)
                .unwrap_or(trimmed.len());
            (end, "AND")
        }
        None => {
            let end = scan.terminators.first().copied().unwrap_or(trimmed.len());
            (end, "WHERE")
        }
    };
    let (prefix, suffix) = trimmed.split_at(insert_at);
    let prefix = prefix.trim_end();
    let suffix = suffix.trim_start();
    let mut out = String::with_capacity(trimmed.len() + predicate.len() + 8);
    out.push_str(prefix);
    out.push(' ');
    out.push_str(connector);
    out.push(' ');
    out.push_str(predicate);
    if !suffix.is_empty() {
        out.push(' ');
        out.push_str(suffix);
    }
    Ok(out)
}

/// 顶层子句扫描结果
struct TopLevelScan {
    /// 首个顶层 WHERE 关键字起始位置
    where_pos: Option<usize>,
    /// 顶层终结子句（GROUP BY/HAVING/ORDER BY/LIMIT/OFFSET/WINDOW/FETCH/FOR）位置
    terminators: Vec<usize>,
    /// 顶层集合操作（UNION/INTERSECT/EXCEPT）位置
    set_op: Option<usize>,
}

/// 扫描语句顶层子句边界（跳过字面量、引用标识符、注释与括号内子句）
fn scan_top_level(sql: &str, backend: SqlBackend) -> TopLevelScan {
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let mut depth: i32 = 0;
    let mut where_pos: Option<usize> = None;
    let mut terminators: Vec<usize> = Vec::new();
    let mut set_op: Option<usize> = None;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'\'' | b'"' | b'`' => {
                i = skip_quoted(sql, i, c, backend);
                continue;
            }
            b'$' => {
                if let Some(end) = skip_dollar_quoted(sql, i) {
                    i = end;
                    continue;
                }
            }
            b'(' => {
                depth += 1;
                i += 1;
                continue;
            }
            b')' => {
                depth -= 1;
                i += 1;
                continue;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                i = skip_line_comment(sql, i);
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = skip_block_comment(sql, i);
                continue;
            }
            _ => {}
        }
        if depth == 0 && (c == b'_' || c.is_ascii_alphabetic()) {
            let start = i;
            let mut j = i;
            while j < bytes.len()
                && (bytes[j] == b'_' || bytes[j] == b'$' || bytes[j].is_ascii_alphanumeric())
            {
                j += 1;
            }
            match sql[start..j].to_ascii_uppercase().as_str() {
                "WHERE" => {
                    if where_pos.is_none() {
                        where_pos = Some(start);
                    }
                }
                // RETURNING 与其它终结子句同级：谓词必须落在 RETURNING 之前，
                // 否则会被追加进 RETURNING 的表达式列表，行级过滤静默失效。
                "GROUP" | "HAVING" | "ORDER" | "LIMIT" | "OFFSET" | "WINDOW" | "FETCH" | "FOR"
                | "RETURNING" => {
                    terminators.push(start);
                }
                "UNION" | "INTERSECT" | "EXCEPT" if set_op.is_none() => {
                    set_op = Some(start);
                }
                _ => {}
            }
            i = j;
            continue;
        }
        i += 1;
    }
    TopLevelScan {
        where_pos,
        terminators,
        set_op,
    }
}

/// 跳过引号包裹片段（字符串字面量 / 引用标识符），返回片段结束后的位置
///
/// 转义口径按后端分化：
/// - 连续两个引号表示一个引号（SQL 标准，各方言通用）；
/// - MySQL 默认模式（`NO_BACKSLASH_ESCAPES` 关闭）下 `\` 是转义符，
///   字符串字面量内的 `\'` 不会结束字面量；
/// - PostgreSQL 转义串（`E'...'`）内 `\` 是转义符；普通串在
///   standard_conforming_strings=on 下 `\` 是普通字符。
fn skip_quoted(sql: &str, start: usize, quote: u8, backend: SqlBackend) -> usize {
    let bytes = sql.as_bytes();
    let backslash_escape = match backend {
        // MySQL：字符串字面量（单引号/双引号）内反斜杠是转义符；
        // 反引号标识符只支持双写转义
        SqlBackend::MySql => quote == b'\'' || quote == b'"',
        // SQL 标准口径：仅 PostgreSQL 转义串（E'...'）内反斜杠是转义符
        SqlBackend::Standard => quote == b'\'' && is_escape_string_prefix(sql, start),
    };
    let mut i = start + 1;
    while i < bytes.len() {
        if backslash_escape && bytes[i] == b'\\' {
            // 反斜杠转义下一个字符（该字符不参与引号闭合判定）
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            if bytes.get(i + 1) == Some(&quote) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

/// 判断引号前是否为 PostgreSQL 转义串前缀（`E'...'`）
///
/// `E` 紧邻引号且其前不是标识符字符时，转义串成立；否则按普通字面量处理。
fn is_escape_string_prefix(sql: &str, quote_start: usize) -> bool {
    let bytes = sql.as_bytes();
    if quote_start == 0 {
        return false;
    }
    let prefix = bytes[quote_start - 1];
    if prefix != b'E' && prefix != b'e' {
        return false;
    }
    if quote_start == 1 {
        return true;
    }
    let before = bytes[quote_start - 2];
    !(before.is_ascii_alphanumeric() || before == b'_' || before == b'$')
}

/// 跳过 PostgreSQL dollar-quoted 片段（`$$...$$` / `$tag$...$tag$`）
///
/// 非 dollar-quoted 形态（如绑定占位符 `$1`）返回 `None`。
fn skip_dollar_quoted(sql: &str, start: usize) -> Option<usize> {
    let bytes = sql.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric()) {
        i += 1;
    }
    if bytes.get(i) != Some(&b'$') {
        return None;
    }
    let tag = &sql[start..=i];
    match sql[i + 1..].find(tag) {
        Some(pos) => Some(i + 1 + pos + tag.len()),
        None => Some(bytes.len()),
    }
}

/// 跳过行注释（`--` 至行尾）
fn skip_line_comment(sql: &str, start: usize) -> usize {
    match sql[start..].find('\n') {
        Some(pos) => start + pos + 1,
        None => sql.len(),
    }
}

/// 跳过块注释（`/* ... */`）
fn skip_block_comment(sql: &str, start: usize) -> usize {
    match sql[start + 2..].find("*/") {
        Some(pos) => start + 2 + pos + 2,
        None => sql.len(),
    }
}

use std::sync::Arc;
use tokio::sync::RwLock;

/// 数据保护配置（脱敏 + RLS），运行时可整体换装（ArcSwap 换装由池侧注入）
#[derive(Debug, Clone, Default)]
pub struct DataProtection {
    /// 字段脱敏引擎（None = 不脱敏）
    pub masking: Option<Arc<MaskingEngine>>,
    /// 行级安全引擎（None = 不注入谓词）
    pub rls: Option<Arc<RlsEngine>>,
}

impl DataProtection {
    /// 只读快照（无锁读取）
    pub fn load(arc: &Arc<RwLock<DataProtection>>) -> DataProtection {
        arc.blocking_read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_masking_hash_strategy() {
        let engine = MaskingEngine::new().rule("email", MaskStrategy::Hash);
        let mut rows = vec![json!({"email": "alice@example.com"})];
        engine.apply(&mut rows);
        let masked = rows[0]["email"].as_str().unwrap();
        // sha2("alice@example.com") 前缀
        assert_eq!(
            masked.len(),
            64,
            "the SHA-256 hex digest should be 64 characters"
        );
        assert!(!masked.contains("alice"));
        assert!(!masked.contains('@'));
    }

    #[test]
    fn test_masking_mask_and_truncate() {
        let engine = MaskingEngine::new()
            .rule("phone", MaskStrategy::Mask { keep: 3 })
            .rule("bio", MaskStrategy::Truncate(5));
        let mut rows = vec![json!({"phone": "13812345678", "bio": "很长的个人简介内容"})];
        engine.apply(&mut rows);
        let phone = rows[0]["phone"].as_str().unwrap();
        assert!(
            phone.starts_with("138"),
            "the first 3 digits should be kept"
        );
        assert!(phone.contains('*'), "the middle should be masked");
        let bio = rows[0]["bio"].as_str().unwrap();
        assert!(
            bio.chars().count() <= 6,
            "truncation to 5 characters plus the ellipsis"
        );
        assert!(bio.ends_with('…'));
    }

    #[test]
    fn test_masking_untouched_columns() {
        let engine = MaskingEngine::new().rule("secret", MaskStrategy::Hash);
        let mut rows = vec![json!({"id": 1, "name": "keep"})];
        engine.apply(&mut rows);
        assert_eq!(rows[0]["name"], "keep");
        assert_eq!(rows[0]["id"], 1);
    }

    #[test]
    fn test_rls_injection() {
        let rls = RlsEngine::new().policy("orders", "tenant_id", "t-100");
        // 无 WHERE → append WHERE（值经转义后写为字面量）
        let out = rls
            .inject("SELECT * FROM orders", Some("orders"), SqlBackend::Standard)
            .unwrap();
        assert_eq!(out, "SELECT * FROM orders WHERE tenant_id = 't-100'");
        // 已有 WHERE → AND 连接
        let out2 = rls
            .inject(
                "SELECT * FROM orders WHERE amount > 10",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out2,
            "SELECT * FROM orders WHERE amount > 10 AND tenant_id = 't-100'"
        );
        // 非目标表不注入
        assert_eq!(
            rls.inject("SELECT * FROM users", Some("users"), SqlBackend::Standard)
                .unwrap(),
            "SELECT * FROM users"
        );
        // 无主表不注入
        assert_eq!(
            rls.inject("SELECT 1", None, SqlBackend::Standard).unwrap(),
            "SELECT 1"
        );
    }

    /// 谓词值必须按后端口径转义：引号/反斜杠不得击穿字面量边界
    #[test]
    fn test_rls_escapes_policy_value_per_backend() {
        let rls = RlsEngine::new().policy("orders", "tenant_id", "o'brien");
        let out = rls
            .inject("SELECT * FROM orders", Some("orders"), SqlBackend::Standard)
            .unwrap();
        assert_eq!(out, "SELECT * FROM orders WHERE tenant_id = 'o''brien'");

        // MySQL 默认模式：反斜杠是转义符，需先加倍
        let rls = RlsEngine::new().policy("orders", "tenant_id", "a\\");
        let out = rls
            .inject("SELECT * FROM orders", Some("orders"), SqlBackend::MySql)
            .unwrap();
        assert_eq!(out, "SELECT * FROM orders WHERE tenant_id = 'a\\\\'");
    }

    /// 谓词必须落在 WHERE 子句内：ORDER BY / GROUP BY / LIMIT 之前，
    /// 否则谓词会被解析成排序键而静默失效
    #[test]
    fn test_rls_predicate_inserted_before_trailing_clauses() {
        let rls = RlsEngine::new().policy("orders", "tenant_id", "t-100");
        let out = rls
            .inject(
                "SELECT id FROM orders WHERE amount > 5 ORDER BY id",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out,
            "SELECT id FROM orders WHERE amount > 5 AND tenant_id = 't-100' ORDER BY id"
        );

        let out = rls
            .inject(
                "SELECT id FROM orders ORDER BY id",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out,
            "SELECT id FROM orders WHERE tenant_id = 't-100' ORDER BY id"
        );

        let out = rls
            .inject(
                "SELECT id FROM orders LIMIT 10",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out,
            "SELECT id FROM orders WHERE tenant_id = 't-100' LIMIT 10"
        );

        let out = rls
            .inject(
                "SELECT id FROM orders GROUP BY id ORDER BY id",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out,
            "SELECT id FROM orders WHERE tenant_id = 't-100' GROUP BY id ORDER BY id"
        );
    }

    /// RETURNING 与其它终结子句同级：谓词必须落在 RETURNING 之前。
    /// 追加到语句末尾会落进 RETURNING 的表达式列表，行级过滤静默失效——
    /// 跨租户的行会被 UPDATE/DELETE 命中。
    #[test]
    fn test_rls_predicate_inserted_before_returning() {
        let rls = RlsEngine::new().policy("orders", "tenant_id", "t-100");

        let out = rls
            .inject(
                "UPDATE orders SET amount = 1 WHERE id = 5 RETURNING id",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out,
            "UPDATE orders SET amount = 1 WHERE id = 5 AND tenant_id = 't-100' RETURNING id"
        );

        let out = rls
            .inject(
                "DELETE FROM orders WHERE id = 5 RETURNING id",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out,
            "DELETE FROM orders WHERE id = 5 AND tenant_id = 't-100' RETURNING id"
        );

        // 无 WHERE 时谓词同样必须插在 RETURNING 之前。
        let out = rls
            .inject(
                "UPDATE orders SET amount = 1 RETURNING id",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out,
            "UPDATE orders SET amount = 1 WHERE tenant_id = 't-100' RETURNING id"
        );
    }

    /// 字面量/子查询里的 where 不得影响谓词插入位置判定
    #[test]
    fn test_rls_ignores_where_inside_literals_and_subqueries() {
        let rls = RlsEngine::new().policy("orders", "tenant_id", "t-100");
        let out = rls
            .inject(
                "SELECT id FROM orders WHERE note = 'a where b' ORDER BY id",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out,
            "SELECT id FROM orders WHERE note = 'a where b' AND tenant_id = 't-100' ORDER BY id"
        );

        // 子查询内的 WHERE 属于括号内子句，不算顶层 WHERE
        let out = rls
            .inject(
                "SELECT id FROM (SELECT id FROM audit WHERE x = 1) t ORDER BY id",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out,
            "SELECT id FROM (SELECT id FROM audit WHERE x = 1) t WHERE tenant_id = 't-100' ORDER BY id"
        );
    }

    /// 反斜杠转义引号不得提前闭合字面量：否则 ORDER BY 被误判为字面量内容，
    /// 谓词会被追加到 ORDER BY 之后而静默失效
    #[test]
    fn test_rls_handles_backslash_escaped_quotes() {
        let rls = RlsEngine::new().policy("orders", "tenant_id", "t-100");

        // MySQL 默认模式：\' 不闭合字面量
        let out = rls
            .inject(
                "SELECT id FROM orders WHERE note = 'a\\'b' ORDER BY id",
                Some("orders"),
                SqlBackend::MySql,
            )
            .unwrap();
        assert_eq!(
            out,
            "SELECT id FROM orders WHERE note = 'a\\'b' AND tenant_id = 't-100' ORDER BY id"
        );

        // PostgreSQL 转义串（E'...'）：反斜杠是转义符
        let out = rls
            .inject(
                "SELECT id FROM orders WHERE note = E'a\\'b' ORDER BY id",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap();
        assert_eq!(
            out,
            "SELECT id FROM orders WHERE note = E'a\\'b' AND tenant_id = 't-100' ORDER BY id"
        );
    }

    /// 列名不在标识符白名单内 → fail-closed 报错（不静默放行）
    #[test]
    fn test_rls_rejects_unsafe_policy_column() {
        let rls = RlsEngine::new().policy("orders", "1=1 --", "t-100");
        let err = rls
            .inject("SELECT * FROM orders", Some("orders"), SqlBackend::Standard)
            .unwrap_err();
        assert!(matches!(err, RlsError::UnsafePolicyColumn { .. }));
    }

    /// 顶层集合操作无法用单表谓词覆盖 → fail-closed 报错
    #[test]
    fn test_rls_rejects_set_operations() {
        let rls = RlsEngine::new().policy("orders", "tenant_id", "t-100");
        let err = rls
            .inject(
                "SELECT id FROM orders UNION SELECT id FROM orders",
                Some("orders"),
                SqlBackend::Standard,
            )
            .unwrap_err();
        assert_eq!(err, RlsError::UnsupportedSetOperation);
    }
}
