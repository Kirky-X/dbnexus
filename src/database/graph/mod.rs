// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 图数据库抽象层
//!
//! 定义图数据库的通用类型和 trait，为 Ladybug（嵌入式）和 Neo4j（服务器端）提供统一抽象。
//!
//! # 核心类型
//!
//! - [`GraphNode`][]: 图节点（label + properties）
//! - [`GraphRel`][]: 图关系（rel_type + src_id + dst_id + properties）
//! - [`GraphValue`][]: 查询返回值（Node/Rel/Path/Scalar）
//! - [`GraphRow`][]: 查询结果行（列名 → 值）
//! - [`GraphQueryResult`][]: 只读查询结果
//! - [`GraphExecResult`][]: 执行结果（Query 或 Write）
//!
//! # Trait
//!
//! - [`GraphConnection`][]: 图数据库连接（execute_cypher / execute_cypher_with_params / health_check / begin_graph_txn）
//! - [`GraphTransaction`][]: 图数据库事务（commit / rollback / execute_cypher / execute_cypher_with_params）

#[cfg(feature = "ladybug")]
pub mod ladybug_conn;
#[cfg(feature = "neo4j")]
pub mod neo4j_conn;

use std::collections::HashMap;

use crate::foundation::{DbError, DbResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ============================================================================
// 图数据类型
// ============================================================================

/// 图节点
///
/// 表示图数据库中的一个节点，包含标签和属性。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNode {
    /// 节点标签（如 "Person"、"Movie"）
    pub label: String,
    /// 节点属性（JSON 对象）
    pub properties: serde_json::Value,
}

/// 图关系
///
/// 表示两个节点之间的关系，包含关系类型、起止节点 ID 和属性。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphRel {
    /// 关系类型（如 "KNOWS"、"ACTED_IN"）
    pub rel_type: String,
    /// 起始节点 ID
    pub src_id: i64,
    /// 目标节点 ID
    pub dst_id: i64,
    /// 关系属性（JSON 对象）
    pub properties: serde_json::Value,
}

/// 图查询返回值
///
/// Cypher 查询可能返回节点、关系、路径或标量值。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GraphValue {
    /// 节点
    Node(GraphNode),
    /// 关系
    Rel(GraphRel),
    /// 路径（节点序列）
    Path(Vec<GraphNode>),
    /// 标量值（数字、字符串、布尔等）
    Scalar(serde_json::Value),
}

/// 图查询结果行
///
/// 一行结果包含多个命名列，每列对应一个 GraphValue。
#[derive(Debug, Clone, PartialEq)]
pub struct GraphRow {
    /// 列数据（列名, 值）
    pub columns: Vec<(String, GraphValue)>,
}

/// 图只读查询结果
///
/// 包含所有结果行和影响的行数（对于只读查询通常为 0）。
#[derive(Debug, Clone, PartialEq)]
pub struct GraphQueryResult {
    /// 结果行
    pub rows: Vec<GraphRow>,
    /// 影响的行数
    pub rows_affected: usize,
}

/// 图执行结果
///
/// 区分只读查询和写操作。
#[derive(Debug, Clone, PartialEq)]
pub enum GraphExecResult {
    /// 只读查询返回结果集
    Query(GraphQueryResult),
    /// 写操作返回影响的行数
    Write {
        /// 影响的行数
        rows_affected: usize,
    },
}

// ============================================================================
// 图连接 Trait
// ============================================================================

/// 图数据库连接 trait
///
/// 为 Ladybug 和 Neo4j 提供统一的连接抽象。
#[async_trait]
pub trait GraphConnection: Send + Sync {
    /// 执行 Cypher 查询
    ///
    /// # 安全性警告（vuln-0005）
    ///
    /// 直接拼接用户输入到 `cypher` 字符串易导致 Cypher 注入。
    /// 应优先使用 [`execute_cypher_with_params`](Self::execute_cypher_with_params)。
    ///
    /// # Errors
    ///
    /// 查询语法错误、连接失败或数据库内部错误时返回 `DbError`。
    async fn execute_cypher(&self, cypher: &str) -> DbResult<GraphExecResult>;

