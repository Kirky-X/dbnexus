// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DDL 安全守卫模块
//!
//! 提供基于 AST（抽象语法树）的 DDL 语句验证，相比字符串前缀匹配更安全可靠。

use sqlparser::ast::Statement;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

/// 允许的 DDL 语句类型白名单
const ALLOWED_DDL_STATEMENTS: &[&str] = &[
    "CreateTable",
    "AlterTable",
    "DropTable",
    "CreateIndex",
    "DropIndex",
    "CreateView",
    "Truncate",
    "Query", // SELECT 查询（用于验证）
    // DML 语句：迁移事务可能混合 DDL+DML（如 ALTER TABLE + UPDATE），
    // admin 路径的 DdlGuard 应放行——表级权限已由 security_gate 前置检查。
    "Insert",
    "Update",
    "Delete",
];

/// 禁止的 SQL 模式（AST 无法捕获的模式，如字符串拼接注入）
const FORBIDDEN_PATTERNS: &[&str] = &["DROP DATABASE", "DROP ALL"];

/// DDL 验证结果
#[derive(Debug)]
pub enum DdlValidationResult {
    /// 验证通过
    Allowed,
    /// 验证失败，包含原因
    Forbidden(String),
    /// SQL 解析失败
    ParseError(String),
}

/// DDL 安全守卫
///
/// 使用 AST 分析验证 DDL SQL 语句的安全性，
/// 替代原有的字符串前缀匹配方案。
pub struct DdlGuard {
    dialect: GenericDialect,
}

impl DdlGuard {
    /// 创建新的 DdlGuard 实例
    pub fn new() -> Self {
        Self {
            dialect: GenericDialect {},
        }
    }

    /// 验证 DDL SQL 是否安全可执行
    ///
    /// 使用 AST 解析进行语义级验证，消除字符串匹配绕过风险。
    ///
    /// # Arguments
    ///
    /// * `sql` - 要验证的 SQL 语句
    ///
    /// # Returns
    ///
    /// * `Ok(DdlValidationResult::Allowed)` - SQL 通过验证
    /// * `Ok(DdlValidationResult::Forbidden(reason))` - SQL 被拦截
    /// * `Err(msg)` - 解析错误
    pub fn validate(&self, sql: &str) -> Result<DdlValidationResult, String> {
        let sql_trimmed = sql.trim();
        if sql_trimmed.is_empty() {
            return Ok(DdlValidationResult::Forbidden("Empty SQL statement".to_string()));
        }

        // 第一步：检查禁止的字符串模式（捕获 AST 无法检测的注入）
        let sql_upper = sql_trimmed.to_uppercase();
        for pattern in FORBIDDEN_PATTERNS {
            if sql_upper.contains(pattern) {
                return Ok(DdlValidationResult::Forbidden(format!(
                    "Contains forbidden pattern: {}",
                    pattern
                )));
            }
        }

        // 第二步：AST 解析验证
        let statements =
            Parser::parse_sql(&self.dialect, sql_trimmed).map_err(|e| format!("Failed to parse SQL: {}", e))?;

        if statements.is_empty() {
            return Ok(DdlValidationResult::Forbidden(
                "Empty SQL statement after parsing".to_string(),
            ));
        }

        // 第三步：检查每条语句是否在白名单中
        for stmt in &statements {
            if !Self::is_allowed_statement(stmt) {
                return Ok(DdlValidationResult::Forbidden(format!(
                    "Statement type '{}' is not in the allowed DDL whitelist",
                    Self::statement_type_name(stmt)
                )));
            }
        }

        Ok(DdlValidationResult::Allowed)
    }

    /// 检查语句类型是否在白名单中
    fn is_allowed_statement(stmt: &Statement) -> bool {
        // 统一走白名单检查（包括 DROP 语句），不再硬编码过滤特定 DROP 类型
        let type_name = Self::statement_type_name(stmt);
        ALLOWED_DDL_STATEMENTS.contains(&type_name.as_str())
    }

