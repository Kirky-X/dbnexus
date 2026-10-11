// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Ladybug 嵌入式图数据库示例
//!
//! 演示 [`LadybugConnection`] 的完整使用流程：
//! - 创建内存图数据库连接
//! - 创建节点表（DDL）和插入节点
//! - Cypher 查询（MATCH ... RETURN）
//! - 参数化查询（防止 Cypher 注入）
//! - 图事务（begin / commit / rollback）
//! - 健康检查
//!
//! Ladybug 是嵌入式图数据库（原 Kuzu），无需外部服务器，数据存储在进程内。
//! 通过 `spawn_blocking` 桥接同步 API 到 Tokio 异步运行时。
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example graph_ladybug --features "ladybug"
//! ```

use std::collections::HashMap;

use dbnexus::database::GraphConnection;
use dbnexus::database::GraphExecResult;
use dbnexus::LadybugConnection;

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🕸️  DBNexus Ladybug graph database example");
    println!("========================================\n");

    // ============================================
    // 1. 创建 Ladybug 内存连接
    // ============================================
    println!("--- 1. create the Ladybug in-memory connection ---\n");
    let conn = LadybugConnection::new(":memory:", 4)?;
    println!("  ✓ LadybugConnection created");
    println!("  backend name:   {}", conn.backend_name());
    println!("  concurrency:     {}", conn.pool_size());
    println!("  Debug format: {:?}\n", conn);

    // ============================================
    // 2. 创建节点表（DDL）
    // ============================================
    println!("--- 2. create node tables ---\n");

    conn.execute_cypher("CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))")
        .await?;
    println!("  ✓ node table created: Person(name, age)");

    conn.execute_cypher("CREATE NODE TABLE City(name STRING, population INT64, PRIMARY KEY(name))")
        .await?;
    println!("  ✓ node table created: City(name, population)");

    conn.execute_cypher("CREATE REL TABLE LIVES_IN(FROM Person TO City)")
        .await?;
    println!("  ✓ relationship table created: LIVES_IN(Person → City)\n");

    // ============================================
    // 3. 插入节点数据
    // ============================================
    println!("--- 3. insert nodes and relationships ---\n");

    let persons = [
        ("CREATE (:Person {name: 'Alice', age: 25})", "Alice"),
        ("CREATE (:Person {name: 'Bob', age: 30})", "Bob"),
        ("CREATE (:Person {name: 'Charlie', age: 35})", "Charlie"),
    ];

    for (cypher, name) in persons {
        conn.execute_cypher(cypher).await?;
        println!("  ✓ node inserted: Person({})", name);
    }

    let cities = [
        ("CREATE (:City {name: 'Beijing', population: 21000000})", "Beijing"),
        ("CREATE (:City {name: 'Shanghai', population: 24000000})", "Shanghai"),
    ];

    for (cypher, name) in cities {
        conn.execute_cypher(cypher).await?;
        println!("  ✓ node inserted: City({})", name);
    }

    // 创建关系
    conn.execute_cypher(
        "MATCH (p:Person), (c:City) WHERE p.name = 'Alice' AND c.name = 'Beijing' CREATE (p)-[:LIVES_IN]->(c)",
    )
    .await?;
    println!("  ✓ relationship created: Alice -[:LIVES_IN]-> Beijing");

    conn.execute_cypher(
        "MATCH (p:Person), (c:City) WHERE p.name = 'Bob' AND c.name = 'Shanghai' CREATE (p)-[:LIVES_IN]->(c)",
    )
    .await?;
    println!("  ✓ relationship created: Bob -[:LIVES_IN]-> Shanghai\n");

    // ============================================
    // 4. Cypher 查询
    // ============================================
    println!("--- 4. Cypher queries ---\n");

    // 查询所有 Person
    let result = conn
        .execute_cypher("MATCH (p:Person) RETURN p.name AS name, p.age AS age ORDER BY p.name")
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("  all Person nodes ({} rows):", q.rows.len());
        for row in &q.rows {
            let name = row
                .columns
                .iter()
                .find(|(k, _)| k == "name")
                .map(|(_, v)| format!("{:?}", v));
            let age = row
                .columns
                .iter()
                .find(|(k, _)| k == "age")
                .map(|(_, v)| format!("{:?}", v));
            println!("    name={:?}, age={:?}", name, age);
        }
    }

    // 查询关系：谁住在哪个城市
    println!();
    let result = conn
        .execute_cypher(
            "MATCH (p:Person)-[:LIVES_IN]->(c:City) \
             RETURN p.name AS person, c.name AS city ORDER BY p.name",
        )
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("  Person → City relationships ({} rows):", q.rows.len());
        for row in &q.rows {
            let person = row
                .columns
                .iter()
                .find(|(k, _)| k == "person")
                .map(|(_, v)| format!("{:?}", v));
            let city = row
                .columns
                .iter()
                .find(|(k, _)| k == "city")
                .map(|(_, v)| format!("{:?}", v));
            println!("    {:?} lives in {:?}", person, city);
        }
    }

    // ============================================
    // 5. 参数化查询（防止 Cypher 注入）
    // ============================================
    println!("\n--- 5. parameterized queries ---\n");

    let mut params = HashMap::new();
    params.insert("target_name".to_string(), serde_json::json!("Alice"));

    let result = conn
        .execute_cypher_with_params(
            "MATCH (p:Person) WHERE p.name = $target_name RETURN p.name AS name, p.age AS age",
            params,
        )
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("  Parameterized query $target_name='Alice':");
        for row in &q.rows {
            let name = row
                .columns
                .iter()
                .find(|(k, _)| k == "name")
                .map(|(_, v)| format!("{:?}", v));
            let age = row
                .columns
                .iter()
                .find(|(k, _)| k == "age")
                .map(|(_, v)| format!("{:?}", v));
            println!("    name={:?}, age={:?}", name, age);
        }
    }
    println!("  ✓ parameterized queries use prepared statements to prevent Cypher injection\n");

    // ============================================
    // 6. 图事务
    // ============================================
    println!("--- 6. graph transactions ---\n");

    // 事务 1：commit
    println!("  Transaction 1: insert data and commit");
    let txn = conn.begin_graph_txn().await?;
    txn.execute_cypher("CREATE (:Person {name: 'Diana', age: 28})").await?;
    println!("    ✓ inserted: Diana");
    txn.commit().await?;
    println!("    ✓ transaction committed");

    // 验证 commit 后数据可见
    let result = conn
        .execute_cypher("MATCH (p:Person) WHERE p.name = 'Diana' RETURN p.name AS name")
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("    verification: {} rows found (Diana visible)\n", q.rows.len());
    }

    // 事务 2：rollback
    println!("  Transaction 2: insert data and roll back");
    let txn = conn.begin_graph_txn().await?;
    txn.execute_cypher("CREATE (:Person {name: 'Evil', age: 99})").await?;
    println!("    ✓ inserted: Evil (visible inside the transaction)");

    // 在事务内查询验证数据可见
    let result = txn
        .execute_cypher("MATCH (p:Person) WHERE p.name = 'Evil' RETURN p.name AS name")
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("    in-transaction query: {} rows (Evil visible)", q.rows.len());
    }

    txn.rollback().await?;
    println!("    ✓ transaction rolled back");

    // 验证 rollback 后数据不可见
    let result = conn
        .execute_cypher("MATCH (p:Person) WHERE p.name = 'Evil' RETURN p.name AS name")
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("    verification: {} rows found (Evil not visible)\n", q.rows.len());
    }

    // ============================================
    // 7. 健康检查
    // ============================================
    println!("--- 7. health check ---\n");
    match conn.health_check().await {
        Ok(()) => println!("  ✓ health check passed"),
        Err(e) => println!("  ✗ health check failed: {}", e),
    }

    // ============================================
    // 8. Clone 行为验证
    // ============================================
    println!("\n--- 8. Clone sharing the database ---\n");
    let conn2 = conn.clone();
    let result = conn2
        .execute_cypher("MATCH (p:Person) RETURN COUNT(*) AS count")
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("  Query the Person count via the cloned connection: {:?}", q.rows.first());
    }
    println!("  ✓ clone shares the same database instance as the original connection");

    println!("\n========================================");
    println!("✨ Ladybug graph database example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - LadybugConnection::new(":memory:", pool_size)  create an in-memory graph database");
    println!("  - execute_cypher(cypher)                           execute a Cypher query");
    println!("  - execute_cypher_with_params(cypher, params)       parameterized query (prevents injection)");
    println!("  - begin_graph_txn() → commit/rollback              graph transactions");
    println!("  - health_check()                                   health check");
    println!("  - Clone shares the underlying Arc<Database>                     thread-safe sharing");

    Ok(())
}
