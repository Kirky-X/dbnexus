// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Neo4j 图数据库连接示例
//!
//! 演示 [`Neo4jConnection`] 的使用，包括：
//! - URL 解析（从 Bolt URL 提取 uri/user/password）
//! - 环境变量凭据回退
//! - 连接失败时的优雅降级处理
//! - GraphConnection trait 统一抽象
//!
//! Neo4j 是服务器端图数据库，需要运行中的 Neo4j 实例。
//! 本示例在无服务器时展示优雅降级，有服务器时执行完整流程。
//!
//! # 运行示例
//!
//! ```bash
//! # 无 Neo4j 服务器（演示优雅降级）
//! cargo run --example graph_neo4j --features "neo4j"
//!
//! # 有 Neo4j 服务器
//! NEO4J_URL=neo4j://user:password@localhost:7687 cargo run --example graph_neo4j --features "neo4j"  // pragma: allowlist secret
//! ```

use std::collections::HashMap;

use dbnexus::Neo4jConnection;
use dbnexus::database::GraphConnection;
use dbnexus::database::GraphExecResult;

// ============================================
// 辅助函数
// ============================================

/// 尝试从环境变量创建 Neo4j 连接
async fn try_connect() -> Option<Neo4jConnection> {
    // 优先从 NEO4J_URL 环境变量获取连接信息
    let url = std::env::var("NEO4J_URL").ok()?;
    let (uri, user, password) = Neo4jConnection::parse_url(&url).ok()?;
    Neo4jConnection::new(&uri, &user, &password).await.ok()
}

/// 打印查询结果
fn print_query_result(label: &str, result: &GraphExecResult) {
    if let GraphExecResult::Query(q) = result {
        println!("  {} ({} rows):", label, q.rows.len());
        for row in &q.rows {
            let cols: Vec<String> = row
                .columns
                .iter()
                .map(|(k, v)| format!("{}={:?}", k, v))
                .collect();
            println!("    {}", cols.join(", "));
        }
    }
}

