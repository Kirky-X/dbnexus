// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

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

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔍 DBNexus SQL 解析器示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建 SQL 解析器
    // ============================================
    let parser = SqlParser::new().await;
    println!("✓ SqlParser 创建成功（内置 LRU 缓存）\n");

    // ============================================
    // 2. 解析 DML 操作（SELECT / INSERT / UPDATE / DELETE）
    // ============================================
    println!("--- DML 操作解析 ---\n");

    let dml_cases = [
        ("SELECT * FROM users WHERE id = 1", "SELECT", "users"),
        ("SELECT name, email FROM users WHERE active = 1", "SELECT", "users"),
        (
            "INSERT INTO users (name, email) VALUES ('Alice', 'alice@test.com')",
            "INSERT",
            "users",
        ),
        ("UPDATE users SET name = 'Bob' WHERE id = 1", "UPDATE", "users"),
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
                let table = parsed.table_name.as_deref().unwrap_or("(无)");
                let mark = if op.to_uppercase() == expected_op && table == expected_table {
                    "✔"
                } else {
                    "✘"
                };
                println!("  {} SQL: {}", mark, sql);
                println!("     操作类型: {} (期望 {})", op, expected_op);
                println!("     目标表:   {} (期望 {})", table, expected_table);
            }
            Err(e) => {
                println!("  ✘ SQL: {}", sql);
                println!("     解析错误: {}", e);
            }
        }
        println!();
    }

    // ============================================
    // 3. 解析 DDL 操作（CREATE / ALTER / DROP）
    // ============================================
    println!("--- DDL 操作解析 ---\n");
    println!("  注意: SqlParser::parse_single 会拦截 DDL 操作（安全考虑）");
    println!("  使用 is_ddl_operation() 函数可单独检测 DDL\n");

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
                "     解析结果: {:?} (表: {:?})",
                parsed.operation_type, parsed.table_name
            ),
            Err(e) => println!("     解析拦截: {}", e),
        }
        println!();
    }

    // ============================================
    // 4. 使用 parse_operation_async 提取操作和表名
    // ============================================
    println!("--- parse_operation_async (DML 操作提取) ---\n");

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
                println!("     表: {}  操作: {}", table, action);
            }
            None => {
                println!("  • SQL: {} → 非 DML 操作", sql);
            }
        }
    }

    // ============================================
    // 5. 展示 DDL vs DML 区分
    // ============================================
    println!("\n--- DDL vs DML 区分 ---\n");

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

    // ============================================
    // 6. 缓存统计
    // ============================================
    println!("\n--- 缓存统计 ---\n");
    let (hits, misses) = parser.cache_stats();
    println!("  缓存命中: {}", hits);
    println!("  缓存未命中: {}", misses);
    if hits + misses > 0 {
        let hit_rate = hits as f64 / (hits + misses) as f64 * 100.0;
        println!("  命中率: {:.1}%", hit_rate);
    }

    // 演示缓存效果：重复解析同一条 SQL
    println!("\n  重复解析同一条 SQL 3 次:");
    let test_sql = "SELECT * FROM cached_table WHERE id = 1";
    for i in 1..=3 {
        parser.parse_single(test_sql).await?;
        let (h, m) = parser.cache_stats();
        println!("    第 {} 次: hits={}, misses={}", i, h, m);
    }

    println!("\n========================================");
    println!("✨ SQL 解析器示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - SqlParser::new().await              创建解析器（异步）");
    println!("  - parser.parse_single(sql).await      解析 SQL，返回 ParsedSqlOperation");
    println!("  - parser.parse_operation_async(sql)   提取 DML 操作和表名");
    println!("  - SqlOperationType                    操作类型枚举（Select/Insert/...）");
    println!("  - is_ddl_operation(sql)               检测 DDL 操作（关键字匹配）");
    println!("  - parser.cache_stats()                获取缓存命中统计");
    println!("\n⚠️  注意: parse_single 会拦截 DDL 和潜在注入模式，");
    println!("   仅安全 DML 操作可成功解析。");

    Ok(())
}
