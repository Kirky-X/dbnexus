// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! SQL 解析器示例
//!
//! 展示如何使用 dbnexus 的 SQL 解析器功能：
//! - 解析 SQL 语句
//! - 提取操作类型
//! - 提取目标表名
//! - 识别 DDL/DCL 操作
//! - 处理复杂查询
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example sql_parser --features "sqlite,sql-parser"
//! ```

use dbnexus::sql_parser::SqlParser;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 DBNexus SQL 解析器示例\n");
    println!("========================================");

    // 1. 创建 SQL 解析器
    println!("\n1️⃣ 创建 SQL 解析器");
    println!("------------------------------------------");
    let parser = SqlParser::new().await;
    println!("✓ SQL 解析器创建成功");

    // 2. 解析 SELECT 语句
    println!("\n2️⃣ 解析 SELECT 语句");
    println!("------------------------------------------");

    let select_queries = vec![
        "SELECT * FROM users WHERE id = 1",
        "SELECT name, email FROM users WHERE status = 'active'",
        "SELECT COUNT(*) FROM orders",
        "SELECT u.name, o.amount FROM users u JOIN orders o ON u.id = o.user_id",
    ];

    for query in select_queries {
        match parser.parse_single(query).await {
            Ok(parsed) => {
                println!("  ✓ SQL: {}", query);
                println!("    操作类型: {:?}", parsed.operation_type);
                println!("    目标表: {:?}", parsed.table_name);
            }
            Err(e) => {
                println!("  ✗ SQL: {}", query);
                println!("    错误: {}", e);
            }
        }
        println!();
    }

    // 3. 解析 INSERT 语句
    println!("3️⃣ 解析 INSERT 语句");
    println!("------------------------------------------");

    let insert_queries = vec![
        "INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')",
        "INSERT INTO orders (user_id, amount) VALUES (1, 99.99)",
    ];

    for query in insert_queries {
        match parser.parse_single(query).await {
            Ok(parsed) => {
                println!("  ✓ SQL: {}", query);
                println!("    操作类型: {:?}", parsed.operation_type);
                println!("    目标表: {:?}", parsed.table_name);
            }
            Err(e) => {
                println!("  ✗ SQL: {}", query);
                println!("    错误: {}", e);
            }
        }
        println!();
    }

    // 4. 解析 UPDATE 语句
    println!("4️⃣ 解析 UPDATE 语句");
    println!("------------------------------------------");

    let update_queries = vec![
        "UPDATE users SET email = 'new@example.com' WHERE id = 1",
        "UPDATE orders SET status = 'completed' WHERE id = 100",
    ];

    for query in update_queries {
        match parser.parse_single(query).await {
            Ok(parsed) => {
                println!("  ✓ SQL: {}", query);
                println!("    操作类型: {:?}", parsed.operation_type);
                println!("    目标表: {:?}", parsed.table_name);
            }
            Err(e) => {
                println!("  ✗ SQL: {}", query);
                println!("    错误: {}", e);
            }
        }
        println!();
    }

    // 5. 解析 DELETE 语句
    println!("5️⃣ 解析 DELETE 语句");
    println!("------------------------------------------");

    let delete_queries = vec![
        "DELETE FROM users WHERE id = 1",
        "DELETE FROM orders WHERE created_at < '2024-01-01'",
    ];

    for query in delete_queries {
        match parser.parse_single(query).await {
            Ok(parsed) => {
                println!("  ✓ SQL: {}", query);
                println!("    操作类型: {:?}", parsed.operation_type);
                println!("    目标表: {:?}", parsed.table_name);
            }
            Err(e) => {
                println!("  ✗ SQL: {}", query);
                println!("    错误: {}", e);
            }
        }
        println!();
    }

    // 6. 解析 DDL 语句
    println!("6️⃣ 解析 DDL 语句");
    println!("------------------------------------------");

    let ddl_queries = vec![
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
        "ALTER TABLE users ADD COLUMN email TEXT",
        "DROP TABLE IF EXISTS old_users",
        "TRUNCATE TABLE temp_data",
    ];

    for query in ddl_queries {
        match parser.parse_single(query).await {
            Ok(parsed) => {
                println!("  ✓ SQL: {}", query);
                println!("    操作类型: {:?}", parsed.operation_type);
                println!("    目标表: {:?}", parsed.table_name);
                println!("    ⚠️  DDL 操作需要特殊权限");
            }
            Err(e) => {
                println!("  ✗ SQL: {}", query);
                println!("    错误: {}", e);
            }
        }
        println!();
    }

    // 7. 解析 DCL 语句
    println!("7️⃣ 解析 DCL 语句");
    println!("------------------------------------------");

    let dcl_queries = vec![
        "GRANT SELECT ON users TO app_user",
        "REVOKE INSERT ON orders FROM app_user",
    ];

    for query in dcl_queries {
        match parser.parse_single(query).await {
            Ok(parsed) => {
                println!("  ✓ SQL: {}", query);
                println!("    操作类型: {:?}", parsed.operation_type);
                println!("    目标表: {:?}", parsed.table_name);
                println!("    ⚠️  DCL 操作需要管理员权限");
            }
            Err(e) => {
                println!("  ✗ SQL: {}", query);
                println!("    错误: {}", e);
            }
        }
        println!();
    }

    // 8. 解析事务控制语句
    println!("8️⃣ 解析事务控制语句");
    println!("------------------------------------------");

    let transaction_queries = vec!["START TRANSACTION", "COMMIT", "ROLLBACK"];

    for query in transaction_queries {
        match parser.parse_single(query).await {
            Ok(parsed) => {
                println!("  ✓ SQL: {}", query);
                println!("    操作类型: {:?}", parsed.operation_type);
                println!("    ⚠️  事务控制语句");
            }
            Err(e) => {
                println!("  ✗ SQL: {}", query);
                println!("    错误: {}", e);
            }
        }
        println!();
    }

    // 9. 处理错误情况
    println!("9️⃣ 处理错误情况");
    println!("------------------------------------------");

    let error_cases = vec![
        ("空语句", ""),
        ("多语句", "SELECT * FROM users; SELECT * FROM orders"),
        ("包含变量", "SELECT * FROM users WHERE id = @user_id"),
        ("语法错误", "SELECT FROM WHERE"),
    ];

    for (name, query) in error_cases {
        match parser.parse_single(query).await {
            Ok(_parsed) => {
                println!("  ✗ {}: 应该失败但成功了", name);
            }
            Err(e) => {
                println!("  ✓ {}: 正确识别错误", name);
                println!("    错误: {}", e);
            }
        }
        println!();
    }

    // 10. 使用 parse_operation 简化 API
    println!("🔟 使用 parse_operation 简化 API");
    println!("------------------------------------------");

    let queries = vec![
        "SELECT * FROM users",
        "INSERT INTO users (name) VALUES ('Alice')",
        "UPDATE users SET name = 'Bob'",
        "DELETE FROM users WHERE id = 1",
    ];

    for query in queries {
        if let Some((table, action)) = parser.parse_operation(query) {
            println!("  ✓ SQL: {}", query);
            println!("    表: {}", table);
            println!("    操作: {:?}", action);
        }
        println!();
    }

    // 11. 演示复杂查询解析
    println!("1️⃣1️⃣ 演示复杂查询解析");
    println!("------------------------------------------");

    let complex_queries = vec![
        "WITH user_stats AS (SELECT user_id, COUNT(*) as order_count FROM orders GROUP BY user_id) SELECT u.name, us.order_count FROM users u JOIN user_stats us ON u.id = us.user_id",
        "SELECT * FROM (SELECT * FROM users WHERE status = 'active') AS active_users WHERE email LIKE '%@example.com'",
        "SELECT DISTINCT category, AVG(price) as avg_price FROM products GROUP BY category HAVING AVG(price) > 100 ORDER BY avg_price DESC LIMIT 10",
    ];

    for query in complex_queries {
        match parser.parse_single(query).await {
            Ok(parsed) => {
                println!("  ✓ 复杂查询解析成功");
                println!("    操作类型: {:?}", parsed.operation_type);
                println!("    目标表: {:?}", parsed.table_name);
            }
            Err(e) => {
                println!("  ✗ 复杂查询解析失败: {}", e);
            }
        }
        println!();
    }

    // 12. 演示权限检查集成
    println!("1️⃣2️⃣ 演示权限检查集成");
    println!("------------------------------------------");

    println!("  💡 SQL 解析器与权限检查集成:");
    println!("     1. 解析 SQL 语句");
    println!("     2. 提取操作类型和目标表");
    println!("     3. 检查用户是否有相应权限");
    println!("     4. 允许或拒绝操作");

    let user_queries = vec![
        ("user_123", "SELECT * FROM users"),
        ("user_123", "INSERT INTO orders (user_id, amount) VALUES (123, 99.99)"),
        ("user_123", "UPDATE users SET name = 'Alice' WHERE id = 123"),
        ("user_123", "DELETE FROM orders WHERE id = 100"),
        ("user_123", "DROP TABLE users"), // 应该被拒绝
    ];

    println!("\n  模拟权限检查:");
    for (user, query) in user_queries {
        if let Some((table, action)) = parser.parse_operation(query) {
            let allowed = check_permission(user, &table, &action);
            println!(
                "    - {}: {} -> {} ({:?})",
                user,
                query,
                if allowed { "✓ 允许" } else { "✗ 拒绝" },
                action
            );
        }
    }

    println!("\n========================================");
    println!("✨ SQL 解析器示例运行完成！");

    println!("\n💡 提示:");
    println!("  - SQL 解析器支持多种 SQL 方言");
    println!("  - 可以用于权限检查、审计日志、查询分析");
    println!("  - 支持复杂的 SQL 语句解析");
    println!("  - 可以检测潜在的安全问题（如 SQL 注入）");
    println!("  - 在生产环境中应该缓存解析结果以提高性能");

    Ok(())
}

/// 模拟权限检查函数
fn check_permission(user: &str, table: &str, action: &dbnexus::sql_parser::PermissionAction) -> bool {
    // 简化的权限检查逻辑
    match action {
        dbnexus::sql_parser::PermissionAction::Select => true,
        dbnexus::sql_parser::PermissionAction::Insert => table != "users",
        dbnexus::sql_parser::PermissionAction::Update => user == "admin" || table == "users",
        dbnexus::sql_parser::PermissionAction::Delete => user == "admin",
    }
}
