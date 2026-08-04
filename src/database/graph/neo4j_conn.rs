// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Neo4j 图数据库连接
//!
//! 提供 Neo4j 服务器端图数据库的异步连接抽象，基于 `neo4rs 0.8.0` crate。
//!
//! # 架构说明
//!
//! [`Neo4jConnection`] 封装 `Arc<neo4rs::Graph>`，`neo4rs::Graph` 内部已维护连接池，
//! 因此无需像 [`LadybugConnection`](super::ladybug_conn::LadybugConnection) 那样自建
//! `Semaphore` 限流。每次 `execute_cypher` 通过 `Graph::execute` 获取
//! `DetachedRowStream`，迭代行并映射到 [`GraphRow`]。
//!
//! # 事务模型
//!
//! `neo4rs::Txn` 的 `execute` 需要 `&mut self`，而 [`GraphTransaction::execute_cypher`]
//! 签名为 `&self`。为解决这一矛盾，[`Neo4jTransaction`] 使用 `AsyncMutex<Option<Txn>>`
//! 提供内部可变性。`commit`/`rollback` 消耗 self 时，从 `Option` 中取出 `Txn` 调用
//! 对应方法。
//!
//! # 线程安全
//!
//! `neo4rs::Graph` 是 `Send + Sync`（内部通过 `Arc` 共享连接池）。
//! `neo4rs::Txn` 是 `Send + Sync`（持有独占连接）。通过 `Arc<Graph>` 共享连接，
//! `AsyncMutex<Option<Txn>>` 保证事务内操作的线程安全。

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex as AsyncMutex;

use crate::database::graph::{
    GraphConnection, GraphExecResult, GraphQueryResult, GraphRow, GraphTransaction, GraphValue,
};
use crate::foundation::{DbError, DbResult};

/// Neo4j 图数据库连接
///
/// 封装 `neo4rs::Graph`，提供 Cypher 查询执行、健康检查和事务能力。
///
/// `graph` 字段为 `Option` 以支持 `new_placeholder()`（用于不连接服务器的单元测试）。
/// 真实连接通过 [`new`](Self::new) 创建，`graph` 为 `Some`。
///
/// # 示例
///
/// ```ignore
/// use dbnexus::database::Neo4jConnection;
///
/// let conn = Neo4jConnection::new("neo4j://localhost:7687", "neo4j", "password").await?;
/// conn.health_check().await?;
/// ```
#[derive(Clone)]
pub struct Neo4jConnection {
    /// Neo4j 连接（None 表示占位连接，不可执行查询）
    graph: Option<Arc<neo4rs::Graph>>,
}

impl Neo4jConnection {
    /// 创建新的 Neo4j 连接
    ///
    /// # 参数
    ///
    /// * `uri` - Neo4j Bolt 协议地址（如 `neo4j://localhost:7687`）
    /// * `user` - 用户名
    /// * `password` - 密码
    ///
    /// # 错误
    ///
    /// 连接失败时返回 `DbError::Connection`
    pub async fn new(uri: &str, user: &str, password: &str) -> DbResult<Self> {
        let graph = neo4rs::Graph::new(uri, user, password)
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j connect: {e}"))))?;
        Ok(Self {
            graph: Some(Arc::new(graph)),
        })
    }