    /// 执行参数化 Cypher 查询（vuln-0005 修复）
    ///
    /// 参数以 `$name` 形式引用，例如：
    /// ```cypher
    /// MATCH (n:User {name: $name}) RETURN n
    /// ```
    /// 通过 `params` 提供 `name` 的值，底层使用 prepared statement，
    /// 数据库不会将参数值解析为 Cypher 代码，从根本上防止注入。
    ///
    /// # 默认实现（HD-1 修复）
    ///
    /// 默认实现返回 `DbError::Config` 错误，**不再静默回退到 `execute_cypher`**。
    /// 原回退实现会丢弃 `params`，违反 Liskov 替换：调用方预期参数化查询被执行，
    /// 但实际被忽略，可能导致 Cypher 注入（参数本应隔离代码与数据）。
    /// 强制每个实现显式重写此方法，或显式接受不安全语义。
    ///
    /// 内置实现（Ladybug/Neo4j）均重写此方法使用真正的 prepared statement。
    ///
    /// # Errors
    ///
    /// - 未重写时返回 `DbError::Config`，消息含 "execute_cypher_with_params not implemented"
    /// - 查询语法错误、参数类型不匹配、连接失败时返回 `DbError`
    async fn execute_cypher_with_params(
        &self,
        _cypher: &str,
        _params: HashMap<String, serde_json::Value>,
    ) -> DbResult<GraphExecResult> {
        // HD-1 修复：默认实现返回错误，不再静默丢弃 params 回退到 execute_cypher
        // 原回退实现违反 Liskov 替换：子类静默忽略 params 可导致 Cypher 注入
        Err(DbError::Config(
            "execute_cypher_with_params not implemented for this GraphConnection implementor; \
             must override to provide parameterized queries (vuln-0005/HD-1)"
                .to_string(),
        ))
    }

    /// 健康检查
    ///
    /// # Errors
    ///
    /// 连接不可用时返回 `DbError`。
    async fn health_check(&self) -> DbResult<()>;

    /// 开始图事务
    ///
    /// # Errors
    ///
    /// 事务开始失败时返回 `DbError`。
    async fn begin_graph_txn(&self) -> DbResult<Box<dyn GraphTransaction + Send>>;

    /// 获取后端名称（如 "ladybug"、"neo4j"）
    fn backend_name(&self) -> &'static str;
}

/// 图数据库事务 trait
///
/// 提供事务内的 Cypher 执行和提交/回滚能力。
///
/// vuln-0005 修复：trait 约束从 `Send` 升级为 `Send + Sync`，
/// 使 `execute_cypher_with_params` 等 `&self` 方法生成的 future 能跨线程传递
/// （`async_trait` 生成的 future 持有 `&Self`，要 `Send` 则 `Self: Sync`）。
/// 两个内置 implementor（`LadybugTransaction`/`Neo4jTransaction`）的内部字段
/// 均为 `Sync` 类型（`mpsc::Sender`/`AsyncMutex`），天然满足此约束。
#[async_trait]
pub trait GraphTransaction: Send + Sync {
    /// 提交事务
    ///
    /// # Errors
    ///
    /// 提交失败时返回 `DbError`。
    async fn commit(self: Box<Self>) -> DbResult<()>;

    /// 回滚事务
    ///
    /// # Errors
    ///
    /// 回滚失败时返回 `DbError`。
    async fn rollback(self: Box<Self>) -> DbResult<()>;

    /// 在事务内执行 Cypher 查询
    ///
    /// # 安全性警告（vuln-0005）
    ///
    /// 直接拼接用户输入到 `cypher` 字符串易导致 Cypher 注入。
    /// 应优先使用 [`execute_cypher_with_params`](Self::execute_cypher_with_params)。
    ///
    /// # Errors
    ///
    /// 查询语法错误或事务已关闭时返回 `DbError`。
    async fn execute_cypher(&self, cypher: &str) -> DbResult<GraphExecResult>;

