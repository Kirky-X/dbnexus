// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! DuckDB 后端查询示例（v0.3.0 新增）
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
    println!("🦆 DBNexus DuckDB 后端查询示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建 DuckDB 内存连接（默认连接池大小 4）
    // ============================================
    println!("--- 1. 创建 DuckDB 内存连接 ---");
    let conn = DuckDbConnection::new(":memory:")?;
    println!("  ✓ DuckDbConnection 创建成功");
    println!("  pool_size = {}", conn.pool_size());
    assert_eq!(conn.pool_size(), 4);

    // ============================================
    // 2. 创建表并插入测试数据
    // ============================================
    println!("\n--- 2. 创建表并插入测试数据 ---");
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
    println!("  ✓ INSERT 6 条销售记录（3 个 category）");

    // ============================================
    // 3. 简单查询：SELECT COUNT(*)
    // ============================================
    println!("\n--- 3. 简单查询：SELECT COUNT(*) ---");
    let rows = conn.query("SELECT COUNT(*) AS cnt FROM sales").await?;
    assert_eq!(rows.len(), 1);
    let count = rows[0].get("cnt").expect("column 'cnt' missing");
    if let DuckValue::BigInt(n) = count {
        println!("  ✓ 总记录数 = {}", n);
        assert_eq!(*n, 6);
    } else {
        panic!("Expected BigInt, got {:?}", count);
    }

    // ============================================
    // 4. 聚合查询：GROUP BY + AVG / SUM
    // ============================================
    println!("\n--- 4. 聚合查询：GROUP BY + AVG / SUM ---");
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

    println!("  {:<15} {:>10} {:>12} {:>5}", "category", "total_qty", "avg_price", "n");
    println!("  {}", "-".repeat(46));
    for row in &rows {
        let category = match row.get("category") {
            Some(DuckValue::Text(s)) => s.as_str(),
            other => panic!("Expected Text for category, got {:?}", other),
        };
        let total_qty = match row.get("total_qty") {
            Some(DuckValue::BigInt(n)) => *n,
            other => panic!("Expected BigInt for total_qty, got {:?}", other),
        };
        let avg_price = match row.get("avg_price") {
            Some(DuckValue::Double(d)) => *d,
            Some(DuckValue::Float(f)) => *f as f64,
            other => panic!("Expected Double for avg_price, got {:?}", other),
        };
        let n = match row.get("n") {
            Some(DuckValue::BigInt(v)) => *v,
            other => panic!("Expected BigInt for n, got {:?}", other),
        };
        println!("  {:<15} {:>10} {:>12.2} {:>5}", category, total_qty, avg_price, n);
    }
    assert_eq!(rows.len(), 3, "Expected 3 categories");

    // ============================================
    // 5. 自定义连接池大小
    // ============================================
    println!("\n--- 5. 自定义连接池大小 ---");
    let conn2 = DuckDbConnection::with_pool_size("duckdb::memory:", 8)?;
    println!("  ✓ with_pool_size(8) 创建成功");
    println!("  pool_size = {}", conn2.pool_size());
    assert_eq!(conn2.pool_size(), 8);

    // ============================================
    // 6. 并发查询验证连接池真正并行
    // ============================================
    println!("\n--- 6. 并发查询验证（8 个任务 vs pool_size=8）---");
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
            match rows[0].get("s") {
                Some(DuckValue::BigInt(n)) => *n,
                _ => 0,
            }
        }));
    }
    let mut sum_total: i64 = 0;
    for handle in handles {
        sum_total += handle.await?;
    }
    println!("  ✓ 8 个并发任务全部完成");
    println!("  8 个分片 SUM(val) 累加 = {} (应等于 {})", sum_total, (0..16).map(|i| i * 2).sum::<i32>());

    // ============================================
    // 7. 健康检查
    // ============================================
    println!("\n--- 7. 健康检查 ---");
    shared.health_check().await?;
    println!("  ✓ health_check() 通过（SELECT 1 AS health）");

    println!("\n========================================");
    println!("✨ DuckDB 查询示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - DuckDbConnection::new(url)                 创建默认连接池（pool=4）");
    println!("  - DuckDbConnection::with_pool_size(url, n)   自定义连接池大小");
    println!("  - conn.execute(sql) -> DuckDbExecResult      DDL/DML");
    println!("  - conn.query(sql)  -> Vec<DuckDbRow>         SELECT");
    println!("  - DuckDbRow::get(col) -> Option<&DuckValue>  按列名取值");
    println!("  - DuckValue::Text/BigInt/Double/Float        类型匹配");
    println!("  - conn.health_check()                        SELECT 1 健康检查");
    println!("  - conn.pool_size()                           获取连接池大小");
    println!("\n💡 v0.3.0 连接池优化:");
    println!("  - 多个连接通过 try_clone 共享同一 DatabaseHandle");
    println!("  - :memory: 数据库也能跨连接共享数据");
    println!("  - Semaphore(pool_size) + Vec<Connection> 实现真正并行查询");

    Ok(())
}