    /// 从 URL 解析出 (uri, user, password)
    ///
    /// 支持的格式：
    /// - `neo4j://user:password@localhost:7687` → (`neo4j://localhost:7687`, `user`, `password`)
    /// - `neo4j+s://user:password@host:7687` → (`neo4j+s://host:7687`, `user`, `password`)
    /// - `neo4j://localhost:7687`（无凭据）→ 从 `NEO4J_USER`/`NEO4J_PASSWORD` 环境变量读取
    ///
    /// # 返回
    ///
    /// 返回三元组 `(uri, user, password)`，uri 已去除 userinfo 部分。
    ///
    /// # 错误
    ///
    /// - URL 无凭据且 `NEO4J_USER`/`NEO4J_PASSWORD` 环境变量未设置时返回错误
    ///   （避免空凭据导致后续 `neo4rs::Graph::new` 认证失败但错误信息不指向根因）
    pub fn parse_url(url: &str) -> DbResult<(String, String, String)> {
        match url::Url::parse(url) {
            Ok(parsed) => {
                let scheme = parsed.scheme();
                let host = parsed.host_str().unwrap_or("localhost");
                let port = parsed.port().unwrap_or(7687);
                let uri = format!("{scheme}://{host}:{port}");

                let user = parsed.username().to_string();
                let password = parsed.password().unwrap_or("").to_string();

                if user.is_empty() {
                    let env_user = std::env::var("NEO4J_USER").map_err(|_| {
                        DbError::Connection(sea_orm::DbErr::Custom(
                            "neo4j URL has no credentials and NEO4J_USER env var is not set".to_string(),
                        ))
                    })?;
                    let env_pass = std::env::var("NEO4J_PASSWORD").map_err(|_| {
                        DbError::Connection(sea_orm::DbErr::Custom(
                            "neo4j URL has no credentials and NEO4J_PASSWORD env var is not set".to_string(),
                        ))
                    })?;
                    Ok((uri, env_user, env_pass))
                } else {
                    Ok((uri, user, password))
                }
            }
            Err(_) => {
                // 非 URL 格式，原样返回 uri，凭据从环境变量读取
                // 错误信息不回显原始 URL，避免凭据泄露（M-49 修复）
                let env_user = std::env::var("NEO4J_USER").map_err(|_| {
                    DbError::Connection(sea_orm::DbErr::Custom(
                        "neo4j URL is not a valid URL and NEO4J_USER env var is not set".to_string(),
                    ))
                })?;
                let env_pass = std::env::var("NEO4J_PASSWORD").map_err(|_| {
                    DbError::Connection(sea_orm::DbErr::Custom(
                        "neo4j URL is not a valid URL and NEO4J_PASSWORD env var is not set".to_string(),
                    ))
                })?;
                Ok((url.to_string(), env_user, env_pass))
            }
        }
    }

    /// 创建占位连接（仅用于不连接服务器的单元测试）
    ///
    /// 返回的连接 `graph` 为 `None`，所有 `GraphConnection` 方法返回明确错误。
    #[cfg(test)]
    pub(crate) fn new_placeholder() -> Self {
        Self { graph: None }
    }
}

impl std::fmt::Debug for Neo4jConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Neo4jConnection")
            .field("connected", &self.graph.is_some())
            .finish()
    }
}

// ============================================================================
// GraphConnection impl
// ============================================================================

#[async_trait::async_trait]
impl GraphConnection for Neo4jConnection {
    async fn execute_cypher(&self, cypher: &str) -> DbResult<GraphExecResult> {
        let graph = self
            .graph
            .as_ref()
            .ok_or_else(|| DbError::Config("Neo4jConnection not connected (placeholder)".to_string()))?;

        let mut stream = graph
            .execute(neo4rs::query(cypher))
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j execute: {e}"))))?;

        let mut rows = Vec::new();
        while let Some(row) = stream
            .next()
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j row fetch: {e}"))))?
        {
            let json_val: serde_json::Value = row
                .to::<serde_json::Value>()
                .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j row deserialize: {e}"))))?;

            let columns = match json_val {
                serde_json::Value::Object(map) => map
                    .into_iter()
                    .map(|(name, val)| (name, GraphValue::Scalar(val)))
                    .collect(),
                other => vec![("value".to_string(), GraphValue::Scalar(other))],
            };
            rows.push(GraphRow { columns });
        }

        Ok(GraphExecResult::Query(GraphQueryResult { rows, rows_affected: 0 }))
    }

