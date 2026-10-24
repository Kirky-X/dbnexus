// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 查询 DSL 宏
//!
//! `q!` 以声明式语法生成类型安全的 SELECT 查询片段：
//!
//! ```ignore
//! use dbnexus::q;
//!
//! let fragment = q! {
//!     select [id, name] from users
//!     where age > 18, status == "active"
//!     order by id desc
//!     limit 10
//! };
//! assert_eq!(fragment.to_sql().unwrap(), "SELECT id, name FROM users WHERE age > 18 AND status = 'active' ORDER BY id DESC LIMIT 10");
//! ```
//!
//! # 类型安全口径
//!
//! - 表名/列名来自 Rust 标识符（`ident` 片段）：语法上不可能携带引号、
//!   空格或分号，杜绝标识符注入；
//! - 比较值来自字面量（`literal` 片段）：按 SQL 标准转义后进入语句，
//!   无法逃逸为任意 SQL；
//! - 运算符为宏匹配的固定 token（`==`/`!=`/`>`/`<`/`>=`/`<=`），
//!   不接受任意文本；
//! - 多个条件以 `AND` 组合（条件组合 MVP：AND 语义）。
//!
//! 标识符白名单为 ASCII（字母/数字/下划线）：非 ASCII 标识符不满足白名单，
//! `to_sql()` 返回 `DbError::Config`（不 panic）。

/// 运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DslOp {
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `>`
    Gt,
    /// `<`
    Lt,
    /// `>=`
    Ge,
    /// `<=`
    Le,
}

impl DslOp {
    /// SQL 运算符文本
    pub fn as_sql(self) -> &'static str {
        match self {
            DslOp::Eq => "=",
            DslOp::Ne => "!=",
            DslOp::Gt => ">",
            DslOp::Lt => "<",
            DslOp::Ge => ">=",
            DslOp::Le => "<=",
        }
    }
}

/// 查询条件
#[derive(Debug, Clone)]
pub struct DslCondition {
    /// 列名
    pub column: String,
    /// 运算符
    pub op: DslOp,
    /// 已转义的 SQL 字面量
    pub literal: String,
}

/// 查询片段：`q!` 的产物
///
/// 持有结构化的表/列/条件/排序/分页信息；`to_sql()` 组装为 SELECT 语句。
/// 片段各部分均源自宏的 ident/literal 片段，天然免疫标识符与值注入。
#[derive(Debug, Clone, Default)]
pub struct QueryFragment {
    /// 表名
    pub table: Option<String>,
    /// 投影列（空 = 未指定，组装时退化为 `*`）
    pub columns: Vec<String>,
    /// 条件（AND 组合）
    pub conditions: Vec<DslCondition>,
    /// 排序列
    pub order_column: Option<String>,
    /// 排序方向（true = DESC）
    pub order_desc: bool,
    /// 行数上限
    pub limit: Option<u64>,
}

/// 标识符安全校验（与仓储/网关同口径：字母或下划线开头，仅含字母/数字/下划线）
pub fn is_safe_ident(name: &str) -> bool {
    crate::database::identifier::is_safe_identifier(name)
}

/// 标识符 token → 标识符文本（内部：供宏展开调用）
///
/// Rust 原始标识符（`r#type`）的 `stringify!` 文本带 `r#` 前缀，
/// 该前缀不属于 SQL 标识符，剥除后再做白名单校验。
pub fn ident_from_token(token: &str) -> String {
    token.strip_prefix("r#").unwrap_or(token).to_string()
}

