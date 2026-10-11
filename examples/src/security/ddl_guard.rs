// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! DDL 安全守卫示例
//!
//! 展示如何使用 DdlGuard 基于 AST 验证 DDL 语句的安全性：
//! - 创建 DdlGuard 实例
//! - 验证合法的 DDL 语句（CREATE TABLE / ALTER TABLE / CREATE INDEX / CREATE VIEW）
//! - 拦截危险的 DDL 语句（DROP TABLE / DROP DATABASE）
//! - 拦截非 DDL 语句（INSERT / UPDATE / DELETE）
//! - 处理解析错误场景
//! - 演示 DdlValidationResult 三种状态
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example ddl_guard --features "sql-parser"
//! ```

use dbnexus::{DdlGuard, DdlValidationResult};

/// 打印验证结果并返回是否通过
fn print_result(label: &str, sql: &str, result: Result<DdlValidationResult, String>) -> bool {
    match result {
        Ok(DdlValidationResult::Allowed) => {
            println!("  ✓ [{}] passed", label);
            println!("    SQL: {}", sql);
            true
        }
        Ok(DdlValidationResult::Forbidden(reason)) => {
            println!("  ✗ [{}] blocked", label);
            println!("    SQL: {}", sql);
            println!("    reason: {}", reason);
            false
        }
        Ok(DdlValidationResult::ParseError(msg)) => {
            println!("  ⚠ [{}] parse error", label);
            println!("    SQL: {}", sql);
            println!("    error: {}", msg);
            false
        }
        Err(msg) => {
            println!("  ⚠ [{}] system error", label);
            println!("    SQL: {}", sql);
            println!("    error: {}", msg);
            false
        }
    }
}