    /// vuln-0005 修复：使用 `neo4rs::query(cypher).params(...)` 执行参数化查询
    ///
    /// neo4rs 在客户端将参数以 Bolt 协议的 typed value 形式发送到服务器，
    /// 服务器不会将参数值解析为 Cypher 代码，从根本上防止注入。
    async fn execute_cypher_with_params(
        &self,
        cypher: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> DbResult<GraphExecResult> {
        let graph = self
            .graph
            .as_ref()
            .ok_or_else(|| DbError::Config("Neo4jConnection not connected (placeholder)".to_string()))?;

        let mut query = neo4rs::query(cypher);
        for (key, value) in params {
            query = attach_param_to_query(query, &key, value);
        }

        let mut stream = graph
            .execute(query)
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j execute (params): {e}"))))?;

        let mut rows = Vec::new();
        while let Some(row) = stream
            .next()
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j row fetch: {e}"))))?
        {
            let json_val: serde_json::Value = row
                .to::<serde_json::Value>()
                .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j row deserialize: {e}"))))?;

            let columns = match json_val {
                serde_json::Value::Object(map) => map
                    .into_iter()
                    .map(|(name, val)| (name, GraphValue::Scalar(val)))
                    .collect(),
                other => vec![("value".to_string(), GraphValue::Scalar(other))],
            };
            rows.push(GraphRow { columns });
        }

        Ok(GraphExecResult::Query(GraphQueryResult { rows, rows_affected: 0 }))
    }

    async fn health_check(&self) -> DbResult<()> {
        let result = self.execute_cypher("RETURN 1").await?;
        match result {
            GraphExecResult::Query(q) if !q.rows.is_empty() => Ok(()),
            GraphExecResult::Query(_) => Err(DbError::Connection(sea_orm::DbErr::Custom(
                "neo4j health check returned no rows".to_string(),
            ))),
            GraphExecResult::Write { .. } => Ok(()),
        }
    }

    async fn begin_graph_txn(&self) -> DbResult<Box<dyn GraphTransaction + Send>> {
        let graph = self
            .graph
            .as_ref()
            .ok_or_else(|| DbError::Config("Neo4jConnection not connected (placeholder)".to_string()))?;

        let txn = graph
            .start_txn()
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j start_txn: {e}"))))?;

        Ok(Box::new(Neo4jTransaction {
            txn: AsyncMutex::new(Some(txn)),
        }))
    }

    fn backend_name(&self) -> &'static str {
        "neo4j"
    }
}

// ============================================================================
// Neo4jTransaction
// ============================================================================

/// Neo4j 图数据库事务
///
/// 包装 `neo4rs::Txn`，通过 `AsyncMutex<Option<Txn>>` 提供内部可变性。
/// `execute_cypher` 锁定 mutex 调用 `Txn::execute`（需要 `&mut self`）。
/// `commit`/`rollback` 从 `Option` 中取出 `Txn` 并消耗它。
///
/// # Drop 行为（FM-2.2 修复）
///
/// `neo4rs::Txn` 的 Drop 只归还连接到池，**不发送 ROLLBACK 消息**，服务器端事务
/// 会一直保持到超时。此处 Drop 时尝试 `try_lock` + `spawn` rollback task：
/// - 锁可用且在 tokio runtime 中：spawn 异步 rollback
/// - 锁被持有（有操作正在进行）或不在 runtime 中：`Txn` 随 Drop 归还连接，
///   服务器端事务在超时后回滚（neo4j 默认 60s）
pub struct Neo4jTransaction {
    /// 事务句柄（None 表示已 commit/rollback）
    txn: AsyncMutex<Option<neo4rs::Txn>>,
}

impl Drop for Neo4jTransaction {
    fn drop(&mut self) {
        // FM-2.2 修复：未显式 commit/rollback 的事务在 Drop 时尝试 rollback
        if let Ok(mut guard) = self.txn.try_lock() {
            if let Some(txn) = guard.take() {
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    // 在 tokio runtime 中：spawn 异步 rollback，不阻塞 Drop
                    handle.spawn(async move {
                        let _ = txn.rollback().await;
                    });
                }
                // 不在 runtime 中：txn 被 drop，连接归还到池，服务器端事务超时后回滚
            }
        }
        // 锁被持有（有操作正在进行）：txn 会随 Neo4jTransaction 一起 drop，连接归还到池
    }
}

#[async_trait::async_trait]
impl GraphTransaction for Neo4jTransaction {
    async fn commit(self: Box<Self>) -> DbResult<()> {
        let mut guard = self.txn.lock().await;
        let txn = guard.take().ok_or_else(|| {
            DbError::Connection(sea_orm::DbErr::Custom("neo4j transaction already consumed".to_string()))
        })?;
        drop(guard);
        txn.commit()
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j commit: {e}"))))
    }

    async fn rollback(self: Box<Self>) -> DbResult<()> {
        let mut guard = self.txn.lock().await;
        let txn = guard.take().ok_or_else(|| {
            DbError::Connection(sea_orm::DbErr::Custom("neo4j transaction already consumed".to_string()))
        })?;
        drop(guard);
        txn.rollback()
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j rollback: {e}"))))
    }

