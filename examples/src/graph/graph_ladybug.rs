// Copyright (c) 2026 Kirky.X
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
    println!("🕸️  DBNexus Ladybug 图数据库示例");
    println!("========================================\n");

    // ============================================
    // 1. 创建 Ladybug 内存连接
    // ============================================
    println!("--- 1. 创建 Ladybug 内存连接 ---\n");
    let conn = LadybugConnection::new(":memory:", 4)?;
    println!("  ✓ LadybugConnection 创建成功");
    println!("  后端名称:   {}", conn.backend_name());
    println!("  并发数:     {}", conn.pool_size());
    println!("  Debug 格式: {:?}\n", conn);

    // ============================================
    // 2. 创建节点表（DDL）
    // ============================================
    println!("--- 2. 创建节点表 ---\n");

    conn.execute_cypher("CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))")
        .await?;
    println!("  ✓ 创建节点表: Person(name, age)");

    conn.execute_cypher("CREATE NODE TABLE City(name STRING, population INT64, PRIMARY KEY(name))")
        .await?;
    println!("  ✓ 创建节点表: City(name, population)");

    conn.execute_cypher("CREATE REL TABLE LIVES_IN(FROM Person TO City)")
        .await?;
    println!("  ✓ 创建关系表: LIVES_IN(Person → City)\n");

    // ============================================
    // 3. 插入节点数据
    // ============================================
    println!("--- 3. 插入节点和关系 ---\n");

    let persons = [
        ("CREATE (:Person {name: 'Alice', age: 25})", "Alice"),
        ("CREATE (:Person {name: 'Bob', age: 30})", "Bob"),
        ("CREATE (:Person {name: 'Charlie', age: 35})", "Charlie"),
    ];

    for (cypher, name) in persons {
        conn.execute_cypher(cypher).await?;
        println!("  ✓ 插入节点: Person({})", name);
    }

    let cities = [
        ("CREATE (:City {name: 'Beijing', population: 21000000})", "Beijing"),
        ("CREATE (:City {name: 'Shanghai', population: 24000000})", "Shanghai"),
    ];

    for (cypher, name) in cities {
        conn.execute_cypher(cypher).await?;
        println!("  ✓ 插入节点: City({})", name);
    }

    // 创建关系
    conn.execute_cypher(
        "MATCH (p:Person), (c:City) WHERE p.name = 'Alice' AND c.name = 'Beijing' CREATE (p)-[:LIVES_IN]->(c)",
    )
    .await?;
    println!("  ✓ 创建关系: Alice -[:LIVES_IN]-> Beijing");

    conn.execute_cypher(
        "MATCH (p:Person), (c:City) WHERE p.name = 'Bob' AND c.name = 'Shanghai' CREATE (p)-[:LIVES_IN]->(c)",
    )
    .await?;
    println!("  ✓ 创建关系: Bob -[:LIVES_IN]-> Shanghai\n");

    // ============================================
    // 4. Cypher 查询
    // ============================================
    println!("--- 4. Cypher 查询 ---\n");

    // 查询所有 Person
    let result = conn
        .execute_cypher("MATCH (p:Person) RETURN p.name AS name, p.age AS age ORDER BY p.name")
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("  所有 Person（{} 行）:", q.rows.len());
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
        println!("  Person → City 关系（{} 行）:", q.rows.len());
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
            println!("    {:?} 住在 {:?}", person, city);
        }
    }

    // ============================================
    // 5. 参数化查询（防止 Cypher 注入）
    // ============================================
    println!("\n--- 5. 参数化查询 ---\n");

    let mut params = HashMap::new();
    params.insert("target_name".to_string(), serde_json::json!("Alice"));

    let result = conn
        .execute_cypher_with_params(
            "MATCH (p:Person) WHERE p.name = $target_name RETURN p.name AS name, p.age AS age",
            params,
        )
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("  参数化查询 $target_name='Alice':");
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
    println!("  ✓ 参数化查询通过 prepared statement 防止 Cypher 注入\n");

    // ============================================
    // 6. 图事务
    // ============================================
    println!("--- 6. 图事务 ---\n");

    // 事务 1：commit
    println!("  事务 1：插入数据并 commit");
    let txn = conn.begin_graph_txn().await?;
    txn.execute_cypher("CREATE (:Person {name: 'Diana', age: 28})").await?;
    println!("    ✓ 插入: Diana");
    txn.commit().await?;
    println!("    ✓ 事务已提交");

    // 验证 commit 后数据可见
    let result = conn
        .execute_cypher("MATCH (p:Person) WHERE p.name = 'Diana' RETURN p.name AS name")
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("    验证: 查询到 {} 行（Diana 可见）\n", q.rows.len());
    }

    // 事务 2：rollback
    println!("  事务 2：插入数据并 rollback");
    let txn = conn.begin_graph_txn().await?;
    txn.execute_cypher("CREATE (:Person {name: 'Evil', age: 99})").await?;
    println!("    ✓ 插入: Evil（事务内可见）");

    // 在事务内查询验证数据可见
    let result = txn
        .execute_cypher("MATCH (p:Person) WHERE p.name = 'Evil' RETURN p.name AS name")
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("    事务内查询: {} 行（Evil 可见）", q.rows.len());
    }

    txn.rollback().await?;
    println!("    ✓ 事务已回滚");

    // 验证 rollback 后数据不可见
    let result = conn
        .execute_cypher("MATCH (p:Person) WHERE p.name = 'Evil' RETURN p.name AS name")
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("    验证: 查询到 {} 行（Evil 不可见）\n", q.rows.len());
    }

    // ============================================
    // 7. 健康检查
    // ============================================
    println!("--- 7. 健康检查 ---\n");
    match conn.health_check().await {
        Ok(()) => println!("  ✓ 健康检查通过"),
        Err(e) => println!("  ✗ 健康检查失败: {}", e),
    }

    // ============================================
    // 8. Clone 行为验证
    // ============================================
    println!("\n--- 8. Clone 共享数据库 ---\n");
    let conn2 = conn.clone();
    let result = conn2
        .execute_cypher("MATCH (p:Person) RETURN COUNT(*) AS count")
        .await?;
    if let GraphExecResult::Query(q) = &result {
        println!("  通过 clone 连接查询 Person 数量: {:?}", q.rows.first());
    }
    println!("  ✓ clone 与原连接共享同一数据库实例");

    println!("\n========================================");
    println!("✨ Ladybug 图数据库示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - LadybugConnection::new(\":memory:\", pool_size)  创建内存图数据库");
    println!("  - execute_cypher(cypher)                           执行 Cypher 查询");
    println!("  - execute_cypher_with_params(cypher, params)       参数化查询（防注入）");
    println!("  - begin_graph_txn() → commit/rollback              图事务");
    println!("  - health_check()                                   健康检查");
    println!("  - Clone 共享底层 Arc<Database>                     线程安全共享");

    Ok(())
}
