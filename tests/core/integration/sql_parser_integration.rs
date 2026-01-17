// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! SQL 解析模块集成测试
//!
//! 测试 SQL 解析、操作类型检测和权限动作映射功能

#[cfg(feature = "permission")]
use dbnexus::PermissionAction;
#[cfg(not(feature = "permission"))]
use dbnexus::sql_parser::PermissionAction;
use dbnexus::sql_parser::{SqlOperationType, SqlParser, is_ddl_operation};

#[tokio::test]
async fn test_parse_select() {
    let parser = SqlParser::new();
    let result = parser.parse_single("SELECT * FROM users WHERE id = 1");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Select);
}

#[tokio::test]
async fn test_parse_insert() {
    let parser = SqlParser::new();
    let result = parser.parse_single("INSERT INTO users (name) VALUES ('test')");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Insert);
    assert_eq!(parsed.table_name, Some("users".to_string()));
}

#[tokio::test]
async fn test_parse_update() {
    let parser = SqlParser::new();
    let result = parser.parse_single("UPDATE users SET name = 'test' WHERE id = 1");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Update);
    assert_eq!(parsed.table_name, Some("users".to_string()));
}

#[tokio::test]
async fn test_parse_delete() {
    let parser = SqlParser::new();
    let result = parser.parse_single("DELETE FROM users WHERE id = 1");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Delete);
    assert_eq!(parsed.table_name, Some("users".to_string()));
}

#[tokio::test]
async fn test_parse_create_table() {
    let parser = SqlParser::new();
    let result = parser.parse_single("CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Ddl);
    assert_eq!(parsed.table_name, Some("users".to_string()));
}

#[tokio::test]
async fn test_parse_drop_table() {
    let parser = SqlParser::new();
    let result = parser.parse_single("DROP TABLE users");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Ddl);
    assert_eq!(parsed.table_name, Some("users".to_string()));
}

#[tokio::test]
async fn test_parse_alter_table() {
    let parser = SqlParser::new();
    let result = parser.parse_single("ALTER TABLE users ADD COLUMN email VARCHAR(255)");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Ddl);
    assert_eq!(parsed.table_name, Some("users".to_string()));
}

#[tokio::test]
async fn test_parse_truncate() {
    let parser = SqlParser::new();
    let result = parser.parse_single("TRUNCATE TABLE users");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Ddl);
    assert_eq!(parsed.table_name, Some("users".to_string()));
}

#[tokio::test]
async fn test_parse_create_index() {
    let parser = SqlParser::new();
    let result = parser.parse_single("CREATE INDEX idx_name ON users(name)");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Ddl);
}

#[tokio::test]
async fn test_parse_grant() {
    let parser = SqlParser::new();
    let result = parser.parse_single("GRANT ALL PRIVILEGES ON users TO user1");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Dcl);
}

#[tokio::test]
async fn test_parse_revoke() {
    let parser = SqlParser::new();
    let result = parser.parse_single("REVOKE ALL PRIVILEGES ON users FROM user1");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Dcl);
}

#[tokio::test]
async fn test_parse_transaction() {
    let parser = SqlParser::new();

    let result = parser.parse_single("START TRANSACTION");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Transaction);

    let result = parser.parse_single("COMMIT");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Transaction);

    let result = parser.parse_single("ROLLBACK");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Transaction);
}

#[tokio::test]
async fn test_parse_select_with_joins() {
    let parser = SqlParser::new();
    let result = parser.parse_single("SELECT u.name, p.title FROM users u JOIN posts p ON u.id = p.user_id");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Select);
}

#[tokio::test]
async fn test_parse_insert_with_multiple_values() {
    let parser = SqlParser::new();
    let result = parser.parse_single(
        "INSERT INTO users (name, email) VALUES ('user1', 'user1@test.com'), ('user2', 'user2@test.com')",
    );
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Insert);
}

#[tokio::test]
async fn test_parse_update_with_subquery() {
    let parser = SqlParser::new();
    let result = parser
        .parse_single("UPDATE users SET status = 'active' WHERE id IN (SELECT user_id FROM orders WHERE total > 100)");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Update);
}

#[tokio::test]
async fn test_parse_delete_with_join() {
    let parser = SqlParser::new();
    let result =
        parser.parse_single("DELETE FROM users WHERE id IN (SELECT user_id FROM orders WHERE status = 'cancelled')");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Delete);
}