    async fn execute_cypher(&self, cypher: &str) -> DbResult<GraphExecResult> {
        let mut guard = self.txn.lock().await;
        let txn = guard.as_mut().ok_or_else(|| {
            DbError::Connection(sea_orm::DbErr::Custom("neo4j transaction already consumed".to_string()))
        })?;

        let mut stream = txn
            .execute(neo4rs::query(cypher))
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j txn execute: {e}"))))?;

        let mut rows = Vec::new();
        while let Some(row) = stream
            .next(&mut *txn)
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j txn row fetch: {e}"))))?
        {
            let json_val: serde_json::Value = row
                .to::<serde_json::Value>()
                .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j txn row deserialize: {e}"))))?;

            let columns = match json_val {
                serde_json::Value::Object(map) => map
                    .into_iter()
                    .map(|(name, val)| (name, GraphValue::Scalar(val)))
                    .collect(),
                other => vec![("value".to_string(), GraphValue::Scalar(other))],
            };
            rows.push(GraphRow { columns });
        }

        Ok(GraphExecResult::Query(GraphQueryResult { rows, rows_affected: 0 }))
    }

    /// vuln-0005 修复：在事务内执行参数化 Cypher 查询
    ///
    /// 通过 `neo4rs::query(cypher).param(key, value)` 在事务上下文内执行，
    /// 参数以 Bolt 协议 typed value 形式发送到服务器，防止注入。
    async fn execute_cypher_with_params(
        &self,
        cypher: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> DbResult<GraphExecResult> {
        let mut guard = self.txn.lock().await;
        let txn = guard.as_mut().ok_or_else(|| {
            DbError::Connection(sea_orm::DbErr::Custom("neo4j transaction already consumed".to_string()))
        })?;

        let mut query = neo4rs::query(cypher);
        for (key, value) in params {
            query = attach_param_to_query(query, &key, value);
        }

        let mut stream = txn
            .execute(query)
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j txn execute (params): {e}"))))?;

        let mut rows = Vec::new();
        while let Some(row) = stream
            .next(&mut *txn)
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j txn row fetch: {e}"))))?
        {
            let json_val: serde_json::Value = row
                .to::<serde_json::Value>()
                .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("neo4j txn row deserialize: {e}"))))?;

            let columns = match json_val {
                serde_json::Value::Object(map) => map
                    .into_iter()
                    .map(|(name, val)| (name, GraphValue::Scalar(val)))
                    .collect(),
                other => vec![("value".to_string(), GraphValue::Scalar(other))],
            };
            rows.push(GraphRow { columns });
        }

        Ok(GraphExecResult::Query(GraphQueryResult { rows, rows_affected: 0 }))
    }
}

// ============================================================================
// 内部辅助函数
// ============================================================================

/// 将 `serde_json::Value` 转换为 `BoltType` 并 attach 到 query（vuln-0005 修复）
///
/// 转换规则：
/// - `Null` → `BoltType::Null`
/// - `Bool` → `BoltType::Boolean`
/// - 整数（i64 范围内）→ `BoltType::Integer`
/// - 浮点 → `BoltType::Float`
/// - 字符串 → `BoltType::String`
/// - 数组 → `BoltType::List`（递归转换元素）
/// - 对象 → `BoltType::Map`（递归转换值）
fn attach_param_to_query(mut query: neo4rs::Query, key: &str, value: serde_json::Value) -> neo4rs::Query {
    let bolt_value = json_to_bolt_type(value);
    query = query.param(key, bolt_value);
    query
}