    /// 获取语句的类型名称
    fn statement_type_name(stmt: &Statement) -> String {
        match stmt {
            Statement::CreateTable(_) => "CreateTable".to_string(),
            Statement::AlterTable(_) => "AlterTable".to_string(),
            Statement::CreateIndex(_) => "CreateIndex".to_string(),
            Statement::CreateView(_) => "CreateView".to_string(),
            Statement::Drop { object_type, .. } => {
                // DROP 语句需要特殊处理，根据 object_type 返回具体类型
                let type_str = format!("{:?}", object_type);
                if type_str.contains("Table") {
                    "DropTable".to_string()
                } else if type_str.contains("Index") {
                    "DropIndex".to_string()
                } else if type_str.contains("View") {
                    "DropView".to_string()
                } else if type_str.contains("Database") {
                    "DropDatabase".to_string()
                } else {
                    format!("Drop{:?}", object_type)
                }
            }
            Statement::Truncate(_) => "Truncate".to_string(),
            Statement::Query(_) => "Query".to_string(),
            Statement::Insert(_) => "Insert".to_string(),
            Statement::Update(_) => "Update".to_string(),
            Statement::Delete(_) => "Delete".to_string(),
            Statement::Set(_) => "Set".to_string(),
            _ => format!("{:?}", stmt),
        }
    }
}

impl Default for DdlGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> DdlGuard {
        DdlGuard::new()
    }

    #[test]
    fn test_valid_create_table() {
        let result = guard().validate("CREATE TABLE users (id INT PRIMARY KEY)").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_create_table_lowercase() {
        let result = guard().validate("create table users (id int)").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_create_or_replace() {
        // CREATE OR REPLACE 是 sqlparser 规范化的语句，仍解析为 CreateTable
        let result = guard().validate("CREATE OR REPLACE TABLE users (id INT)").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_alter_table() {
        let result = guard()
            .validate("ALTER TABLE users ADD COLUMN name VARCHAR(255)")
            .unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_create_index() {
        let result = guard().validate("CREATE INDEX idx_name ON users (name)").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_drop_index() {
        let result = guard().validate("DROP INDEX idx_name").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_drop_view() {
        let result = guard().validate("DROP VIEW active_users").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_create_view() {
        let result = guard()
            .validate("CREATE VIEW active_users AS SELECT * FROM users WHERE active = true")
            .unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_valid_select() {
        let result = guard().validate("SELECT 1").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_drop_database_rejected() {
        let result = guard().validate("DROP DATABASE production").unwrap();
        assert!(matches!(
            result,
            DdlValidationResult::Forbidden(ref msg) if msg.contains("DROP DATABASE")
        ));
    }

    #[test]
    fn test_drop_database_lowercase_rejected() {
        let result = guard().validate("drop database production").unwrap();
        assert!(matches!(
            result,
            DdlValidationResult::Forbidden(ref msg) if msg.contains("forbidden")
        ));
    }

    #[test]
    fn test_drop_all_rejected() {
        let result = guard().validate("DROP ALL TABLES").unwrap();
        assert!(matches!(
            result,
            DdlValidationResult::Forbidden(ref msg) if msg.contains("DROP ALL")
        ));
    }

    #[test]
    fn test_drop_table_allowed_for_admin() {
        // DROP TABLE 在白名单中（admin 路径 DdlGuard 放行，角色检查由 security_gate 前置保证）
        let result = guard().validate("DROP TABLE users").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_insert_allowed_for_admin() {
        // DML 在白名单中（迁移事务可能混合 DDL+DML）
        let result = guard().validate("INSERT INTO users (id) VALUES (1)").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_update_allowed_for_admin() {
        let result = guard().validate("UPDATE users SET name = 'test' WHERE id = 1").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_delete_allowed_for_admin() {
        let result = guard().validate("DELETE FROM users WHERE id = 1").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_delete_lowercase_allowed_for_admin() {
        let result = guard().validate("delete from users where id = 1").unwrap();
        assert!(matches!(result, DdlValidationResult::Allowed));
    }

    #[test]
    fn test_empty_sql() {
        let result = guard().validate("").unwrap();
        assert!(matches!(
            result,
            DdlValidationResult::Forbidden(ref msg) if msg.contains("Empty")
        ));
    }

    #[test]
    fn test_whitespace_sql() {
        let result = guard().validate("   \n\t  ").unwrap();
        assert!(matches!(
            result,
            DdlValidationResult::Forbidden(ref msg) if msg.contains("Empty")
        ));
    }
}