#[tokio::test]
async fn test_multiple_statements_rejected() {
    let parser = SqlParser::new();
    let result = parser.parse_single("SELECT * FROM users; SELECT * FROM posts");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_empty_statement_rejected() {
    let parser = SqlParser::new();
    let result = parser.parse_single("");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_whitespace_only_statement_rejected() {
    let parser = SqlParser::new();
    let result = parser.parse_single("   ");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_variables_detected() {
    let parser = SqlParser::new();

    // 测试 @variable
    let result = parser.parse_single("SELECT * FROM users WHERE id = @userId");
    assert!(result.is_err());

    // 测试 :variable
    let result = parser.parse_single("SELECT * FROM users WHERE id = :userId");
    assert!(result.is_err());

    // 测试 $variable
    let result = parser.parse_single("SELECT * FROM users WHERE id = $userId");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_set_variable_with_semicolon_allowed() {
    let parser = SqlParser::new();
    let result = parser.parse_single("SET sql_mode = 'STRICT_ALL_TABLES';");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Ddl);
}

#[tokio::test]
async fn test_variables_inside_string_literals_not_detected() {
    let parser = SqlParser::new();
    let result = parser.parse_single("SELECT '@userId' AS v");
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_is_ddl_operation() {
    assert!(is_ddl_operation("CREATE TABLE users (id INT)"));
    assert!(is_ddl_operation("DROP TABLE users"));
    assert!(is_ddl_operation("ALTER TABLE users ADD COLUMN name VARCHAR(255)"));
    assert!(is_ddl_operation("TRUNCATE TABLE users"));
    assert!(is_ddl_operation("CREATE INDEX idx_name ON users(name)"));
    assert!(!is_ddl_operation("SELECT * FROM users"));
    assert!(!is_ddl_operation("INSERT INTO users (name) VALUES ('test')"));
    assert!(!is_ddl_operation("UPDATE users SET name = 'test'"));
    assert!(!is_ddl_operation("DELETE FROM users WHERE id = 1"));
}

#[tokio::test]
async fn test_parse_operation_mapping() {
    let parser = SqlParser::new();

    // 测试 SELECT 映射到 Select
    let result = parser.parse_operation("SELECT * FROM users");
    assert!(result.is_some());
    let (_sql, action) = result.unwrap();
    assert_eq!(action, PermissionAction::Select);

    // 测试 INSERT 映射到 Insert
    let result = parser.parse_operation("INSERT INTO users (name) VALUES ('test')");
    assert!(result.is_some());
    let (_sql, action) = result.unwrap();
    assert_eq!(action, PermissionAction::Insert);

    // 测试 UPDATE 映射到 Update
    let result = parser.parse_operation("UPDATE users SET name = 'test'");
    assert!(result.is_some());
    let (_sql, action) = result.unwrap();
    assert_eq!(action, PermissionAction::Update);

    // 测试 DELETE 映射到 Delete
    let result = parser.parse_operation("DELETE FROM users WHERE id = 1");
    assert!(result.is_some());
    let (_sql, action) = result.unwrap();
    assert_eq!(action, PermissionAction::Delete);
}

#[tokio::test]
async fn test_parser_with_dialect() {
    let parser = SqlParser::with_dialect("postgres");
    let result = parser.parse_single("SELECT * FROM users");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Select);
}

#[tokio::test]
async fn test_complex_select_query() {
    let parser = SqlParser::new();
    let result = parser.parse_single(
        "SELECT u.id, u.name, COUNT(p.id) as post_count FROM users u LEFT JOIN posts p ON u.id = p.user_id WHERE u.active = 1 GROUP BY u.id, u.name HAVING COUNT(p.id) > 0 ORDER BY post_count DESC LIMIT 10",
    );
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Select);
}

#[tokio::test]
async fn test_set_variable_statement() {
    let parser = SqlParser::new();
    let result = parser.parse_single("SET sql_mode = 'STRICT_TRANS_TABLES'");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    // SET 语句应该被识别为 Other 或 Ddl
    assert!(matches!(
        parsed.operation_type,
        SqlOperationType::Other | SqlOperationType::Ddl
    ));
}

#[tokio::test]
async fn test_parsed_sql_operation_fields() {
    let parser = SqlParser::new();
    let result = parser.parse_single("INSERT INTO users (name, email) VALUES ('test', 'test@test.com')");
    assert!(result.is_ok());
    let parsed = result.unwrap();

    // 验证 ParsedSqlOperation 的字段
    assert_eq!(parsed.operation_type, SqlOperationType::Insert);
    assert_eq!(parsed.table_name, Some("users".to_string()));
    assert_eq!(
        parsed.sql,
        "INSERT INTO users (name, email) VALUES ('test', 'test@test.com')"
    );
}

#[tokio::test]
async fn test_case_insensitive_keywords() {
    let parser = SqlParser::new();

    // 测试小写关键字
    let result = parser.parse_single("select * from users");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Select);

    // 测试混合大小写
    let result = parser.parse_single("Select * From Users");
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert_eq!(parsed.operation_type, SqlOperationType::Select);
}