impl QueryFragment {
    /// 组装为 SELECT SQL
    ///
    /// 标识符（表名/投影列/条件列/排序列）逐一经白名单校验；非法标识符
    /// 返回 `DbError::Config`，不 panic——公共 API 不得用 panic 表达输入
    /// 校验失败（外部输入进入标识符槽位即可触发线程 panic）。
    ///
    /// # Errors
    ///
    /// 任一标识符不在白名单内时返回 `DbError::Config`。
    pub fn to_sql(&self) -> crate::foundation::DbResult<String> {
        let columns = if self.columns.is_empty() {
            "*".to_string()
        } else {
            self.columns
                .iter()
                .map(|c| {
                    if !is_safe_ident(c) {
                        return Err(crate::foundation::DbError::Config(format!(
                            "query fragment column name must be a safe identifier: '{c}'"
                        )));
                    }
                    Ok(c.clone())
                })
                .collect::<Result<Vec<_>, crate::foundation::DbError>>()?
                .join(", ")
        };
        let table = self.table.as_deref().unwrap_or("(?)");
        if !(table == "(?)" || is_safe_ident(table)) {
            return Err(crate::foundation::DbError::Config(format!(
                "query fragment table name must be a safe identifier: '{table}'"
            )));
        }

        let mut sql = format!("SELECT {columns} FROM {table}");
        if !self.conditions.is_empty() {
            let mut clauses: Vec<String> = Vec::with_capacity(self.conditions.len());
            for c in &self.conditions {
                if !is_safe_ident(&c.column) {
                    return Err(crate::foundation::DbError::Config(format!(
                        "query fragment condition column must be a safe identifier: '{}'",
                        c.column
                    )));
                }
                clauses.push(format!("{} {} {}", c.column, c.op.as_sql(), c.literal));
            }
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        if let Some(order) = &self.order_column {
            if !is_safe_ident(order) {
                return Err(crate::foundation::DbError::Config(format!(
                    "query fragment order column must be a safe identifier: '{order}'"
                )));
            }
            let dir = if self.order_desc { "DESC" } else { "ASC" };
            sql.push_str(&format!(" ORDER BY {order} {dir}"));
        }
        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        Ok(sql)
    }
}

/// 解码 Rust 源码字符串转义序列（单遍扫描；`\` 后为未知转义时按原样保留）
fn unescape_rust_string(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('u') => {
                // `\u{...}` 形式：取花括号内十六进制码点；格式非法时原样回填
                let mut hex = String::new();
                let mut closed = false;
                if chars.next() == Some('{') {
                    for hc in chars.by_ref() {
                        if hc == '}' {
                            closed = true;
                            break;
                        }
                        hex.push(hc);
                    }
                }
                match (
                    closed,
                    u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32),
                ) {
                    (true, Some(ch)) => out.push(ch),
                    _ => {
                        out.push_str("\\u{");
                        out.push_str(&hex);
                        if closed {
                            out.push('}');
                        }
                    }
                }
            }
            Some(other) => {
                // 未知/不支持转义（\x.. 等）：保留原文，保证值不失真
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// 宏 literal token 文本 → SQL 字面量（内部）
///
/// `stringify!` 保留字面量原始文本：字符串字面量带引号（如 `"active"`），
/// 数值/布尔为裸文本（如 `10`、`true`）。据此分流：
/// - 字符串：剥引号 + 解码 Rust 转义序列 + SQL 标准转义（单引号加倍）；
/// - 数值/布尔：文本直接作为 SQL 字面量（token 无法携带任意 SQL）。
pub fn literal_from_token(token: &str) -> String {
    let trimmed = token.trim();
    let bytes = trimmed.as_bytes();
    let is_quoted = bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''));
    if is_quoted {
        let inner = &trimmed[1..trimmed.len() - 1];
        let unescaped = unescape_rust_string(inner);
        format!("'{}'", unescaped.replace('\'', "''"))
    } else {
        trimmed.to_string()
    }
}

/// 值 → SQL 文本（内部：委托 literal_from_token）
#[macro_export]
#[doc(hidden)]
macro_rules! __dsl_value {
    ($v:literal) => {
        $crate::database::query_dsl::literal_from_token(stringify!($v))
    };
}

