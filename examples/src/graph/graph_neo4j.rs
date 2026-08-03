// Copyright (c) 2026 Kirky.X
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

use dbnexus::database::GraphConnection;
use dbnexus::database::GraphExecResult;
use dbnexus::Neo4jConnection;

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
        println!("  {} ({} 行):", label, q.rows.len());
        for row in &q.rows {
            let cols: Vec<String> = row.columns.iter().map(|(k, v)| format!("{}={:?}", k, v)).collect();
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
    println!("🕸️  DBNexus Neo4j 图数据库示例");
    println!("========================================\n");

    // ============================================
    // 1. URL 解析演示
    // ============================================
    println!("--- 1. URL 解析 ---\n");

    let url_cases = [
        ("neo4j://admin:secret@dbhost:7687", "带凭据的标准 URL"), // pragma: allowlist secret
        ("neo4j+s://user:p%40ss@cluster.example.com:7687", "TLS 加密连接 URL"), // pragma: allowlist secret
        ("neo4j://user:pass@localhost", "默认端口 URL"),          // pragma: allowlist secret
    ];

    for (url, desc) in url_cases {
        match Neo4jConnection::parse_url(url) {
            Ok((uri, user, pass)) => {
                println!("  ✓ {}", desc);
                println!("    输入: {}", url);
                println!("    URI:    {}", uri);
                println!("    User:   {}", user);
                println!("    Pass:   {}", if pass.is_empty() { "(空)" } else { "***" });
            }
            Err(e) => println!("  ✗ {} 解析失败: {}", desc, e),
        }
        println!();
    }

    // 无凭据 URL 需要环境变量（演示错误处理）
    println!("  无凭据 URL（需要 NEO4J_USER/NEO4J_PASSWORD 环境变量）:");
    match Neo4jConnection::parse_url("neo4j://localhost:7687") {
        Ok(_) => println!("    ✓ 从环境变量获取到凭据"),
        Err(e) => println!("    ✓ 预期错误: {}", e),
    }

    // ============================================
    // 2. 连接尝试（优雅降级）
    // ============================================
    println!("\n--- 2. 连接尝试 ---\n");

    let conn = match try_connect().await {
        Some(c) => {
            println!("  ✓ 成功连接到 Neo4j 服务器");
            println!("  后端名称: {}", c.backend_name());
            Some(c)
        }
        None => {
            println!("  ⚠ 未检测到 Neo4j 服务器");
            println!("  提示: 设置 NEO4J_URL 环境变量以连接真实服务器");
            println!("  格式: neo4j://user:password@localhost:7687"); // pragma: allowlist secret
            println!("\n  以下演示 API 使用模式（无需真实连接）...\n");
            None
        }
    };

    // ============================================
    // 3. 完整使用流程（需要真实连接）
    // ============================================
    if let Some(ref conn) = conn {
        println!("\n--- 3. 健康检查 ---\n");
        match conn.health_check().await {
            Ok(()) => println!("  ✓ 健康检查通过"),
            Err(e) => println!("  ✗ 健康检查失败: {}", e),
        }

        // 创建测试节点
        println!("\n--- 4. 创建节点和查询 ---\n");
        let _ = conn.execute_cypher("MATCH (n:DbNexusTest) DETACH DELETE n").await;

        conn.execute_cypher("CREATE (:DbNexusTest {name: 'Alice', score: 100})")
            .await?;
        conn.execute_cypher("CREATE (:DbNexusTest {name: 'Bob', score: 85})")
            .await?;
        println!("  ✓ 创建 2 个测试节点");

        let result = conn
            .execute_cypher("MATCH (n:DbNexusTest) RETURN n.name AS name, n.score AS score ORDER BY n.name")
            .await?;
        print_query_result("查询结果", &result);

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
        print_query_result("参数化查询", &result);

        // 事务
        println!("\n--- 5. 图事务 ---\n");
        let txn = conn.begin_graph_txn().await?;
        txn.execute_cypher("CREATE (:DbNexusTest {name: 'Charlie', score: 92})")
            .await?;
        println!("  ✓ 事务内插入: Charlie");
        txn.commit().await?;
        println!("  ✓ 事务已提交");

        let result = conn
            .execute_cypher("MATCH (n:DbNexusTest) RETURN COUNT(*) AS total")
            .await?;
        print_query_result("提交后总数", &result);

        // 清理
        let _ = conn.execute_cypher("MATCH (n:DbNexusTest) DETACH DELETE n").await;
        println!("\n  ✓ 测试数据已清理");
    } else {
        // 无连接时的 API 使用模式演示
        println!("--- 3. API 使用模式（离线演示）---\n");
        println!("  连接后可执行的操作:");
        println!("    conn.execute_cypher(\"CREATE (:Person {{name: 'Alice'}})\")");
        println!("    conn.execute_cypher(\"MATCH (p:Person) RETURN p\")");
        println!("    conn.execute_cypher_with_params(\"MATCH (p) WHERE p.name = $n RETURN p\", params)");
        println!("    conn.health_check().await");
        println!("    let txn = conn.begin_graph_txn().await?;");
        println!("    txn.execute_cypher(\"CREATE (:Person {{name: 'Bob'}})\").await?;");
        println!("    txn.commit().await?;");

        println!("\n--- 4. GraphConnection trait 统一抽象 ---\n");
        println!("  Neo4jConnection 和 LadybugConnection 实现相同的 GraphConnection trait:");
        println!("    - execute_cypher(cypher)           执行 Cypher 查询");
        println!("    - execute_cypher_with_params(...)   参数化查询");
        println!("    - health_check()                    健康检查");
        println!("    - begin_graph_txn()                 开始事务");
        println!("    - backend_name()                    后端标识（\"neo4j\" / \"ladybug\"）");
    }

    println!("\n========================================");
    println!("✨ Neo4j 图数据库示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - Neo4jConnection::new(uri, user, pass)  创建 Neo4j 连接");
    println!("  - Neo4jConnection::parse_url(url)        解析 Bolt URL");
    println!("  - GraphConnection trait                  图数据库统一抽象");
    println!("  - begin_graph_txn() → commit/rollback    图事务");
    println!("  - 无服务器时优雅降级                      生产就绪");

    Ok(())
}