fn main() {
    println!("========================================");
    println!("🛡  DBNexus DDL safety guard example");
    println!("========================================\n");

    let guard = DdlGuard::new();
    println!("✓ DdlGuard instance created\n");

    // ============================================
    // 合法的 DDL 语句（应通过验证）
    // ============================================
    println!("--- Scenario 1: valid DDL statements (should pass) ---");

    let valid_cases = [
        (
            "CREATE TABLE",
            "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))",
        ),
        (
            "CREATE TABLE lowercase",
            "create table products (id int, price real)",
        ),
        (
            "CREATE OR REPLACE",
            "CREATE OR REPLACE TABLE sessions (id INT)",
        ),
        (
            "ALTER TABLE ADD COLUMN",
            "ALTER TABLE users ADD COLUMN email VARCHAR(255)",
        ),
        ("CREATE INDEX", "CREATE INDEX idx_name ON users (name)"),
        (
            "CREATE VIEW",
            "CREATE VIEW active_users AS SELECT * FROM users WHERE active = true",
        ),
        ("DROP INDEX", "DROP INDEX idx_name"),
        ("DROP VIEW", "DROP VIEW active_users"),
        ("SELECT query", "SELECT 1"),
        ("TRUNCATE", "TRUNCATE TABLE audit_log"),
    ];

    let mut passed = 0;
    for (label, sql) in &valid_cases {
        let result = guard.validate(sql);
        if print_result(label, sql, result) {
            passed += 1;
        }
    }
    println!(
        "\n  valid DDL verified: {}/{} passed\n",
        passed,
        valid_cases.len()
    );

    // ============================================
    // 危险的 DDL 语句（应被拦截）
    // ============================================
    println!("--- Scenario 2: dangerous DDL statements (should be blocked) ---");

    let dangerous_cases = [
        ("DROP TABLE", "DROP TABLE users"),
        ("DROP DATABASE", "DROP DATABASE production"),
        ("DROP DATABASE lowercase", "drop database production"),
        ("DROP ALL", "DROP ALL TABLES"),
        ("DROP SCHEMA", "DROP SCHEMA public"),
    ];

    let mut blocked = 0;
    for (label, sql) in &dangerous_cases {
        let result = guard.validate(sql);
        if !print_result(label, sql, result) {
            blocked += 1;
        }
    }
    println!(
        "\n  dangerous DDL blocked: {}/{} succeeded\n",
        blocked,
        dangerous_cases.len()
    );

    // ============================================
    // 非 DDL 语句（DML，应被拦截）
    // ============================================
    println!("--- Scenario 3: non-DDL statement (DML, should be blocked) ---");

    let dml_cases = [
        ("INSERT", "INSERT INTO users (id, name) VALUES (1, 'Alice')"),
        ("UPDATE", "UPDATE users SET name = 'Bob' WHERE id = 1"),
        ("DELETE", "DELETE FROM users WHERE id = 1"),
        ("DELETE lowercase", "delete from users where id = 1"),
    ];

    let mut dml_blocked = 0;
    for (label, sql) in &dml_cases {
        let result = guard.validate(sql);
        if !print_result(label, sql, result) {
            dml_blocked += 1;
        }
    }
    println!(
        "\n  DML blocked: {}/{} succeeded\n",
        dml_blocked,
        dml_cases.len()
    );

    // ============================================
    // 边界场景
    // ============================================
    println!("--- Scenario 4: edge cases ---");

    // 空 SQL
    let empty_result = guard.validate("");
    print_result("empty SQL", "", empty_result);

    // 纯空白字符
    let whitespace_result = guard.validate("   \n\t  ");
    print_result("whitespace only", "   \\n\\t  ", whitespace_result);

    // 解析错误（语法不合法）
    let invalid_sql = "CREATE TABLED users (id INT)";
    let parse_result = guard.validate(invalid_sql);
    print_result("syntax error", invalid_sql, parse_result);

    println!();

    // ============================================
    // Default trait 演示
    // ============================================
    println!("--- Scenario 5: the Default trait ---");
    let default_guard: DdlGuard = DdlGuard::default();
    let result = default_guard.validate("CREATE TABLE test (id INT)");
    print_result("created via Default", "CREATE TABLE test (id INT)", result);

    println!();

    // ============================================
    // 批量验证场景（模拟 DDL 审计）
    // ============================================
    println!("--- Scenario 6: batch DDL audit ---");
    let audit_batch = [
        "CREATE TABLE orders (id INT, user_id INT, amount REAL)",
        "CREATE INDEX idx_orders_user ON orders (user_id)",
        "ALTER TABLE orders ADD COLUMN status VARCHAR(20)",
        "DROP TABLE orders",
        "DROP DATABASE shop",
        "INSERT INTO orders (id) VALUES (1)",
    ];

    let mut allowed_count = 0;
    let mut blocked_count = 0;
    for sql in &audit_batch {
        match guard.validate(sql) {
            Ok(DdlValidationResult::Allowed) => {
                println!("  ✓ ALLOW: {}", sql);
                allowed_count += 1;
            }
            Ok(DdlValidationResult::Forbidden(reason)) => {
                println!("  ✗ BLOCK: {} ({})", sql, reason);
                blocked_count += 1;
            }
            Ok(DdlValidationResult::ParseError(msg)) => {
                println!("  ⚠ ERROR: {} ({})", sql, msg);
                blocked_count += 1;
            }
            Err(msg) => {
                println!("  ⚠ SYSERR: {} ({})", sql, msg);
                blocked_count += 1;
            }
        }
    }
    println!(
        "\n  audit result: {} passed, {} blocked",
        allowed_count, blocked_count
    );

    println!("\n========================================");
    println!("✨ DDL safety guard example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - DdlGuard::new()              - create a guard instance");
    println!("  - DdlGuard::default()          - create via the Default trait");
    println!("  - guard.validate(sql)          - validate a SQL statement");
    println!("  - DdlValidationResult::Allowed - validation passed");
    println!("  - DdlValidationResult::Forbidden - validation failed (with a reason)");
    println!("  - DdlValidationResult::ParseError - SQL parse error");
    println!("  - allowlist: CREATE TABLE/INDEX/VIEW, ALTER TABLE, TRUNCATE, SELECT");
    println!("  - block: DROP TABLE/DATABASE/SCHEMA, INSERT/UPDATE/DELETE");
    println!("  - allow DROP: DROP INDEX, DROP VIEW");
}
