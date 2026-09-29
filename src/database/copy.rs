// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 批量写入（`copy` feature）：COPY 封装 + 多值 INSERT 构建
//!
//! 两条批量写入路径，按后端能力择一：
//!
//! - **COPY 路径**：`CopyStatement` 构建期校验标识符（白名单）并生成
//!   `COPY ... FROM STDIN`（postgres 协议传输，经 sea-orm 复用 sqlx-postgres
//!   `PgPoolCopyExt`）或 `COPY ... FROM 'file'`（DuckDB CSV 文件导入）；
//!   `encode_copy_rows` / `encode_duckdb_copy_rows` 提供配套行编码。
//! - **多值 INSERT 路径**：`BatchInsertStatement::chunk_rows` 把行集按
//!   500 行分块（对齐 global_index 的分块范式）构建参数化
//!   `INSERT ... VALUES (...), (...)`——占位符绑定而非文本拼接，供无 COPY
//!   协议的后端（sqlite/mysql/duckdb）与已持有 SQL 执行通道的消费方
//!   （如 inklog 的批量日志落库）使用。
//!
//! # 契约（两条路径共用）
//!
//! - **注入面在构建期关闭**：标识符白名单校验（字母/数字/下划线/点）；
//!   数据值经编码（COPY）或绑定参数（batch_insert）传递，绝不拼入 SQL 文本
//! - **不静默退化**：`copy_in` 在无 COPY 能力的后端上显式报错，绝不退化为
//!   逐行 INSERT；batch_insert 的产出也始终是多值语句而非逐行语句
//! - **错误显性化**：空行集、行列数不匹配、非法标识符、单语句参数超限
//!   （列数 > `bind_param_limit`）均在构建期返回错误——`chunk_rows`
//!   返回 `Ok` 即保证**无构建期错误**；多语句分次执行不具备原子性，中途块
//!   失败时已执行块不会回滚，整体原子性由消费方的事务通道保证（sea-orm
//!   后端 `Session::begin_transaction`、duckdb 走
//!   `Session::execute_duckdb_transaction`）
//! - **sqlite 无 COPY 协议**：契约测试断言语句构建/编码可用且 `copy_in`
//!   显式拒绝

use crate::foundation::DbError;

/// 绑定参数单语句上限（按占位符方言取各后端保守下界）
///
/// 分块构建按 `min(块行数, 上限/列数)` 收缩，保证单语句占位符数不超限：
/// - `QMark`：32766 = SQLite 默认 `SQLITE_MAX_VARIABLE_NUMBER`（≥3.32；
///   本仓 libsqlite3-sys bundled）——同时是 mysql/duckdb 上限的保守下界
/// - `Dollar`：65535 = PostgreSQL Bind 协议参数编号 u16 上限
pub fn bind_param_limit(style: PlaceholderStyle) -> usize {
    match style {
        PlaceholderStyle::QMark => 32_766,
        PlaceholderStyle::Dollar => 65_535,
    }
}

/// 多值 INSERT 默认分块行数（对齐 global_index 的 `BATCH_SYNC_CHUNK_SIZE`
/// 范式）；实际分块大小为 `min(本值, bind_param_limit(style)/列数)`——
/// 宽表场景自动收缩行数，单语句占位符数恒不超后端绑定上限
pub const BATCH_INSERT_CHUNK_SIZE: usize = 500;

/// 多值 INSERT 占位符风格（按后端方言选择）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderStyle {
    /// `?` 占位（sqlite / mysql / duckdb）
    QMark,
    /// `$N` 编号占位（postgres；编号跨行连续，全局唯一）
    Dollar,
}

/// COPY 数据格式（MVP：仅 text 格式；CSV/binary 留扩展）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyFormat {
    /// PG 文本格式（默认）
    Text,
}

/// COPY FROM STDIN 语句
///
/// 标识符在构建期经白名单校验（字母/数字/下划线/点），生成语句为
/// `COPY "<table>" ("<col>", ...) FROM STDIN`；双引号包裹标识符使保留字
/// 与大小写差异安全，注入面在构建期关闭。
#[derive(Debug, Clone)]
pub struct CopyStatement {
    table: String,
    columns: Vec<String>,
    format: CopyFormat,
}