/// 运算符 token → DslOp（内部）
#[macro_export]
#[doc(hidden)]
macro_rules! __dsl_op {
    (==) => {
        $crate::database::query_dsl::DslOp::Eq
    };
    (!=) => {
        $crate::database::query_dsl::DslOp::Ne
    };
    (>) => {
        $crate::database::query_dsl::DslOp::Gt
    };
    (<) => {
        $crate::database::query_dsl::DslOp::Lt
    };
    (>=) => {
        $crate::database::query_dsl::DslOp::Ge
    };
    (<=) => {
        $crate::database::query_dsl::DslOp::Le
    };
}

/// 条件组合（内部递归：多条件逐条登记，AND 语义在 to_sql 组装）
#[macro_export]
#[doc(hidden)]
macro_rules! __dsl_conditions {
    ($builder:ident, $col:ident $op:tt $val:literal) => {
        $builder.add_condition(
            &$crate::database::query_dsl::ident_from_token(stringify!($col)),
            $crate::__dsl_op!($op),
            $crate::__dsl_value!($val),
        );
    };
    ($builder:ident, $col:ident $op:tt $val:literal, $($rest:tt)+) => {
        $builder.add_condition(
            &$crate::database::query_dsl::ident_from_token(stringify!($col)),
            $crate::__dsl_op!($op),
            $crate::__dsl_value!($val),
        );
        $crate::__dsl_conditions!($builder, $($rest)+);
    };
}

/// 查询 DSL 宏
///
/// 语法（MVP）：
///
/// ```text
/// q! {
///     select [$($col:ident),+]
///     from $table:ident
///     $(where $($col:ident $op:tt $val:literal),+)?
///     $(order by $ocol:ident $dir:ident)?      // dir ∈ {asc, desc}
///     $(limit $n:literal)?
/// }
/// ```
///
/// 产物为 [`QueryFragment`]；`to_sql()` 输出参数化的安全 SELECT。
#[macro_export]
macro_rules! q {
    (select [$($col:ident),+ $(,)?] from $table:ident
     $(where $($wcol:ident $wop:tt $wval:literal),+ $(,)?)?
     $(order by $ocol:ident $odir:ident)?
     $(limit $lim:literal)?
     $(,)?
    ) => {{
        #[allow(unused_mut)]
        let mut fragment = $crate::database::query_dsl::QueryFragment {
            table: Some($crate::database::query_dsl::ident_from_token(stringify!($table))),
            columns: vec![$($crate::database::query_dsl::ident_from_token(stringify!($col))),+],
            ..Default::default()
        };
        $( $crate::__dsl_conditions!(fragment, $($wcol $wop $wval),+); )?
        $( fragment.order_column = Some($crate::database::query_dsl::ident_from_token(stringify!($ocol)));
           fragment.order_desc = match stringify!($odir) { "desc" => true, _ => false }; )?
        $( fragment.limit = Some($lim); )?
        fragment
    }};
    // 无投影列（SELECT *）形态
    (select * from $table:ident
     $(where $($wcol:ident $wop:tt $wval:literal),+ $(,)?)?
     $(order by $ocol:ident $odir:ident)?
     $(limit $lim:literal)?
     $(,)?
    ) => {{
        #[allow(unused_mut)]
        let mut fragment = $crate::database::query_dsl::QueryFragment {
            table: Some($crate::database::query_dsl::ident_from_token(stringify!($table))),
            ..Default::default()
        };
        $( $crate::__dsl_conditions!(fragment, $($wcol $wop $wval),+); )?
        $( fragment.order_column = Some(stringify!($ocol).to_string());
           fragment.order_desc = match stringify!($odir) { "desc" => true, _ => false }; )?
        $( fragment.limit = Some($lim); )?
        fragment
    }};
}

/// add_condition 的宿主（供宏展开调用；独立于 QueryFragment 保存可读性）
impl QueryFragment {
    /// 追加条件（宏内部使用；公开以支持程序化组装）
    pub fn add_condition(&mut self, column: &str, op: DslOp, literal: String) {
        self.conditions.push(DslCondition {
            column: column.to_string(),
            op,
            literal,
        });
    }
}
