// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Ladybug 图数据库连接（stub，T026 将填充真实实现）
//!
//! 当前为占位类型，所有 [`GraphConnection`] 方法返回明确错误。
//! T026 将替换为基于 `lbug::Database` + `Mutex<Vec<lbug::Connection>>` 的真实实现。

use crate::database::graph::{GraphConnection, GraphExecResult, GraphTransaction};
use crate::foundation::{DbError, DbResult};
use async_trait::async_trait;

/// Ladybug 图数据库连接（stub）
///
/// 这是 T024 创建的占位类型，T026 将替换为真实实现
/// （`Arc<lbug::Database>` + `Mutex<Vec<lbug::Connection>>` 连接池模式）。
#[derive(Debug, Clone)]
pub struct LadybugConnection {
    _private: (),
}

impl LadybugConnection {
    /// 创建占位连接（仅用于 T024 的 DbConnection 接线测试）
    ///
    /// T026 将实现真实的 `new(path, pool_size) -> DbResult<Self>`。
    #[cfg(test)]
    pub(crate) fn new_placeholder() -> Self {
        Self { _private: () }
    }
}

#[async_trait]
impl GraphConnection for LadybugConnection {
    async fn execute_cypher(&self, _cypher: &str) -> DbResult<GraphExecResult> {
        Err(DbError::Config(
            "LadybugConnection not yet implemented (T026 will implement)".to_string(),
        ))
    }

    async fn health_check(&self) -> DbResult<()> {
        Err(DbError::Config(
            "LadybugConnection not yet implemented (T026 will implement)".to_string(),
        ))
    }

    async fn begin_graph_txn(&self) -> DbResult<Box<dyn GraphTransaction + Send>> {
        Err(DbError::Config(
            "LadybugConnection not yet implemented (T026 will implement)".to_string(),
        ))
    }

    fn backend_name(&self) -> &'static str {
        "ladybug"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ladybug_stub_execute_cypher_returns_error() {
        let conn = LadybugConnection::new_placeholder();
        let result = conn.execute_cypher("RETURN 1").await;
        assert!(result.is_err(), "stub should return error");
    }

    #[tokio::test]
    async fn test_ladybug_stub_health_check_returns_error() {
        let conn = LadybugConnection::new_placeholder();
        let result = conn.health_check().await;
        assert!(result.is_err(), "stub should return error");
    }

    #[tokio::test]
    async fn test_ladybug_stub_begin_graph_txn_returns_error() {
        let conn = LadybugConnection::new_placeholder();
        let result = conn.begin_graph_txn().await;
        assert!(result.is_err(), "stub should return error");
    }

    #[test]
    fn test_ladybug_backend_name() {
        let conn = LadybugConnection::new_placeholder();
        assert_eq!(conn.backend_name(), "ladybug");
    }
}
