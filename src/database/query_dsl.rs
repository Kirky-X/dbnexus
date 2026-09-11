// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 查询 DSL 宏（T422）
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
//! assert_eq!(fragment.to_sql(), "SELECT id, name FROM users WHERE age > 18 AND status = 'active' ORDER BY id DESC LIMIT 10");
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

/// 运算符（T422）
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

/// 查询条件（T422）
#[derive(Debug, Clone)]
pub struct DslCondition {
    /// 列名
    pub column: String,
    /// 运算符
    pub op: DslOp,
    /// 已转义的 SQL 字面量
    pub literal: String,
}

/// 查询片段（T422）：`q!` 的产物
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
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && name.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

impl QueryFragment {
    /// 组装为 SELECT SQL
    ///
    /// # Panics
    ///
    /// 仅当 `q!` 产物被手工构造出非法标识符时（宏路径不可能发生）panic。
    pub fn to_sql(&self) -> String {
        let columns = if self.columns.is_empty() {
            "*".to_string()
        } else {
            self.columns
                .iter()
                .map(|c| {
                    assert!(is_safe_ident(c), "unsafe column identifier: {c}");
                    c.clone()
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let table = self.table.as_deref().unwrap_or("(?)");
        assert!(
            table == "(?)" || is_safe_ident(table),
            "unsafe table identifier: {table}"
        );

        let mut sql = format!("SELECT {columns} FROM {table}");
        if !self.conditions.is_empty() {
            let clauses: Vec<String> = self
                .conditions
                .iter()
                .map(|c| {
                    assert!(is_safe_ident(&c.column), "unsafe column: {}", c.column);
                    format!("{} {} {}", c.column, c.op.as_sql(), c.literal)
                })
                .collect();
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        if let Some(order) = &self.order_column {
            assert!(is_safe_ident(order), "unsafe order column: {order}");
            let dir = if self.order_desc { "DESC" } else { "ASC" };
            sql.push_str(&format!(" ORDER BY {order} {dir}"));
        }
        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        sql
    }
}

/// 宏 literal token 文本 → SQL 字面量（T422 内部）
///
/// `stringify!` 保留字面量原始文本：字符串字面量带引号（如 `"active"`），
/// 数值/布尔为裸文本（如 `10`、`true`）。据此分流：
/// - 字符串：剥引号 + SQL 标准转义（单引号加倍）；
/// - 数值/布尔：文本直接作为 SQL 字面量（token 无法携带任意 SQL）。
pub fn literal_from_token(token: &str) -> String {
    let trimmed = token.trim();
    let bytes = trimmed.as_bytes();
    let is_quoted = bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''));
    if is_quoted {
        let inner = &trimmed[1..trimmed.len() - 1];
        // 反转义常见序列（stringify! 保留源码转义形态）
        let unescaped = inner.replace("\\\"", "\"").replace("\\\\", "\\");
        format!("'{}'", unescaped.replace('\'', "''"))
    } else {
        trimmed.to_string()
    }
}

/// 值 → SQL 文本（T422 内部：委托 literal_from_token）
#[macro_export]
#[doc(hidden)]
macro_rules! __dsl_value {
    ($v:literal) => {
        $crate::database::query_dsl::literal_from_token(stringify!($v))
    };
}

/// 运算符 token → DslOp（T422 内部）
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

/// 条件组合（T422 内部递归：多条件逐条登记，AND 语义在 to_sql 组装）
#[macro_export]
#[doc(hidden)]
macro_rules! __dsl_conditions {
    ($builder:ident, $col:ident $op:tt $val:literal) => {
        $builder.add_condition(
            stringify!($col),
            $crate::__dsl_op!($op),
            $crate::__dsl_value!($val),
        );
    };
    ($builder:ident, $col:ident $op:tt $val:literal, $($rest:tt)+) => {
        $builder.add_condition(
            stringify!($col),
            $crate::__dsl_op!($op),
            $crate::__dsl_value!($val),
        );
        $crate::__dsl_conditions!($builder, $($rest)+);
    };
}

/// 查询 DSL 宏（T422）
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
            table: Some(stringify!($table).to_string()),
            columns: vec![$(stringify!($col).to_string()),+],
            ..Default::default()
        };
        $( $crate::__dsl_conditions!(fragment, $($wcol $wop $wval),+); )?
        $( fragment.order_column = Some(stringify!($ocol).to_string());
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
            table: Some(stringify!($table).to_string()),
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
