// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! DuckDB 后端查询示例
//!
//! 演示 [`DuckDbConnection`] 的完整使用流程：
//! - 创建 DuckDB 内存数据库连接（默认连接池大小 = 4）
//! - 通过 `execute` 执行 DDL/DML
//! - 通过 `query` 执行聚合查询（`GROUP BY` + `AVG`）并读取 `DuckDbRow`
//! - 自定义连接池大小（`with_pool_size`）
//! - 并发查询验证连接池真正并行
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example duckdb_query --features "duckdb"
//! ```

use std::sync::Arc;

use dbnexus::DuckDbConnection;
use duckdb::types::Value as DuckValue;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🦆 DBNexus DuckDB backend query example");
    println!("========================================\n");

    // ============================================
    // 创建 DuckDB 内存连接（默认连接池大小 4）
    // ============================================
    println!("--- 1. create a DuckDB in-memory connection ---");
    let conn = DuckDbConnection::new(":memory:")?;
    println!("  ✓ DuckDbConnection created");
    println!("  pool_size = {}", conn.pool_size());
    assert_eq!(conn.pool_size(), 4);

    // ============================================
    // 创建表并插入测试数据
    // ============================================
    println!("\n--- 2. create a table and insert test data ---");
    conn.execute(
        "CREATE TABLE sales (
            id INTEGER PRIMARY KEY,
            product VARCHAR NOT NULL,
            category VARCHAR NOT NULL,
            quantity INTEGER NOT NULL,
            price DOUBLE NOT NULL
        )",
    )
    .await?;
    println!("  ✓ CREATE TABLE sales");

    // 插入 6 条记录覆盖 3 个 category
    let inserts = [
        (1, "Widget", "gadget", 10, 9.99),
        (2, "Gizmo", "gadget", 5, 19.99),
        (3, "Phone", "electronics", 3, 599.0),
        (4, "Tablet", "electronics", 7, 399.0),
        (5, "Book", "media", 50, 14.99),
        (6, "Magazine", "media", 30, 4.99),
    ];
    for (id, product, category, qty, price) in &inserts {
        conn.execute(&format!(
            "INSERT INTO sales VALUES ({id}, '{product}', '{category}', {qty}, {price})"
        ))
        .await?;
    }
    println!("  ✓ inserted 6 sales records (3 categories)");

    // ============================================
    // 简单查询：SELECT COUNT(*)
    // ============================================
    println!("\n--- 3. simple query: SELECT COUNT(*) ---");
    let rows = conn.query("SELECT COUNT(*) AS cnt FROM sales").await?;
    assert_eq!(rows.len(), 1);
    let count = rows[0].get("cnt").expect("column 'cnt' missing");
    if let DuckValue::BigInt(n) = count {
        println!("  ✓ total records = {}", n);
        assert_eq!(*n, 6);
    } else {
        panic!("Expected BigInt, got {:?}", count);
    }

    // ============================================
    // 聚合查询：GROUP BY + AVG / SUM
    // ============================================
    println!("\n--- 4. aggregate query: GROUP BY + AVG / SUM ---");
    let rows = conn
        .query(
            "SELECT
                category,
                SUM(quantity) AS total_qty,
                AVG(price) AS avg_price,
                COUNT(*) AS n
             FROM sales
             GROUP BY category
             ORDER BY avg_price DESC",
        )
        .await?;

    println!(
        "  {:<15} {:>10} {:>12} {:>5}",
        "category", "total_qty", "avg_price", "n"
    );
    println!("  {}", "-".repeat(46));
    for row in &rows {
        let category = match row.get("category") {
            Some(DuckValue::Text(s)) => s.as_str(),
            other => panic!("Expected Text for category, got {:?}", other),
        };
        // NOTE: DuckDB SUM(integer_column) 返回 HUGEINT (i128) 以防止溢出；
        // COUNT(*) 返回 BIGINT (i64)。两者均需兼容匹配，否则会 panic。
        let total_qty = match row.get("total_qty") {
            Some(DuckValue::BigInt(n)) => *n,
            Some(DuckValue::HugeInt(n)) => i64::try_from(*n).expect("HugeInt overflow"),
            other => panic!("Expected BigInt/HugeInt for total_qty, got {:?}", other),
        };
        let avg_price = match row.get("avg_price") {
            Some(DuckValue::Double(d)) => *d,
            Some(DuckValue::Float(f)) => *f as f64,
            other => panic!("Expected Double for avg_price, got {:?}", other),
        };
        let n = match row.get("n") {
            Some(DuckValue::BigInt(v)) => *v,
            Some(DuckValue::HugeInt(v)) => i64::try_from(*v).expect("HugeInt overflow"),
            other => panic!("Expected BigInt/HugeInt for n, got {:?}", other),
        };
        println!(
            "  {:<15} {:>10} {:>12.2} {:>5}",
            category, total_qty, avg_price, n
        );
    }
    assert_eq!(rows.len(), 3, "Expected 3 categories");

    // ============================================
    // 自定义连接池大小
    // ============================================
    println!("\n--- 5. custom pool size ---");
    let conn2 = DuckDbConnection::with_pool_size("duckdb::memory:", 8)?;
    println!("  ✓ with_pool_size(8) created");
    println!("  pool_size = {}", conn2.pool_size());
    assert_eq!(conn2.pool_size(), 8);

    // ============================================
    // 并发查询验证连接池真正并行
    // ============================================
    println!("\n--- 6. concurrent query verification (8 tasks vs pool_size=8) ---");
    let shared = Arc::new(DuckDbConnection::with_pool_size(":memory:", 8)?);
    shared
        .execute("CREATE TABLE parallel_demo (id INTEGER, val INTEGER)")
        .await?;
    for i in 0..16 {
        shared
            .execute(&format!("INSERT INTO parallel_demo VALUES ({i}, {i} * 2)"))
            .await?;
    }

    let mut handles = Vec::new();
    for task_id in 0..8u32 {
        let c = shared.clone();
        handles.push(tokio::spawn(async move {
            let sql = format!("SELECT SUM(val) AS s FROM parallel_demo WHERE id % 8 = {task_id}");
            let rows = c.query(&sql).await.expect("query failed");
            // SUM(integer) 返回 HUGEINT；同时兼容 BigInt 以应对未来 DuckDB 版本变更
            match rows[0].get("s") {
                Some(DuckValue::BigInt(n)) => *n,
                Some(DuckValue::HugeInt(n)) => i64::try_from(*n).expect("HugeInt overflow"),
                _ => 0,
            }
        }));
    }
    let mut sum_total: i64 = 0;
    for handle in handles {
        sum_total += handle.await?;
    }
    println!("  ✓ all 8 concurrent tasks completed");
    println!(
        "  8 shards SUM(val) accumulated = {} (should equal {})",
        sum_total,
        (0..16).map(|i| i * 2).sum::<i32>()
    );

    // ============================================
    // 健康检查
    // ============================================
    println!("\n--- 7. health check ---");
    shared.health_check().await?;
    println!("  ✓ health_check() passed (SELECT 1 AS health)");

    println!("\n========================================");
    println!("✨ DuckDB query example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - DuckDbConnection::new(url)                 create a default pool (pool=4)");
    println!("  - DuckDbConnection::with_pool_size(url, n)   custom pool size");
    println!("  - conn.execute(sql) -> DuckDbExecResult      DDL/DML");
    println!("  - conn.query(sql)  -> Vec<DuckDbRow>         SELECT");
    println!("  - DuckDbRow::get(col) -> Option<&DuckValue>  get a value by column name");
    println!("  - DuckValue::Text/BigInt/Double/Float        type matching");
    println!("  - conn.health_check()                       SELECT 1 health check");
    println!("  - conn.pool_size()                          get the pool size");
    println!("\n💡 v0.3.0 pool optimization:");
    println!("  - multiple connections share one DatabaseHandle via try_clone");
    println!("  - a :memory: database can also share data across connections");
    println!("  - Semaphore(pool_size) + Vec<Connection> implements truly parallel queries");

    Ok(())
}