/// 将 `serde_json::Value` 转换为 `neo4rs::BoltType`
fn json_to_bolt_type(value: serde_json::Value) -> neo4rs::BoltType {
    use neo4rs::BoltType;
    match value {
        serde_json::Value::Null => BoltType::Null(neo4rs::BoltNull),
        serde_json::Value::Bool(b) => BoltType::from(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                BoltType::from(i)
            } else if let Some(f) = n.as_f64() {
                BoltType::from(f)
            } else {
                // 大整数 / 特殊数值 → 序列化为字符串保留精度
                BoltType::from(n.to_string())
            }
        }
        serde_json::Value::String(s) => BoltType::from(s),
        serde_json::Value::Array(arr) => {
            let bolt_list: Vec<neo4rs::BoltType> = arr.into_iter().map(json_to_bolt_type).collect();
            BoltType::List(neo4rs::BoltList::from(bolt_list))
        }
        serde_json::Value::Object(obj) => {
            let mut bolt_map = neo4rs::BoltMap::default();
            for (k, v) in obj {
                bolt_map.put(k.into(), json_to_bolt_type(v));
            }
            BoltType::Map(bolt_map)
        }
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ===== parse_url 测试 =====

    #[test]
    fn test_parse_url_neo4j_with_credentials() {
        let (uri, user, pass) = Neo4jConnection::parse_url("neo4j://user:pass@localhost:7687")
            .expect("parse_url with credentials should succeed");
        assert_eq!(uri, "neo4j://localhost:7687");
        assert_eq!(user, "user");
        assert_eq!(pass, "pass");
    }

    #[test]
    fn test_parse_url_neo4j_plus_s_with_credentials() {
        let (uri, user, pass) = Neo4jConnection::parse_url("neo4j+s://admin:secret@host:7687")
            .expect("parse_url with credentials should succeed");
        assert_eq!(uri, "neo4j+s://host:7687");
        assert_eq!(user, "admin");
        assert_eq!(pass, "secret");
    }

    #[test]
    fn test_parse_url_neo4j_no_credentials_returns_error() {
        // 无凭据且环境变量未设置时必须返回明确错误（LOW-001 修复）
        let result = Neo4jConnection::parse_url("neo4j://localhost:7687");
        assert!(result.is_err(), "parse_url without credentials should error");
    }

    #[test]
    fn test_parse_url_neo4j_default_port() {
        let (uri, _, _) = Neo4jConnection::parse_url("neo4j://user:pass@localhost")
            .expect("parse_url with credentials should succeed");
        assert_eq!(uri, "neo4j://localhost:7687");
    }

    #[test]
    fn test_parse_url_invalid_returns_error() {
        // 非 URL 格式且环境变量未设置时必须返回明确错误（LOW-001 修复）
        let result = Neo4jConnection::parse_url("not_a_url");
        assert!(result.is_err(), "parse_url with invalid URL should error");
    }

    #[test]
    fn test_parse_url_password_with_special_chars() {
        let (uri, user, pass) = Neo4jConnection::parse_url("neo4j://user:p%40ss@host:7687")
            .expect("parse_url with credentials should succeed");
        assert_eq!(uri, "neo4j://host:7687");
        assert_eq!(user, "user");
        // url crate 不对 password 做百分号解码，保持原始编码形式
        assert_eq!(pass, "p%40ss");
    }

    // ===== backend_name 测试 =====

    #[test]
    fn test_neo4j_backend_name() {
        let conn = Neo4jConnection::new_placeholder();
        assert_eq!(conn.backend_name(), "neo4j");
    }

    // ===== Debug / Clone 测试 =====

    #[test]
    fn test_neo4j_debug_format_placeholder() {
        let conn = Neo4jConnection::new_placeholder();
        let debug_str = format!("{:?}", conn);
        assert!(
            debug_str.contains("Neo4jConnection"),
            "Debug should contain 'Neo4jConnection': {debug_str}"
        );
        assert!(
            debug_str.contains("connected: false"),
            "Debug should show connected: false for placeholder: {debug_str}"
        );
    }

    #[test]
    fn test_neo4j_clone_preserves_backend_name() {
        let conn = Neo4jConnection::new_placeholder();
        let cloned = conn.clone();
        assert_eq!(conn.backend_name(), cloned.backend_name());
    }

    // ===== placeholder GraphConnection 方法返回错误 =====

    #[tokio::test]
    async fn test_neo4j_placeholder_execute_cypher_returns_error() {
        let conn = Neo4jConnection::new_placeholder();
        let result = conn.execute_cypher("RETURN 1").await;
        assert!(result.is_err(), "placeholder should return error");
        let err = result.unwrap_err();
        match err {
            DbError::Config(msg) => assert!(
                msg.contains("not connected"),
                "error should mention 'not connected': {msg}"
            ),
            other => panic!("expected DbError::Config, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_neo4j_placeholder_health_check_returns_error() {
        let conn = Neo4jConnection::new_placeholder();
        let result = conn.health_check().await;
        assert!(result.is_err(), "placeholder should return error");
    }

    #[tokio::test]
    async fn test_neo4j_placeholder_begin_graph_txn_returns_error() {
        let conn = Neo4jConnection::new_placeholder();
        let result = conn.begin_graph_txn().await;
        assert!(result.is_err(), "placeholder should return error");
    }

    // ===== 集成测试（需要 Neo4j 服务器，标记 #[ignore]） =====

    /// 从环境变量获取 Neo4j 连接信息，未设置则返回 None
    async fn neo4j_test_connection() -> Option<Neo4jConnection> {
        let url = std::env::var("NEO4J_URL").ok()?;
        // parse_url 现在返回 Result，无凭据时从环境变量读取
        let (uri, user, password) = Neo4jConnection::parse_url(&url).ok()?;

        Neo4jConnection::new(&uri, &user, &password).await.ok()
    }

    #[tokio::test]
    #[ignore = "需要 Neo4j 服务器，设置 NEO4J_URL/NEO4J_USER/NEO4J_PASSWORD 环境变量后运行"]
    async fn test_neo4j_execute_return_1() {
        let conn = neo4j_test_connection()
            .await
            .expect("NEO4J_URL not set or connection failed");
        let result = conn
            .execute_cypher("RETURN 1 AS n")
            .await
            .expect("execute_cypher RETURN 1 should succeed");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should return 1 row");
                let value = &q.rows[0].columns[0].1;
                match value {
                    GraphValue::Scalar(s) => assert_eq!(s, &serde_json::json!(1), "should return scalar 1"),
                    other => panic!("expected Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    #[tokio::test]
    #[ignore = "需要 Neo4j 服务器，设置 NEO4J_URL/NEO4J_USER/NEO4J_PASSWORD 环境变量后运行"]
    async fn test_neo4j_health_check() {
        let conn = neo4j_test_connection()
            .await
            .expect("NEO4J_URL not set or connection failed");
        conn.health_check()
            .await
            .expect("health check should pass on connected Neo4j");
    }

    #[tokio::test]
    #[ignore = "需要 Neo4j 服务器，设置 NEO4J_URL/NEO4J_USER/NEO4J_PASSWORD 环境变量后运行"]
    async fn test_neo4j_txn_commit() {
        let conn = neo4j_test_connection()
            .await
            .expect("NEO4J_URL not set or connection failed");
        // 清理可能存在的测试数据
        let _ = conn.execute_cypher("MATCH (n:T029Test) DETACH DELETE n").await;

        let txn = conn.begin_graph_txn().await.expect("begin_graph_txn should succeed");
        txn.execute_cypher("CREATE (n:T029Test {name: 'Alice'})")
            .await
            .expect("create in txn should succeed");
        txn.commit().await.expect("commit should succeed");

        // 验证事务提交后数据可见
        let result = conn
            .execute_cypher("MATCH (n:T029Test) RETURN n.name AS name")
            .await
            .expect("match after commit should succeed");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should see 1 node after commit");
                let name = &q.rows[0].columns[0].1;
                match name {
                    GraphValue::Scalar(serde_json::Value::String(s)) => {
                        assert_eq!(s, "Alice", "node name should be Alice")
                    }
                    other => panic!("expected String Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }

        // 清理
        let _ = conn.execute_cypher("MATCH (n:T029Test) DETACH DELETE n").await;
    }

    #[tokio::test]
    #[ignore = "需要 Neo4j 服务器，设置 NEO4J_URL/NEO4J_USER/NEO4J_PASSWORD 环境变量后运行"]
    async fn test_neo4j_txn_rollback() {
        let conn = neo4j_test_connection()
            .await
            .expect("NEO4J_URL not set or connection failed");
        // 使用独立标签避免与 commit 测试并行运行时的数据竞争
        let _ = conn.execute_cypher("MATCH (n:T029RollbackTest) DETACH DELETE n").await;

        let txn = conn.begin_graph_txn().await.expect("begin_graph_txn should succeed");
        txn.execute_cypher("CREATE (n:T029RollbackTest {name: 'Bob'})")
            .await
            .expect("create in txn should succeed");
        txn.rollback().await.expect("rollback should succeed");

        // 验证事务回滚后数据不可见
        let result = conn
            .execute_cypher("MATCH (n:T029RollbackTest) RETURN n.name AS name")
            .await
            .expect("match after rollback should succeed");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 0, "should see 0 nodes after rollback")
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }

        // 清理
        let _ = conn.execute_cypher("MATCH (n:T029RollbackTest) DETACH DELETE n").await;
    }
}