impl CopyStatement {
    /// 构建 COPY 语句对象（表名/列名非法时返回错误）
    pub fn new(table: &str, columns: &[String]) -> Result<Self, DbError> {
        validate_identifier(table).map_err(|_| {
            DbError::Config(format!(
                "COPY target table identifier is invalid: '{table}' \
                 (allowed: letters/digits/underscore/dot)"
            ))
        })?;
        let mut cols = Vec::with_capacity(columns.len());
        for c in columns {
            validate_identifier(c).map_err(|_| {
                DbError::Config(format!(
                    "COPY column identifier is invalid: '{c}' \
                     (allowed: letters/digits/underscore/dot)"
                ))
            })?;
            cols.push(c.clone());
        }
        if cols.is_empty() {
            return Err(DbError::Config(
                "COPY requires at least one column (MVP contract)".to_string(),
            ));
        }
        Ok(Self {
            table: table.to_string(),
            columns: cols,
            format: CopyFormat::Text,
        })
    }

    /// 数据格式（MVP 恒为 Text）
    pub fn format(&self) -> CopyFormat {
        self.format
    }

    /// 目标表名
    pub fn table(&self) -> &str {
        &self.table
    }

    /// 目标列名
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// 生成 COPY 语句文本
    pub fn build(&self) -> String {
        let cols = self
            .columns
            .iter()
            .map(|c| quote_identifier(c))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "COPY {} ({}) FROM STDIN",
            quote_identifier(&self.table),
            cols
        )
    }

    /// 生成 DuckDB CSV 文件导入语句
    ///
    /// 输出 `COPY "<table>" ("<col>", ...) FROM '<path>' (FORMAT CSV, ...)`，
    /// 选项串与 [`encode_duckdb_copy_rows`] 的编码契约**一一配对**：
    ///
    /// - `NULLSTR ''` + `ALLOW_QUOTED_NULLS false`：未引用空字段 = NULL，
    ///   引用空字段 `""` = 空字符串（编码层正是以此区分两种语义）
    /// - `DELIMITER ','` + `QUOTE '"'` + `ESCAPE '"'`：标准 CSV 转义
    /// - `HEADER 0`：载荷无表头行
    ///
    /// 路径内单引号翻倍转义（`'` → `''`），语句逃逸面关闭；表/列标识符
    /// 已在 [`Self::new`] 经白名单校验。
    ///
    /// # 执行
    ///
    /// 该语句面向 duckdb 驱动（`DbPool::copy_in` 的 duckdb 路径内部即
    /// 本方法 + [`encode_duckdb_copy_rows`] + 临时文件传输）；postgres 的
    /// 文件导入方言选项名不同，请使用 [`Self::build`]（STDIN 协议）。
    pub fn build_from_file(&self, path: &str) -> Result<String, DbError> {
        if path.is_empty() {
            return Err(DbError::Config(
                "COPY file path must not be empty".to_string(),
            ));
        }
        let cols = self
            .columns
            .iter()
            .map(|c| quote_identifier(c))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(format!(
            "COPY {} ({}) FROM '{}' (FORMAT CSV, HEADER 0, DELIMITER ',', \
             QUOTE '\"', ESCAPE '\"', NULLSTR '', ALLOW_QUOTED_NULLS false)",
            quote_identifier(&self.table),
            cols,
            path.replace('\'', "''")
        ))
    }
}

/// 标识符白名单校验（字母/数字/下划线/点；不允许引号/空白/反斜杠/分号等）
fn validate_identifier(name: &str) -> Result<(), ()> {
    if name.is_empty() {
        return Err(());
    }
    let first = name.chars().next().unwrap();
    let valid = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.';
    if !(first.is_ascii_alphabetic() || first == '_') || !name.chars().all(valid) {
        return Err(());
    }
    // 不允许连续点（空段）
    if name.contains("..") || name.starts_with('.') || name.ends_with('.') {
        return Err(());
    }
    Ok(())
}

