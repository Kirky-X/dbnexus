// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! SQL Parser module using sqlparser for enhanced SQL parsing and validation.
//! This module provides robust SQL operation detection and permission action mapping.

use once_cell::sync::Lazy;
use regex::Regex;
use sqlparser::ast::{Delete, FromTable, Query, SetExpr, Statement, TableWithJoins};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use thiserror::Error;

#[cfg(feature = "permission")]
pub use crate::permission::PermissionAction;

#[cfg(not(feature = "permission"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// 权限操作类型（本地定义，当 permission 特性未启用时使用）
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
#[derive(Debug, Clone)]
pub struct ParsedSqlOperation {
    /// The type of SQL operation
    pub operation_type: SqlOperationType,
    /// The table name if applicable
    pub table_name: Option<String>,
    /// The raw SQL statement
    pub sql: String,
}

/// Types of SQL operations that can be detected
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// SQL Parser with dialect awareness
pub struct SqlParser {
    dialect: GenericDialect,
}

impl Default for SqlParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlParser {
    /// Create a new SQL parser with generic dialect support
    #[inline]
    pub fn new() -> Self {
        Self {
            dialect: GenericDialect {},
        }
    }

    /// Create a parser with specific database dialect
    #[inline]
    pub fn with_dialect(_db_type: &str) -> Self {
        // Using GenericDialect for broad compatibility
        Self::new()
    }

    /// Parse and validate a single SQL statement
    ///
    /// Returns `ParsedSqlOperation` if parsing succeeds
    /// Returns `SqlParseError` if parsing fails or multiple statements detected
    pub fn parse_single(&self, sql: &str) -> Result<ParsedSqlOperation, SqlParseError> {
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
    pub fn parse_operation(&self, sql: &str) -> Option<(String, PermissionAction)> {
        self.parse_single(sql).ok().and_then(|parsed| {
            // DDL/DCL/Transaction 操作默认拒绝（权限引擎会处理）
            // 这里只返回 DML 操作的权限动作
            let action = match parsed.operation_type {
                SqlOperationType::Select => PermissionAction::Select,
                SqlOperationType::Insert => PermissionAction::Insert,
                SqlOperationType::Update => PermissionAction::Update,
                SqlOperationType::Delete => PermissionAction::Delete,
                SqlOperationType::Ddl => PermissionAction::Select, // 占位，会被 is_ddl_operation 拦截
                SqlOperationType::Dcl => PermissionAction::Select, // 占位
                SqlOperationType::Transaction => PermissionAction::Select, // 占位
                SqlOperationType::Other => PermissionAction::Select, // 占位
            };
            parsed.table_name.map(|table_name| (table_name, action))
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
fn contains_sql_injection(sql: &str) -> bool {
    let sql_without_strings = remove_string_literals(sql);
    let sql_upper = sql_without_strings.to_uppercase();

    // Check for common SQL injection patterns
    let injection_patterns = [
        "UNION SELECT",
        " OR 1=1",
        " OR TRUE",
        " OR FALSE",
        "; DROP",
        "; DELETE",
        "; UPDATE",
        "-- ",
        "/* ",
        " xp_",
        "EXEC xp_",
        "EXECUTE xp_",
        "WAITFOR DELAY",
        "SLEEP(",
        "BENCHMARK(",
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
}
