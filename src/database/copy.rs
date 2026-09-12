// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! COPY 批量写入（`copy` feature）
//!
//! PostgreSQL `COPY FROM STDIN` 批量插入路径（比逐行 INSERT 高一个数量级）：
//!
//! - `CopyStatement`：构建期校验标识符（白名单）并生成 `COPY ... FROM STDIN` 语句，
//!   注入面在构建期关闭
//! - `encode_copy_rows`：行 → PG text COPY 格式编码（`\t` 分列、`\n` 分行、
//!   `\\`/`\t`/`\n`/`\r` 转义、NULL=`\N`）
//! - `DbPool::copy_in`：**协议传输路径**仅 `postgres` 驱动组（经 sea-orm 复用
//!   sqlx-postgres `PgPoolCopyExt`）；其他后端/未启用 postgres 时返回显式错误
//!   （契约：不退化为逐行 INSERT，避免静默性能劣化）
//!
//! sqlite 无 COPY 协议：契约测试断言语句构建/编码可用且 `copy_in` 显式拒绝。

use crate::foundation::DbError;

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
}

/// 标识符白名单校验（字母/数字/下划线/点；不允许引号/空白/反斜杠/分号等）
fn validate_identifier(name: &str) -> Result<(), ()> {
    if name.is_empty() {
        return Err(());
    }
    let first = name.chars().next().unwrap();
    let valid = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.';
    if !(first.is_ascii_alphabetic() || first == '_')
        || !name.chars().all(valid)
    {
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
/// 作为文本列时经转义嵌入）。
pub fn encode_copy_rows(rows: &[Vec<serde_json::Value>]) -> String {
    let mut out = String::new();
    for row in rows {
        let fields: Vec<String> = row.iter().map(encode_copy_value).collect();
        out.push_str(&fields.join("\t"));
        out.push('\n');
    }
    out
}

/// 单值 → PG text COPY 字段
fn encode_copy_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "\\N".to_string(),
        serde_json::Value::String(s) => escape_copy_text(s),
        other => escape_copy_text(&other.to_string()),
    }
}

/// PG text 转义（`\\` `\t` `\n` `\r`）
fn escape_copy_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
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

impl crate::database::DbPool {
    /// COPY 批量写入：按语句将行数据经 COPY 协议送入 postgres
    ///
    /// # 契约
    ///
    /// - **postgres**：语句经 `PgPoolCopyExt::copy_in_raw` 流式传输，
    ///   返回插入行数；数据块超限由驱动内部分片（1 GiB 上限内安全）
    /// - **其他后端**（sqlite/mysql/duckdb/图 DB）：显式错误——COPY 是
    ///   postgres 专属协议，绝不静默退化为逐行 INSERT
    pub async fn copy_in(
        &self,
        statement: &CopyStatement,
        rows: &[Vec<serde_json::Value>],
    ) -> crate::foundation::DbResult<u64> {
        #[cfg(feature = "postgres")]
        {
            use sea_orm::sqlx::postgres::PgPoolCopyExt;

            // 空行集是调用方错误：COPY 空集无意义，直接拒绝（协议层无法表达）
            if rows.is_empty() {
                return Err(crate::foundation::DbError::Config(
                    "copy_in requires at least one row".to_string(),
                ));
            }
            let conn = self.acquire_connection().await?;
            // 无论成功失败都归还连接：错误路径漏归还将永久占用池槽位
            let outcome: crate::foundation::DbResult<u64> = async {
                let sea_conn = conn.as_sea_orm()?;
                let pg_pool = sea_conn.get_postgres_connection_pool();
                let sql = statement.build();
                let payload = encode_copy_rows(rows);
                let mut copy_in = pg_pool.copy_in_raw(&sql).await.map_err(|e| {
                    crate::foundation::DbError::Connection(sea_orm::DbErr::Conn(
                        sea_orm::RuntimeErr::SqlxError(std::sync::Arc::new(e)),
                    ))
                })?;
                copy_in
                    .send(payload.into_bytes())
                    .await
                    .map_err(|e| {
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
            return outcome;
        }

        #[cfg(not(feature = "postgres"))]
        {
            let _ = (statement, rows);
            Err(crate::foundation::DbError::Query(
                "copy_in supports the postgres backend only (COPY protocol is \
                 PostgreSQL-specific); enable the postgres driver feature. \
                 sqlite/mysql/duckdb must use the regular INSERT paths"
                    .to_string(),
            ))
        }
    }
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
}