/// 标识符双引号包裹（内嵌双引号翻倍转义；输入已经白名单校验）
fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// 行数据 → PG text COPY 载荷（行尾 `\n`；列间 `\t`；NULL=`\N`）
///
/// 转义规则（PostgreSQL 文档 COPY text 格式）：`\\`、`\t`、`\n`、`\r`。
/// 非字符串值经 JSON 文本化后原样嵌入（数值/布尔安全；JSON 对象/数组
/// 作为文本列时经转义嵌入）。字段缓冲跨行复用，无需转义的字段走借用
/// 视图零拷贝（与 [`encode_duckdb_copy_rows`] 同范式）。
pub fn encode_copy_rows(rows: &[Vec<serde_json::Value>]) -> String {
    // 容量粗估（典型短字段 ~16B + 分隔）：仅避免冷启动扩容，无需精确
    let mut out = String::with_capacity(rows.len() * 32);
    // 字段缓冲跨行复用：clear 保留已分配容量，行间零重分配
    let mut fields: Vec<std::borrow::Cow<'_, str>> = Vec::new();
    for row in rows {
        fields.clear();
        fields.extend(row.iter().map(encode_copy_value));
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                out.push('\t');
            }
            out.push_str(field);
        }
        out.push('\n');
    }
    out
}

/// 单值 → PG text COPY 字段
fn encode_copy_value(value: &serde_json::Value) -> std::borrow::Cow<'_, str> {
    match value {
        serde_json::Value::Null => std::borrow::Cow::Borrowed("\\N"),
        serde_json::Value::String(s) => escape_copy_text(s),
        // 数值/布尔文本化后不含特殊字符，原样嵌入；JSON 对象/数组含
        // 逗号/引号等，按 PG text 规则转义
        other => {
            let text = other.to_string();
            if copy_text_needs_escape(&text) {
                std::borrow::Cow::Owned(escape_copy_text_owned(&text))
            } else {
                std::borrow::Cow::Owned(text)
            }
        }
    }
}

/// PG text 转义（`\\` `\t` `\n` `\r`）；无需转义的输入返回借用视图零拷贝
fn escape_copy_text(s: &str) -> std::borrow::Cow<'_, str> {
    if copy_text_needs_escape(s) {
        std::borrow::Cow::Owned(escape_copy_text_owned(s))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

fn copy_text_needs_escape(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '\\' | '\t' | '\n' | '\r'))
}

fn escape_copy_text_owned(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// 行数据 → DuckDB CSV 载荷（逗号分列、行尾 `\n`）
///
/// 编码契约与 [`CopyStatement::build_from_file`] 的选项串一一配对：
///
/// - **NULL** → 未引用空字段（配 `NULLSTR ''` 解码为 NULL）
/// - **空字符串** → 引用字段 `""`（配 `ALLOW_QUOTED_NULLS false` 解码为
///   空串而非 NULL，与 NULL 语义可区分）
/// - 含逗号/双引号/换行/回车的字段 → 整体双引号引用，内嵌双引号翻倍转义
///   （标准 CSV；引用字段内的换行不结束记录）
///
/// 非字符串值经 JSON 文本化后原样嵌入（数值/布尔安全；JSON 对象/数组
/// 作为文本列时按 CSV 规则引用嵌入）。
pub fn encode_duckdb_copy_rows(rows: &[Vec<serde_json::Value>]) -> String {
    // 容量粗估（典型短字段 ~16B + 分隔）：仅避免冷启动扩容，无需精确
    let mut out = String::with_capacity(rows.len() * 32);
    // 字段缓冲跨行复用：clear 保留已分配容量，行间零重分配
    let mut fields: Vec<std::borrow::Cow<'_, str>> = Vec::new();
    for row in rows {
        fields.clear();
        fields.extend(row.iter().map(encode_duckdb_value));
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(field);
        }
        out.push('\n');
    }
    out
}

/// 单值 → DuckDB CSV 字段
fn encode_duckdb_value(value: &serde_json::Value) -> std::borrow::Cow<'_, str> {
    match value {
        serde_json::Value::Null => std::borrow::Cow::Borrowed(""),
        serde_json::Value::String(s) => encode_duckdb_text(s),
        // 数值/布尔文本化后不含特殊字符，原样嵌入；JSON 对象/数组含
        // 逗号/引号，必须按 CSV 规则引用转义，否则破坏列结构
        other => {
            let text = other.to_string();
            if duckdb_csv_needs_quoting(&text) {
                std::borrow::Cow::Owned(escape_duckdb_csv(&text))
            } else {
                std::borrow::Cow::Owned(text)
            }
        }
    }
}