    /// 在事务内执行参数化 Cypher 查询（vuln-0005 修复）
    ///
    /// 语义同 [`GraphConnection::execute_cypher_with_params`]，
    /// 但在事务上下文内执行，确保事务内所有操作使用同一连接。
    ///
    /// # 默认实现（HD-1 修复）
    ///
    /// 默认实现返回 `DbError::Config` 错误，**不再静默回退到 `execute_cypher`**。
    /// 原回退实现会丢弃 `params`，违反 Liskov 替换。强制每个事务实现显式重写。
    ///
    /// 内置实现（Ladybug/Neo4j）均重写此方法使用真正的 prepared statement。
    ///
    /// # Errors
    ///
    /// - 未重写时返回 `DbError::Config`，消息含 "execute_cypher_with_params not implemented"
    /// - 查询语法错误、参数类型不匹配、事务已关闭时返回 `DbError`
    async fn execute_cypher_with_params(
        &self,
        _cypher: &str,
        _params: HashMap<String, serde_json::Value>,
    ) -> DbResult<GraphExecResult> {
        // HD-1 修复：默认实现返回错误，不再静默丢弃 params 回退到 execute_cypher
        Err(DbError::Config(
            "execute_cypher_with_params not implemented for this GraphTransaction implementor; \
             must override to provide parameterized queries (vuln-0005/HD-1)"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::DbError;
    use serde_json::json;

    // ===== GraphNode 测试 =====

    #[test]
    fn test_graph_node_serde_roundtrip() {
        let node = GraphNode {
            label: "Person".to_string(),
            properties: json!({"name": "Alice", "age": 30}),
        };
        let json_str = serde_json::to_string(&node).expect("serialize should succeed");
        let restored: GraphNode = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(node, restored);
    }

    #[test]
    fn test_graph_node_empty_properties() {
        let node = GraphNode {
            label: "Empty".to_string(),
            properties: json!({}),
        };
        let json_str = serde_json::to_string(&node).expect("serialize should succeed");
        let restored: GraphNode = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(node, restored);
    }

    // ===== GraphRel 测试 =====

    #[test]
    fn test_graph_rel_serde_roundtrip() {
        let rel = GraphRel {
            rel_type: "KNOWS".to_string(),
            src_id: 1,
            dst_id: 2,
            properties: json!({"since": "2024-01-01"}),
        };
        let json_str = serde_json::to_string(&rel).expect("serialize should succeed");
        let restored: GraphRel = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(rel, restored);
    }

    #[test]
    fn test_graph_rel_negative_ids() {
        let rel = GraphRel {
            rel_type: "BLOCKS".to_string(),
            src_id: -1,
            dst_id: -2,
            properties: json!(null),
        };
        let json_str = serde_json::to_string(&rel).expect("serialize should succeed");
        let restored: GraphRel = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(rel, restored);
    }

    // ===== GraphValue 测试 =====

    #[test]
    fn test_graph_value_node_variant() {
        let val = GraphValue::Node(GraphNode {
            label: "Movie".to_string(),
            properties: json!({"title": "Inception"}),
        });
        let json_str = serde_json::to_string(&val).expect("serialize should succeed");
        let restored: GraphValue = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(val, restored);
    }

    #[test]
    fn test_graph_value_rel_variant() {
        let val = GraphValue::Rel(GraphRel {
            rel_type: "ACTED_IN".to_string(),
            src_id: 10,
            dst_id: 20,
            properties: json!({"role": "Cobb"}),
        });
        let json_str = serde_json::to_string(&val).expect("serialize should succeed");
        let restored: GraphValue = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(val, restored);
    }

    #[test]
    fn test_graph_value_path_variant() {
        let val = GraphValue::Path(vec![
            GraphNode {
                label: "A".to_string(),
                properties: json!({}),
            },
            GraphNode {
                label: "B".to_string(),
                properties: json!({}),
            },
        ]);
        let json_str = serde_json::to_string(&val).expect("serialize should succeed");
        let restored: GraphValue = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(val, restored);
    }

    #[test]
    fn test_graph_value_scalar_variant() {
        let val = GraphValue::Scalar(json!(42));
        let json_str = serde_json::to_string(&val).expect("serialize should succeed");
        let restored: GraphValue = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(val, restored);
    }

    #[test]
    fn test_graph_value_scalar_string() {
        let val = GraphValue::Scalar(json!("hello"));
        let json_str = serde_json::to_string(&val).expect("serialize should succeed");
        let restored: GraphValue = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(val, restored);
    }

    #[test]
    fn test_graph_value_scalar_null() {
        let val = GraphValue::Scalar(json!(null));
        let json_str = serde_json::to_string(&val).expect("serialize should succeed");
        let restored: GraphValue = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(val, restored);
    }

    #[test]
    fn test_graph_value_empty_path() {
        let val = GraphValue::Path(vec![]);
        let json_str = serde_json::to_string(&val).expect("serialize should succeed");
        let restored: GraphValue = serde_json::from_str(&json_str).expect("deserialize should succeed");
        assert_eq!(val, restored);
    }

    // ===== GraphRow 测试 =====

    #[test]
    fn test_graph_row_construction() {
        let row = GraphRow {
            columns: vec![
                ("n".to_string(), GraphValue::Scalar(json!(1))),
                ("name".to_string(), GraphValue::Scalar(json!("Alice"))),
            ],
        };
        assert_eq!(row.columns.len(), 2);
        assert_eq!(row.columns[0].0, "n");
        assert_eq!(row.columns[1].0, "name");
    }

    #[test]
    fn test_graph_row_empty() {
        let row = GraphRow { columns: vec![] };
        assert!(row.columns.is_empty());
    }

    // ===== GraphQueryResult 测试 =====

    #[test]
    fn test_graph_query_result_construction() {
        let result = GraphQueryResult {
            rows: vec![GraphRow {
                columns: vec![("count".to_string(), GraphValue::Scalar(json!(5)))],
            }],
            rows_affected: 0,
        };
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows_affected, 0);
    }

    #[test]
    fn test_graph_query_result_empty() {
        let result = GraphQueryResult {
            rows: vec![],
            rows_affected: 0,
        };
        assert!(result.rows.is_empty());
    }

    // ===== GraphExecResult 测试 =====

    #[test]
    fn test_graph_exec_result_query_variant() {
        let result = GraphExecResult::Query(GraphQueryResult {
            rows: vec![GraphRow {
                columns: vec![("n".to_string(), GraphValue::Scalar(json!(1)))],
            }],
            rows_affected: 0,
        });
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1);
                assert_eq!(q.rows_affected, 0);
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    #[test]
    fn test_graph_exec_result_write_variant() {
        let result = GraphExecResult::Write { rows_affected: 42 };
        match result {
            GraphExecResult::Query(_) => panic!("expected Write variant"),
            GraphExecResult::Write { rows_affected } => {
                assert_eq!(rows_affected, 42);
            }
        }
    }

    // ===== HD-1 测试：execute_cypher_with_params 默认实现返回错误（不静默回退） =====

    /// HD-1 Red-1：GraphConnection 默认 execute_cypher_with_params 返回错误而非回退
    ///
    /// 构造一个仅实现必需方法的 mock GraphConnection，验证未重写的
    /// `execute_cypher_with_params` 返回 `DbError::Config` 错误，
    /// 而不是静默回退到 `execute_cypher` 丢弃 params。
    #[tokio::test]
    async fn test_hd1_graph_connection_default_with_params_returns_error() {
        use async_trait::async_trait;

        /// Mock 实现：仅实现必需方法，不重写 execute_cypher_with_params
        struct MockGraphConnection;

        #[async_trait]
        impl GraphConnection for MockGraphConnection {
            async fn execute_cypher(&self, _cypher: &str) -> DbResult<GraphExecResult> {
                // 不应被调用（默认实现应返回错误而非回退到此）
                Ok(GraphExecResult::Write { rows_affected: 0 })
            }
            async fn health_check(&self) -> DbResult<()> {
                Ok(())
            }
            async fn begin_graph_txn(&self) -> DbResult<Box<dyn GraphTransaction + Send>> {
                Err(DbError::Config("not implemented in mock".to_string()))
            }
            fn backend_name(&self) -> &'static str {
                "mock"
            }
        }

        let conn = MockGraphConnection;
        let mut params = HashMap::new();
        params.insert("name".to_string(), serde_json::json!("Alice"));

        let result = conn.execute_cypher_with_params("RETURN $name", params).await;
        assert!(
            result.is_err(),
            "HD-1: default execute_cypher_with_params must return error, not silently fall back"
        );
        match result {
            Err(DbError::Config(msg)) => {
                assert!(
                    msg.contains("execute_cypher_with_params not implemented"),
                    "error should mention 'not implemented', got: {}",
                    msg
                );
            }
            other => panic!("HD-1: expected DbError::Config, got {:?}", other),
        }
    }

    /// HD-1 Red-2：GraphTransaction 默认 execute_cypher_with_params 返回错误而非回退
    ///
    /// 构造一个仅实现必需方法的 mock GraphTransaction，验证未重写的
    /// `execute_cypher_with_params` 返回 `DbError::Config` 错误。
    #[tokio::test]
    async fn test_hd1_graph_transaction_default_with_params_returns_error() {
        use async_trait::async_trait;

        /// Mock 实现：仅实现必需方法，不重写 execute_cypher_with_params
        struct MockGraphTransaction;

        #[async_trait]
        impl GraphTransaction for MockGraphTransaction {
            async fn commit(self: Box<Self>) -> DbResult<()> {
                Ok(())
            }
            async fn rollback(self: Box<Self>) -> DbResult<()> {
                Ok(())
            }
            async fn execute_cypher(&self, _cypher: &str) -> DbResult<GraphExecResult> {
                // 不应被调用
                Ok(GraphExecResult::Write { rows_affected: 0 })
            }
        }

        let txn = MockGraphTransaction;
        let mut params = HashMap::new();
        params.insert("name".to_string(), serde_json::json!("Alice"));

        let result = txn.execute_cypher_with_params("RETURN $name", params).await;
        assert!(
            result.is_err(),
            "HD-1: default execute_cypher_with_params must return error, not silently fall back"
        );
        match result {
            Err(DbError::Config(msg)) => {
                assert!(
                    msg.contains("execute_cypher_with_params not implemented"),
                    "error should mention 'not implemented', got: {}",
                    msg
                );
            }
            other => panic!("HD-1: expected DbError::Config, got {:?}", other),
        }
    }
}
