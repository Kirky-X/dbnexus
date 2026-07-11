// Copyright (c) 2026 Kirky.X
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
            println!("  ✓ [{}] 通过", label);
            println!("    SQL: {}", sql);
            true
        }
        Ok(DdlValidationResult::Forbidden(reason)) => {
            println!("  ✗ [{}] 拦截", label);
            println!("    SQL: {}", sql);
            println!("    原因: {}", reason);
            false
        }
        Ok(DdlValidationResult::ParseError(msg)) => {
            println!("  ⚠ [{}] 解析错误", label);
            println!("    SQL: {}", sql);
            println!("    错误: {}", msg);
            false
        }
        Err(msg) => {
            println!("  ⚠ [{}] 系统错误", label);
            println!("    SQL: {}", sql);
            println!("    错误: {}", msg);
            false
        }
    }
}

fn main() {
    println!("========================================");
    println!("🛡  DBNexus DDL 安全守卫示例");
    println!("========================================\n");

    let guard = DdlGuard::new();
    println!("✓ DdlGuard 实例创建成功\n");

    // ============================================
    // 1. 合法的 DDL 语句（应通过验证）
    // ============================================
    println!("--- 场景 1：合法的 DDL 语句（应通过） ---");

    let valid_cases = [
        (
            "CREATE TABLE",
            "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))",
        ),
        ("CREATE TABLE 小写", "create table products (id int, price real)"),
        ("CREATE OR REPLACE", "CREATE OR REPLACE TABLE sessions (id INT)"),
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
        ("SELECT 查询", "SELECT 1"),
        ("TRUNCATE", "TRUNCATE TABLE audit_log"),
    ];

    let mut passed = 0;
    for (label, sql) in &valid_cases {
        let result = guard.validate(sql);
        if print_result(label, sql, result) {
            passed += 1;
        }
    }
    println!("\n  合法 DDL 验证：{}/{} 通过\n", passed, valid_cases.len());

    // ============================================
    // 2. 危险的 DDL 语句（应被拦截）
    // ============================================
    println!("--- 场景 2：危险的 DDL 语句（应被拦截） ---");

    let dangerous_cases = [
        ("DROP TABLE", "DROP TABLE users"),
        ("DROP DATABASE", "DROP DATABASE production"),
        ("DROP DATABASE 小写", "drop database production"),
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
    println!("\n  危险 DDL 拦截：{}/{} 成功\n", blocked, dangerous_cases.len());

    // ============================================
    // 3. 非 DDL 语句（DML，应被拦截）
    // ============================================
    println!("--- 场景 3：非 DDL 语句（DML，应被拦截） ---");

    let dml_cases = [
        ("INSERT", "INSERT INTO users (id, name) VALUES (1, 'Alice')"),
        ("UPDATE", "UPDATE users SET name = 'Bob' WHERE id = 1"),
        ("DELETE", "DELETE FROM users WHERE id = 1"),
        ("DELETE 小写", "delete from users where id = 1"),
    ];

    let mut dml_blocked = 0;
    for (label, sql) in &dml_cases {
        let result = guard.validate(sql);
        if !print_result(label, sql, result) {
            dml_blocked += 1;
        }
    }
    println!("\n  DML 拦截：{}/{} 成功\n", dml_blocked, dml_cases.len());

    // ============================================
    // 4. 边界场景
    // ============================================
    println!("--- 场景 4：边界场景 ---");

    // 空 SQL
    let empty_result = guard.validate("");
    print_result("空 SQL", "", empty_result);

    // 纯空白字符
    let whitespace_result = guard.validate("   \n\t  ");
    print_result("纯空白", "   \\n\\t  ", whitespace_result);

    // 解析错误（语法不合法）
    let invalid_sql = "CREATE TABLED users (id INT)";
    let parse_result = guard.validate(invalid_sql);
    print_result("语法错误", invalid_sql, parse_result);

    println!();

    // ============================================
    // 5. Default trait 演示
    // ============================================
    println!("--- 场景 5：Default trait ---");
    let default_guard: DdlGuard = DdlGuard::default();
    let result = default_guard.validate("CREATE TABLE test (id INT)");
    print_result("Default 创建", "CREATE TABLE test (id INT)", result);

    println!();

    // ============================================
    // 6. 批量验证场景（模拟 DDL 审计）
    // ============================================
    println!("--- 场景 6：批量 DDL 审计 ---");
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
    println!("\n  审计结果：{} 通过，{} 拦截", allowed_count, blocked_count);

    println!("\n========================================");
    println!("✨ DDL 安全守卫示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - DdlGuard::new()              - 创建守卫实例");
    println!("  - DdlGuard::default()          - 使用 Default trait 创建");
    println!("  - guard.validate(sql)          - 验证 SQL 语句");
    println!("  - DdlValidationResult::Allowed - 验证通过");
    println!("  - DdlValidationResult::Forbidden - 验证失败（含原因）");
    println!("  - DdlValidationResult::ParseError - SQL 解析错误");
    println!("  - 白名单：CREATE TABLE/INDEX/VIEW, ALTER TABLE, TRUNCATE, SELECT");
    println!("  - 拦截：DROP TABLE/DATABASE/SCHEMA, INSERT/UPDATE/DELETE");
    println!("  - 允许 DROP：DROP INDEX, DROP VIEW");
}