/// DuckDB CSV 字段转义：含分隔符/引号/换行的字段整体引用（含空串）；
/// 无需引用的路径返回借用视图零拷贝
fn encode_duckdb_text(s: &str) -> std::borrow::Cow<'_, str> {
    if duckdb_csv_needs_quoting(s) {
        std::borrow::Cow::Owned(escape_duckdb_csv(s))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

fn duckdb_csv_needs_quoting(s: &str) -> bool {
    s.is_empty() || s.chars().any(|c| matches!(c, ',' | '"' | '\n' | '\r'))
}

/// CSV 字段引用转义：整体双引号包裹，内嵌双引号翻倍
fn escape_duckdb_csv(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// 多值 INSERT 批量构建器
///
/// 把行集构建为**参数化**的多值 INSERT 语句（`INSERT INTO "t" ("a", "b")
/// VALUES (?, ?), (?, ?)`），按 [`BATCH_INSERT_CHUNK_SIZE`] 分块——一条语句
/// 承载多行，替代逐行 INSERT 的逐条往返开销；占位符绑定使数据值永不进入
/// SQL 文本，注入面在绑定层关闭。
///
/// # 供消费方执行的契约
///
/// [`Self::chunk_rows`] 返回 `Ok` 时，`(sql, params)` 序列**保证全部可安全
/// 执行**（行宽 fail-fast 全量校验先行，杜绝"前几块已落库、后块报错"的
/// 部分写入）；逐条送入 [`crate::database::Session::execute_with_params`]
/// （sqlite/mysql/postgres）或 DuckDB 参数化执行通道即可。典型消费方如
/// inklog 的批量日志落库：构建器负责语句/参数形状，执行与事务由消费方
/// 的既有通道承担。
///
/// # 示例
///
/// ```ignore
/// use dbnexus::database::copy::{BatchInsertStatement, PlaceholderStyle};
///
/// let stmt = BatchInsertStatement::new("t_logs", &["ts".into(), "msg".into()])?;
/// for (sql, params) in stmt.chunk_rows(&rows, PlaceholderStyle::QMark)? {
///     session.execute_with_params(&sql, &params).await?;
/// }
/// ```
#[derive(Debug, Clone)]
pub struct BatchInsertStatement {
    table: String,
    columns: Vec<String>,
}

impl BatchInsertStatement {
    /// 构建多值 INSERT 语句对象（表名/列名非法或列为空时返回错误）
    ///
    /// 标识符白名单与 [`CopyStatement::new`] 同一口径（字母/数字/下划线/点）。
    pub fn new(table: &str, columns: &[String]) -> Result<Self, DbError> {
        validate_identifier(table).map_err(|_| {
            DbError::Config(format!(
                "INSERT target table identifier is invalid: '{table}' \
                 (allowed: letters/digits/underscore/dot)"
            ))
        })?;
        let mut cols = Vec::with_capacity(columns.len());
        for c in columns {
            validate_identifier(c).map_err(|_| {
                DbError::Config(format!(
                    "INSERT column identifier is invalid: '{c}' \
                     (allowed: letters/digits/underscore/dot)"
                ))
            })?;
            cols.push(c.clone());
        }
        if cols.is_empty() {
            return Err(DbError::Config(
                "INSERT requires at least one column".to_string(),
            ));
        }
        Ok(Self {
            table: table.to_string(),
            columns: cols,
        })
    }

    /// 目标表名
    pub fn table(&self) -> &str {
        &self.table
    }

    /// 目标列名
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// 构建承载 `row_count` 行的多值 INSERT 语句文本
    ///
    /// `row_count = 0` 返回错误（空 VALUES 子句无意义，对齐 `copy_in`
    /// 的空集拒绝契约）。
    pub fn build(&self, row_count: usize, style: PlaceholderStyle) -> Result<String, DbError> {
        if row_count == 0 {
            return Err(DbError::Config(
                "multi-row INSERT requires at least one row (empty VALUES is meaningless)"
                    .to_string(),
            ));
        }
        let cols = self
            .columns
            .iter()
            .map(|c| quote_identifier(c))
            .collect::<Vec<_>>()
            .join(", ");
        let width = self.columns.len();
        let mut sql = format!(
            "INSERT INTO {} ({}) VALUES ",
            quote_identifier(&self.table),
            cols
        );
        // VALUES 主体一次性预留（每单元格峰值 ≈ 分隔 2 + 括号 2 + 占位符
        // 均长 ~4），避免长语句倍增扩容反复搬运
        sql.reserve(row_count * width * 8);
        let mut placeholder_no = 0usize;
        for row_idx in 0..row_count {
            if row_idx > 0 {
                sql.push_str(", ");
            }
            sql.push('(');
            for col_idx in 0..width {
                if col_idx > 0 {
                    sql.push_str(", ");
                }
                match style {
                    PlaceholderStyle::QMark => sql.push('?'),
                    PlaceholderStyle::Dollar => {
                        placeholder_no += 1;
                        sql.push('$');
                        // 手写十进制展开：避开每占位符一次 format! 的堆分配
                        let mut digits = [0u8; 20];
                        let mut n = placeholder_no;
                        let mut end = digits.len();
                        loop {
                            end -= 1;
                            digits[end] = b'0' + (n % 10) as u8;
                            n /= 10;
                            if n == 0 {
                                break;
                            }
                        }
                        sql.push_str(
                            std::str::from_utf8(&digits[end..])
                                .expect("placeholder digits are ASCII"),
                        );
                    }
                }
            }
            sql.push(')');
        }
        Ok(sql)
    }

    /// 按分块构建多值 INSERT 语句并扁平化绑定参数
    ///
    /// - 块行数 = [`BATCH_INSERT_CHUNK_SIZE`] 与 `bind_param_limit(style)/列数`
    ///   取小——宽表自动收缩，单语句占位符数恒不超后端绑定上限；产出
    ///   `(sql, params)` 有序序列，`params` 按行优先列序展开（与语句占位符
    ///   顺序一致）
    /// - **fail-fast 全量校验**：任一行宽度 ≠ 列数即整体返回错误（含行号
    ///   与期望/实际宽度），不产出部分结果——调用方拿到 `Ok` 后逐块执行
    ///   即可，无需自行处理中途失败的部分写入；列数本身超过绑定上限时
    ///   同样构建期报错（任何行数的语句都无法安全执行）
    /// - 空行集返回空序列（合法输入，调用方无事可做）
    pub fn chunk_rows(
        &self,
        rows: &[Vec<serde_json::Value>],
        style: PlaceholderStyle,
    ) -> Result<Vec<(String, Vec<serde_json::Value>)>, DbError> {
        let width = self.columns.len();
        for (idx, row) in rows.iter().enumerate() {
            if row.len() != width {
                return Err(DbError::Config(format!(
                    "row {idx} has {} value(s) but the INSERT targets {width} column(s); \
                     ragged rows are rejected before any statement executes",
                    row.len(),
                )));
            }
        }
        let limit = bind_param_limit(style);
        if width > limit {
            return Err(DbError::Config(format!(
                "INSERT targets {width} column(s) but the placeholder style binds at \
                 most {limit} parameter(s) per statement; no row count can fit, split \
                 the INSERT instead"
            )));
        }
        let chunk_size = BATCH_INSERT_CHUNK_SIZE.min(limit / width).max(1);
        let mut out = Vec::with_capacity(rows.len().div_ceil(chunk_size));
        for chunk in rows.chunks(chunk_size) {
            let sql = self.build(chunk.len(), style)?;
            let mut params = Vec::with_capacity(chunk.len() * width);
            for row in chunk {
                params.extend(row.iter().cloned());
            }
            out.push((sql, params));
        }
        Ok(out)
    }
}

/// `copy_in` 后端不支持时的统一契约错误
///
/// 无驱动组与"驱动组启用但池连接类型不匹配"（如 sqlite+duckdb 组合下
/// 拿到 sqlite 连接）共用同一基础文案；`detail` 携带路径特定的补救指引
/// 或根因尾注——失配路径必须透传下转错误的原始连接类型（"got SeaOrm"
/// 等），这是排查多驱动池配置的唯一线索，丢弃即违反错误显性化契约
fn copy_in_backend_error(detail: &str) -> crate::foundation::DbError {
    crate::foundation::DbError::Query(format!(
        "copy_in supports the postgres and duckdb backends only (COPY protocol is \
             PostgreSQL-specific; DuckDB uses file-based COPY); non-COPY backends \
             (sqlite/mysql/graph) must use their own write paths (batch_insert \
             multi-row INSERT for SQL backends){detail}"
    ))
}

impl crate::database::DbPool {
    /// COPY 批量写入：按语句将行数据送入 postgres 协议或 DuckDB 文件导入
    ///
    /// # 契约
    ///
    /// - **postgres**：语句经 `PgPoolCopyExt::copy_in_raw` 流式传输，
    ///   返回插入行数；数据块超限由驱动内部分片（1 GiB 上限内安全）
    /// - **duckdb**：行数据经 [`encode_duckdb_copy_rows`] 编码为 CSV 载荷
    ///   写入临时文件（`O_EXCL` 创建防路径碰撞），以
    ///   [`CopyStatement::build_from_file`] 的语句执行 `COPY ... FROM`，
    ///   返回导入行数；载荷文件成败皆清理（错误路径不残留）
    /// - **其他后端**（sqlite/mysql/图 DB）：显式错误——sqlite 无 COPY
    ///   能力、mysql 需服务器端 `LOAD DATA`（非本路径），绝不静默退化为
    ///   逐行 INSERT；这些后端的批量写入请使用 [`BatchInsertStatement`]
    pub async fn copy_in(
        &self,
        statement: &CopyStatement,
        rows: &[Vec<serde_json::Value>],
    ) -> crate::foundation::DbResult<u64> {
        // 空行集是调用方错误：COPY 空集无意义，直接拒绝（两条传输路径同契约）
        if rows.is_empty() {
            return Err(crate::foundation::DbError::Config(
                "copy_in requires at least one row".to_string(),
            ));
        }

        #[cfg(feature = "postgres")]
        {
            use sea_orm::sqlx::postgres::PgPoolCopyExt;

            let conn = self.acquire_connection().await?;
            // 无论成功失败都归还连接：错误路径漏归还将永久占用池槽位
            let outcome: crate::foundation::DbResult<u64> = async {
                let sea_conn = conn.as_sea_orm().map_err(|e| {
                    copy_in_backend_error(&format!(
                        " (pool returned an incompatible connection: {e})"
                    ))
                })?;
                let pg_pool = sea_conn.get_postgres_connection_pool();
                let sql = statement.build();
                let payload = encode_copy_rows(rows);
                let mut copy_in = pg_pool.copy_in_raw(&sql).await.map_err(|e| {
                    crate::foundation::DbError::Connection(sea_orm::DbErr::Conn(
                        sea_orm::RuntimeErr::SqlxError(std::sync::Arc::new(e)),
                    ))
                })?;
                copy_in.send(payload.into_bytes()).await.map_err(|e| {
                    crate::foundation::DbError::Connection(sea_orm::DbErr::Conn(
                        sea_orm::RuntimeErr::SqlxError(std::sync::Arc::new(e)),
                    ))
                })?;
                copy_in.finish().await.map_err(|e| {
                    crate::foundation::DbError::Connection(sea_orm::DbErr::Conn(
                        sea_orm::RuntimeErr::SqlxError(std::sync::Arc::new(e)),
                    ))
                })
            }
            .await;
            self.release_connection(conn);
            // 各 cfg 分支在其驱动组下均为函数尾块，以表达式隐式返回
            outcome
        }

        #[cfg(all(feature = "duckdb", not(feature = "postgres")))]
        {
            let conn = self.acquire_connection().await?;
            let outcome: crate::foundation::DbResult<u64> = async {
                let duck_conn = conn.as_duckdb().map_err(|e| {
                    copy_in_backend_error(&format!(
                        " (pool returned an incompatible connection: {e})"
                    ))
                })?;
                let payload = encode_duckdb_copy_rows(rows);
                // 载荷随行数线性增长，同步文件 I/O 经 spawn_blocking 下放，
                // 避免阻塞 tokio 执行器线程
                let path = tokio::task::spawn_blocking(move || write_copy_payload_file(&payload))
                    .await
                    .map_err(|e| {
                        crate::foundation::DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB COPY payload write task join failed: {e}"
                        )))
                    })?
                    .map_err(|e| {
                        crate::foundation::DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB COPY payload temp file creation failed: {e}"
                        )))
                    })?;
                // 载荷文件已在手：此后一切可失败步骤收在内层块，成败皆清理
                let inner: crate::foundation::DbResult<u64> = async {
                    let path_str = path.to_str().ok_or_else(|| {
                        crate::foundation::DbError::Connection(sea_orm::DbErr::Custom(
                            "COPY payload temp path is not valid UTF-8".to_string(),
                        ))
                    })?;
                    let sql = statement.build_from_file(path_str)?;
                    duck_conn.copy_from_file(&sql).await
                }
                .await;
                // 清理失败不掩盖主结果（载荷在系统临时目录，残留由系统清理
                // 策略兜底），但经 log 门面留痕拉长暴露窗口的异常
                let removed =
                    tokio::task::spawn_blocking(move || std::fs::remove_file(&path)).await;
                if let Ok(Err(e)) = removed {
                    log::warn!("DuckDB COPY payload temp file cleanup failed: {e}");
                }
                inner
            }
            .await;
            self.release_connection(conn);
            outcome
        }

        #[cfg(not(any(feature = "postgres", feature = "duckdb")))]
        {
            let _ = (statement, rows);
            Err(copy_in_backend_error(
                " — enable the postgres or duckdb driver feature to use COPY",
            ))
        }
    }
}

