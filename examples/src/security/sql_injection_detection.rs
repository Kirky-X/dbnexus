// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! SQL 注入检测示例
//!
//! 演示 `contains_sql_injection` 函数的使用，包括：
//! - 测试多种 SQL 注入模式（UNION / OR 1=1 / 注释注入 / 堆叠查询）
//! - 展示安全 SQL vs 恶意 SQL 的检测结果
//! - 演示 `is_ddl_operation` 函数
//! - 展示 Unicode 规范化绕过防护
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example sql_injection_detection --features "sql-parser"
//! ```

use dbnexus::contains_sql_injection;
use dbnexus::is_ddl_operation;

// ============================================
// 主函数
// ============================================

fn main() {
    println!("========================================");
    println!("🛡️  DBNexus SQL injection detection example");
    println!("========================================\n");

    // ============================================
    // 安全 SQL 检测（不应被标记为注入）
    // ============================================
    println!("--- ✅ safe SQL (should not be detected as injection) ---\n");

    let safe_sqls = [
        "SELECT id, name FROM users WHERE id = 1",
        "SELECT * FROM products WHERE price > 100 ORDER BY name",
        "INSERT INTO users (name, email) VALUES ('test', 'test@example.com')",
        "UPDATE users SET name = 'new_name' WHERE id = 1",
        "DELETE FROM users WHERE id = 1",
        "SELECT u.name, o.order_id FROM users u JOIN orders o ON u.id = o.user_id",
        "SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)",
    ];

    for sql in safe_sqls {
        let is_injection = contains_sql_injection(sql);
        let mark = if !is_injection { "✅" } else { "❌" };
        println!("  {} safe: {}", mark, sql);
    }

    // ============================================
    // UNION 注入检测
    // ============================================
    println!("\n--- 🚫 UNION injection ---\n");

    let union_injections = [
        "SELECT * FROM users UNION SELECT * FROM admin",
        "SELECT * FROM users UNION ALL SELECT * FROM admin",
        "SELECT * FROM users UNION DISTINCT SELECT password FROM admin",
        "select * from users union select * from admin",
    ];

    for sql in union_injections {
        let detected = contains_sql_injection(sql);
        let mark = if detected { "🚫" } else { "❌" };
        println!("  {} injection detected: {}", mark, sql);
    }

    // ============================================
    // 布尔盲注检测（OR 1=1 等）
    // ============================================
    println!("\n--- 🚫 boolean blind injection ---\n");

    let boolean_injections = [
        "SELECT * FROM users WHERE id = 1 OR 1=1",
        "SELECT * FROM users WHERE id = 1 OR 1 = 1",
        "SELECT * FROM users WHERE id = 1 OR TRUE",
        "SELECT * FROM users WHERE id = 1 AND 1=1",
        "SELECT * FROM users WHERE id = 1 AND TRUE",
        "select * from users where id = 1 or 1=1",
    ];

    for sql in boolean_injections {
        let detected = contains_sql_injection(sql);
        let mark = if detected { "🚫" } else { "❌" };
        println!("  {} injection detected: {}", mark, sql);
    }

    // ============================================
    // 注释注入检测
    // ============================================
    println!("\n--- 🚫 comment injection ---\n");

    let comment_injections = [
        "SELECT * FROM users WHERE id = 1 -- ",
        "SELECT * FROM users WHERE id = 1 --+",
        "SELECT * FROM users WHERE id = 1 #",
        "SELECT * /* comment */ FROM users",
        "SELECT * FROM users WHERE id = 1 /* bypass */",
    ];

    for sql in comment_injections {
        let detected = contains_sql_injection(sql);
        let mark = if detected { "🚫" } else { "❌" };
        println!("  {} injection detected: {}", mark, sql);
    }

    // ============================================
    // 堆叠查询检测
    // ============================================
    println!("\n--- 🚫 stacked queries ---\n");

    let stacked_injections = [
        "SELECT * FROM users; DROP TABLE users",
        "SELECT * FROM users; DELETE FROM users",
        "SELECT * FROM users; UPDATE users SET admin = 1",
        "SELECT * FROM users; INSERT INTO users VALUES (1, 'hacker')",
        "SELECT * FROM users; TRUNCATE TABLE users",
        "SELECT * FROM users; ALTER TABLE users ADD COLUMN hacked INT",
        "SELECT * FROM users; CREATE TABLE hacked (id INT)",
    ];

    for sql in stacked_injections {
        let detected = contains_sql_injection(sql);
        let mark = if detected { "🚫" } else { "❌" };
        println!("  {} injection detected: {}", mark, sql);
    }

    // ============================================
    // 时间盲注检测
    // ============================================
    println!("\n--- 🚫 time-based blind injection ---\n");

    let time_injections = [
        "SELECT * FROM users WHERE id = 1 AND SLEEP(5)",
        "SELECT * FROM users WHERE id = 1 AND BENCHMARK(10000000,SHA1('test'))",
        "SELECT * FROM users WHERE id = 1 AND PG_SLEEP(5)",
        "WAITFOR DELAY '0:0:5'",
        "SELECT DBMS_PIPE.RECEIVE_MESSAGE('test', 5) FROM dual",
    ];

    for sql in time_injections {
        let detected = contains_sql_injection(sql);
        let mark = if detected { "🚫" } else { "❌" };
        println!("  {} injection detected: {}", mark, sql);
    }

    // ============================================
    // 信息泄露检测
    // ============================================
    println!("\n--- 🚫 information disclosure ---\n");

    let info_injections = [
        "SELECT * FROM INFORMATION_SCHEMA.TABLES",
        "SELECT * FROM SYSOBJECTS",
        "SELECT * FROM MYSQL.USER",
        "SELECT * FROM PG_USER",
        "SELECT * FROM ALL_TABLES",
    ];

    for sql in info_injections {
        let detected = contains_sql_injection(sql);
        let mark = if detected { "🚫" } else { "❌" };
        println!("  {} injection detected: {}", mark, sql);
    }

    // ============================================
    // Unicode 绕过防护
    // ============================================
    println!("\n--- 🚫 Unicode bypass protection ---\n");

    let unicode_injections = [
        // 全角字符尝试绕过
        "SELECT * FROM users ＵＮＩＯＮ SELECT * FROM admin",
        "SELECT * FROM users WHERE id = 1 ＯＲ 1=1",
    ];

    for sql in unicode_injections {
        let detected = contains_sql_injection(sql);
        let mark = if detected { "🚫" } else { "❌" };
        println!("  {} injection detected: {}", mark, sql);
    }

    // ============================================
    // DDL 操作检测
    // ============================================
    println!("\n--- 🏗️  DDL operation detection (is_ddl_operation) ---\n");

    let ddl_cases = [
        ("CREATE TABLE users (id INT)", true),
        ("DROP TABLE users", true),
        ("ALTER TABLE users ADD COLUMN name VARCHAR(255)", true),
        ("TRUNCATE TABLE users", true),
        ("CREATE INDEX idx_name ON users (name)", true),
        ("SELECT * FROM users", false),
        ("INSERT INTO users VALUES (1)", false),
        ("UPDATE users SET name = 'test'", false),
        ("DELETE FROM users WHERE id = 1", false),
    ];

    for (sql, expected) in ddl_cases {
        let is_ddl = is_ddl_operation(sql);
        let mark = if is_ddl == expected { "✔" } else { "✘" };
        let category = if is_ddl { "DDL" } else { "non-DDL" };
        println!("  {} [{}] {}", mark, category, sql);
    }

    // ============================================
    // 字符串字面量中的注入模式不误报
    // ============================================
    println!("\n--- ✅ no false positives inside string literals ---\n");

    let string_literal_sqls = [
        "SELECT * FROM users WHERE name = 'test OR 1=1'",
        "SELECT * FROM users WHERE comment = 'This is -- a comment'",
    ];

    for sql in string_literal_sqls {
        let detected = contains_sql_injection(sql);
        let mark = if !detected { "✅" } else { "❌" };
        println!("  {} no false positive: {}", mark, sql);
    }

    println!("\n========================================");
    println!("✨ SQL injection detection example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - contains_sql_injection(sql)   detect SQL injection patterns");
    println!("  - is_ddl_operation(sql)         detect DDL operations");
    println!("\n🛡️  Injection types covered:");
    println!("  • UNION injection (UNION SELECT / UNION ALL SELECT)");
    println!("  • boolean blind injection (OR 1=1 / OR TRUE / AND 1=1)");
    println!("  • time-based blind injection (SLEEP / BENCHMARK / PG_SLEEP / WAITFOR)");
    println!("  • stacked queries (; DROP / ; DELETE / ; UPDATE etc.)");
    println!("  • comment injection (-- / # / /* */)");
    println!("  • information disclosure (INFORMATION_SCHEMA / SYSOBJECTS)");
    println!("  • dynamic execution (EXEC / SP_EXECUTESQL / XP_CMDSHELL)");
    println!("  • Unicode bypass (full-width characters NFKC normalized)");
    println!(
        "\n⚠️  injection patterns inside string literals are not false positives (string content removed)"
    );
}