// ============================================
// 主函数
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🕸️  DBNexus Neo4j graph database example");
    println!("========================================\n");

    // ============================================
    // URL 解析演示
    // ============================================
    println!("--- 1. URL parsing ---\n");

    let url_cases = [
        (
            "neo4j://admin:secret@dbhost:7687",
            "standard URL with credentials",
        ), // pragma: allowlist secret
        (
            "neo4j+s://user:p%40ss@cluster.example.com:7687",
            "TLS-encrypted connection URL",
        ), // pragma: allowlist secret
        ("neo4j://user:pass@localhost", "default-port URL"), // pragma: allowlist secret
    ];

    for (url, desc) in url_cases {
        match Neo4jConnection::parse_url(url) {
            Ok((uri, user, pass)) => {
                println!("  ✓ {}", desc);
                println!("    input: {}", url);
                println!("    URI:    {}", uri);
                println!("    User:   {}", user);
                println!(
                    "    Pass:   {}",
                    if pass.is_empty() { "(empty)" } else { "***" }
                );
            }
            Err(e) => println!("  ✗ {} parse failed: {}", desc, e),
        }
        println!();
    }

    // 无凭据 URL 需要环境变量（演示错误处理）
    println!(
        "  URL without credentials (needs the NEO4J_USER/NEO4J_PASSWORD environment variables):"
    );
    match Neo4jConnection::parse_url("neo4j://localhost:7687") {
        Ok(_) => println!("    ✓ credentials fetched from environment variables"),
        Err(e) => println!("    ✓ expected error: {}", e),
    }

    // ============================================
    // 连接尝试（优雅降级）
    // ============================================
    println!("\n--- 2. connection attempt ---\n");

    let conn = match try_connect().await {
        Some(c) => {
            println!("  ✓ connected to the Neo4j server successfully");
            println!("  backend name: {}", c.backend_name());
            Some(c)
        }
        None => {
            println!("  ⚠ no Neo4j server detected");
            println!("  hint: set the NEO4J_URL environment variable to connect to a real server");
            println!("  format: neo4j://user:password@localhost:7687"); // pragma: allowlist secret
            println!(
                "\n  the following demonstrates the API usage pattern (no real connection needed)...\n"
            );
            None
        }
    };

    // ============================================
    // 完整使用流程（需要真实连接）
    // ============================================
    if let Some(ref conn) = conn {
        println!("\n--- 3. health check ---\n");
        match conn.health_check().await {
            Ok(()) => println!("  ✓ health check passed"),
            Err(e) => println!("  ✗ health check failed: {}", e),
        }

        println!("\n--- 4. create nodes and run queries ---\n");
        let _ = conn
            .execute_cypher("MATCH (n:DbNexusTest) DETACH DELETE n")
            .await;

        conn.execute_cypher("CREATE (:DbNexusTest {name: 'Alice', score: 100})")
            .await?;
        conn.execute_cypher("CREATE (:DbNexusTest {name: 'Bob', score: 85})")
            .await?;
        println!("  ✓ created 2 test nodes");

        let result = conn
            .execute_cypher(
                "MATCH (n:DbNexusTest) RETURN n.name AS name, n.score AS score ORDER BY n.name",
            )
            .await?;
        print_query_result("query results", &result);

        // 参数化查询
        println!();
        let mut params = HashMap::new();
        params.insert("target".to_string(), serde_json::json!("Alice"));
        let result = conn
            .execute_cypher_with_params(
                "MATCH (n:DbNexusTest) WHERE n.name = $target RETURN n.name AS name",
                params,
            )
            .await?;
        print_query_result("Parameterized query", &result);

        // 事务
        println!("\n--- 5. graph transactions ---\n");
        let txn = conn.begin_graph_txn().await?;
        txn.execute_cypher("CREATE (:DbNexusTest {name: 'Charlie', score: 92})")
            .await?;
        println!("  ✓ inserted inside the transaction: Charlie");
        txn.commit().await?;
        println!("  ✓ transaction committed");

        let result = conn
            .execute_cypher("MATCH (n:DbNexusTest) RETURN COUNT(*) AS total")
            .await?;
        print_query_result("total after commit", &result);

        let _ = conn
            .execute_cypher("MATCH (n:DbNexusTest) DETACH DELETE n")
            .await;
        println!("\n  ✓ test data cleaned up");
    } else {
        // 无连接时的 API 使用模式演示
        println!("--- 3. API usage patterns (offline demo) ---\n");
        println!("  operations available after connecting:");
        println!("    conn.execute_cypher(\"CREATE (:Person {{name: 'Alice'}})\")");
        println!("    conn.execute_cypher(\"MATCH (p:Person) RETURN p\")");
        println!(
            "    conn.execute_cypher_with_params(\"MATCH (p) WHERE p.name = $n RETURN p\", params)"
        );
        println!("    conn.health_check().await");
        println!("    let txn = conn.begin_graph_txn().await?;");
        println!("    txn.execute_cypher(\"CREATE (:Person {{name: 'Bob'}})\").await?;");
        println!("    txn.commit().await?;");

        println!("\n--- 4. GraphConnection trait unified abstraction ---\n");
        println!(
            "  Neo4jConnection and LadybugConnection implement the same GraphConnection trait:"
        );
        println!("    - execute_cypher(cypher)            execute a Cypher query");
        println!("    - execute_cypher_with_params(...)   parameterized query");
        println!("    - health_check()                    health check");
        println!("    - begin_graph_txn()                 begin a transaction");
        println!(
            "    - backend_name()                    backend identifier (\"neo4j\" / \"ladybug\")"
        );
    }

    println!("\n========================================");
    println!("✨ Neo4j graph database example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - Neo4jConnection::new(uri, user, pass)  create a Neo4j connection");
    println!("  - Neo4jConnection::parse_url(url)        parse a Bolt URL");
    println!("  - GraphConnection trait                  the unified graph database abstraction");
    println!("  - begin_graph_txn() → commit/rollback    graph transactions");
    println!("  - graceful degradation without a server production ready");

    Ok(())
}