/// 把 COPY 载荷写入临时文件（`O_EXCL` 创建，文件名含进程号 + 原子计数器）
///
/// 返回创建的路径；调用方负责用后删除（成败皆清）。`create_new` 从机制上
/// 排除同进程/跨进程的文件名碰撞与符号链接抢占。Unix 下权限收紧为属主
/// 0600——载荷是批量行数据，系统临时目录可能多用户共享（默认 0644 全局
/// 可读构成暴露面；其余平台忽略该设置，属主隔离由目录权限兜底）。
#[cfg(all(feature = "duckdb", not(feature = "postgres")))]
fn write_copy_payload_file(payload: &str) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut last_err: Option<std::io::Error> = None;
    // 极小概率的文件名碰撞（同纳秒 + 同计数器不可能，但目录被并发清理
    // 时 create_new 可能失败）：重试数次而非一次失败即放弃
    for _ in 0..8 {
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "dbnexus_copy_{}_{}_{n}.csv",
            std::process::id(),
            now
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                file.write_all(payload.as_bytes())?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                last_err = Some(e);
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err
        .unwrap_or_else(|| std::io::Error::other("could not create COPY payload temp file")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_identifier_escapes_quotes() {
        // 白名单已排除引号，此处为防御性转义回归
        assert_eq!(quote_identifier("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn test_validate_identifier() {
        assert!(validate_identifier("t_users").is_ok());
        assert!(validate_identifier("public.users").is_ok());
        assert!(validate_identifier("_v2").is_ok());
        assert!(validate_identifier("9t").is_err());
        assert!(validate_identifier("t;drop").is_err());
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier("t..x").is_err());
    }

    #[test]
    fn test_encode_empty_rows() {
        assert_eq!(encode_copy_rows(&[]), "");
    }

    #[test]
    fn test_copy_statement_rejects_empty_columns() {
        assert!(CopyStatement::new("t", &[]).is_err());
    }

    // 载荷含批量行数据，临时目录可能多用户共享：权限必须收紧到属主
    #[cfg(all(unix, feature = "duckdb", not(feature = "postgres")))]
    #[test]
    fn test_copy_payload_file_owner_only_permissions() {
        let path = write_copy_payload_file("p").expect("create payload file");
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .expect("payload metadata")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "COPY 载荷临时文件应仅属主可读写，实际: {:o}",
            mode & 0o777
        );
        let _ = std::fs::remove_file(&path);
    }
}
