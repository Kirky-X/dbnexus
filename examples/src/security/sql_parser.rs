// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! SQL 解析器示例
//!
//! 演示 `SqlParser` 的使用，包括：
//! - 解析不同类型 SQL（SELECT / INSERT / UPDATE / DELETE / CREATE / DROP）
//! - 提取操作类型（`SqlOperationType`）
//! - 提取目标表名
//! - 展示 DDL vs DML 区分
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example sql_parser --features "sql-parser"
//! ```

use dbnexus::SqlParser;

// 主函数

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔍 DBNexus SQL parser example");
    println!("========================================\n");

    // 创建 SQL 解析器
    let parser = SqlParser::new().await;
    println!("✓ SqlParser created (built-in LRU cache)\n");

    // 解析 DML 操作（SELECT / INSERT / UPDATE / DELETE）
    println!("--- DML operation parsing ---\n");

    let dml_cases = [
        ("SELECT * FROM users WHERE id = 1", "SELECT", "users"),
        (
            "SELECT name, email FROM users WHERE active = 1",
            "SELECT",
            "users",
        ),
        (
            "INSERT INTO users (name, email) VALUES ('Alice', 'alice@test.com')",
            "INSERT",
            "users",
        ),
        (
            "UPDATE users SET name = 'Bob' WHERE id = 1",
            "UPDATE",
            "users",
        ),
        ("DELETE FROM users WHERE id = 1", "DELETE", "users"),
        (
            "SELECT u.name, o.id FROM users u JOIN orders o ON u.id = o.user_id",
            "SELECT",
            "users",
        ),
    ];

    for (sql, expected_op, expected_table) in dml_cases {
        match parser.parse_single(sql).await {
            Ok(parsed) => {
                let op = format!("{:?}", parsed.operation_type);
                let table = parsed.table_name.as_deref().unwrap_or("(none)");
                let mark = if op.to_uppercase() == expected_op && table == expected_table {
                    "✔"
                } else {
                    "✘"
                };
                println!("  {} SQL: {}", mark, sql);
                println!("     operation type: {} (expected {})", op, expected_op);
                println!(
                    "     target table:   {} (expected {})",
                    table, expected_table
                );
            }
            Err(e) => {
                println!("  ✘ SQL: {}", sql);
                println!("     parse error: {}", e);
            }
        }
        println!();
    }

    // 解析 DDL 操作（CREATE / ALTER / DROP）
    println!("--- DDL operation parsing ---\n");
    println!("  note: SqlParser::parse_single blocks DDL operations (for safety)");
    println!("  use the is_ddl_operation() function to detect DDL separately\n");

    let ddl_cases = [
        "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))",
        "ALTER TABLE users ADD COLUMN email VARCHAR(255)",
        "DROP TABLE users",
        "CREATE INDEX idx_name ON users (name)",
        "TRUNCATE TABLE users",
    ];

    for sql in ddl_cases {
        let is_ddl = dbnexus::is_ddl_operation(sql);
        let parse_result = parser.parse_single(sql).await;
        println!("  SQL: {}", sql);
        println!("     is_ddl_operation: {}", is_ddl);
        match parse_result {
            Ok(parsed) => println!(
                "     parse result: {:?} (table: {:?})",
                parsed.operation_type, parsed.table_name
            ),
            Err(e) => println!("     parse blocked: {}", e),
        }
        println!();
    }

    // 使用 parse_operation_async 提取操作和表名
    println!("--- parse_operation_async (DML operation extraction) ---\n");

    let dml_sqls = [
        "SELECT * FROM products WHERE price > 100",
        "INSERT INTO orders (user_id, total) VALUES (1, 99.99)",
        "UPDATE products SET stock = 0 WHERE id = 5",
        "DELETE FROM logs WHERE created_at < '2024-01-01'",
    ];

    for sql in dml_sqls {
        match parser.parse_operation_async(sql).await? {
            Some((table, action)) => {
                println!("  ✔ SQL: {}", sql);
                println!("     table: {}  operation: {}", table, action);
            }
            None => {
                println!("  • SQL: {} → non-DML operation", sql);
            }
        }
    }

    // 展示 DDL vs DML 区分
    println!("\n--- DDL vs DML distinction ---\n");

    let mixed_sqls = [
        ("SELECT * FROM users", false),
        ("INSERT INTO users VALUES (1)", false),
        ("CREATE TABLE test (id INT)", true),
        ("DROP TABLE test", true),
        ("ALTER TABLE test ADD COLUMN x INT", true),
        ("TRUNCATE TABLE test", true),
        ("CREATE INDEX idx ON test (x)", true),
    ];

    for (sql, expected_ddl) in mixed_sqls {
        let is_ddl = dbnexus::is_ddl_operation(sql);
        let mark = if is_ddl == expected_ddl { "✔" } else { "✘" };
        let category = if is_ddl { "DDL" } else { "DML" };
        println!("  {} [{}] {}", mark, category, sql);
    }

    // 缓存统计
    println!("\n--- cache statistics ---\n");
    let (hits, misses) = parser.cache_stats();
    println!("  cache hits: {}", hits);
    println!("  cache misses: {}", misses);
    if hits + misses > 0 {
        let hit_rate = hits as f64 / (hits + misses) as f64 * 100.0;
        println!("  hit rate: {:.1}%", hit_rate);
    }

    // 演示缓存效果：重复解析同一条 SQL
    println!("\n  parse the same SQL 3 times:");
    let test_sql = "SELECT * FROM cached_table WHERE id = 1";
    for i in 1..=3 {
        parser.parse_single(test_sql).await?;
        let (h, m) = parser.cache_stats();
        println!("     attempt {}: hits={}, misses={}", i, h, m);
    }

    println!("\n========================================");
    println!("✨ SQL parser example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - SqlParser::new().await              create a parser (async)");
    println!("  - parser.parse_single(sql).await      parse SQL, returning a ParsedSqlOperation");
    println!("  - parser.parse_operation_async(sql)   extract the DML operation and table name");
    println!("  - SqlOperationType                    the operation type enum (Select/Insert/...)");
    println!("  - is_ddl_operation(sql)               detect a DDL operation (keyword matching)");
    println!("  - parser.cache_stats()                get the cache hit statistics");
    println!("\n⚠️  Note: parse_single blocks DDL operations and potential injection patterns,");
    println!("   only safe DML operations can be parsed successfully.");

    Ok(())
}
