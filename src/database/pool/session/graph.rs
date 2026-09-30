// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 大文件拆分：自 session.rs 按职责纯移动的 impl 块（行为不变）。

use super::*;

impl Session {
    /// 执行参数化 Cypher 查询（vuln-0005 修复）
    ///
    /// 与 `execute_cypher_in_transaction` 相同的事务分发逻辑，
    /// 但通过 `$name` 占位符 + `params` 映射传递用户输入，
    /// 底层使用 prepared statement，数据库不会将参数值解析为 Cypher 代码，
    /// 从根本上防止 Cypher 注入。
    ///
    /// # 参数
    ///
    /// * `cypher` - Cypher 查询语句（含 `$param` 占位符）
    /// * `params` - 参数映射（key 必须与 Cypher 中的 `$param` 名称一致）
    ///
    /// # 返回
    ///
    /// 图执行结果（Query 或 Write）
    ///
    /// # Errors
    ///
    /// - 非 admin 角色调用时返回 `DbError::Permission`
    /// - 连接不是图连接时返回 `DbError::Connection`
    /// - 查询语法错误或执行失败时返回对应的 `DbError`
    /// - Cypher 包含多语句/注释/危险过程时返回 `DbError::Permission`（vuln-0005）
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let mut params = HashMap::new();
    /// params.insert("name".to_string(), serde_json::json!("Alice"));
    /// params.insert("age".to_string(), serde_json::json!(30));
    /// let result = session.execute_cypher_with_params(
    ///     "CREATE (n:User {name: $name, age: $age})",
    ///     params,
    /// ).await?;
    /// ```
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    pub async fn execute_cypher_with_params(
        &self,
        cypher: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> DbResult<crate::database::graph::GraphExecResult> {
        // MD-1 修复：委托给 execute_cypher_in_transaction helper 复用事务分发逻辑
        self.execute_cypher_in_transaction(cypher, Some(params))
            .await
    }

    /// 执行 Cypher 查询的内部 helper（MD-1 提取，统一事务分发逻辑）
    ///
    /// `execute_cypher_with_params` 的完整事务分发流程：
    /// 1. 注入防护（vuln-0005）
    /// 2. 图权限检查
    /// 3. 取连接 + 获取图操作互斥锁（串行化 take → put back）
    /// 4. 短锁 take graph_transaction（含 poisoned 检查）
    /// 5. 事务内执行：PoisonGuard + take → await → put back
    /// 6. 事务外执行：直接在连接上调用
    ///
    /// # 参数
    ///
    /// * `cypher` - Cypher 查询语句
    /// * `params` - 参数映射（key 对应 Cypher 中的 `$param` 占位符）
    ///
    /// # 设计说明
    ///
    /// 使用 `Option<HashMap>` 区分两种操作。`params.take()` 在互斥分支中消耗参数，
    /// 避免在调用路径强制构造空 HashMap，也无需 clone。
    ///
    /// # HD-2 误报说明（架构审查）
    ///
    /// 审查曾标记"Session 直接依赖 Ladybug/Neo4j 具体实现"为 HIGH 架构问题。此为误报：
    /// 所有图操作通过 `conn.as_graph()?` 获取 `&dyn GraphConnection` trait 对象，
    /// Session 仅持有 `DbConnection` 枚举与 `&dyn GraphConnection`/`Box<dyn GraphTransaction>`
    /// trait 对象，不直接依赖任何具体类型。`begin_transaction` 与本 helper 的图路径
    /// 均通过 trait 方法分发，Ladybug/Neo4j 实现细节封装在各自 `ladybug_conn`/`neo4j_conn`
    /// 模块内。这与 `sea_orm::DatabaseTransaction` 的依赖方式一致：通过 trait 对象解耦，
    /// 而非 concrete type。新增图后端只需实现 `GraphConnection`/`GraphTransaction` trait，
    /// Session 代码无需改动（开放-封闭原则）。
    #[cfg(any(feature = "ladybug", feature = "neo4j"))]
    async fn execute_cypher_in_transaction(
        &self,
        cypher: &str,
        mut params: Option<HashMap<String, serde_json::Value>>,
    ) -> DbResult<crate::database::graph::GraphExecResult> {
        // vuln-0005 修复：注入防护检查（长度限制 + 危险模式检测）
        validate_cypher_safety(cypher)?;

        // 图权限检查（admin 角色由 GraphPermissionContext 内部处理）
        #[cfg(feature = "permission")]
        {
            let graph_perm_ctx = crate::access::permission::GraphPermissionContext::new(
                &self.role,
                &self.pool_inner.admin_role,
            );
            graph_perm_ctx
                .check_graph_access(crate::access::permission::PermissionAction::Traverse)?;
        }

        // 取连接
        let conn = self.connection.as_ref().ok_or_else(|| {
            DbError::Config(
                "Connection not available - Session may have been invalidated".to_string(),
            )
        })?;

        // 获取图操作互斥锁（防止并发 take → put back 窗口绕过事务隔离）
        let _graph_op_guard = self.graph_op_mutex.lock().await;

        // 检查是否在图事务中（短锁 take → 锁外执行 → 短锁 put back）
        let graph_txn = {
            let mut state = self.state.write().await;
            if state.graph_txn_poisoned {
                return Err(DbError::Transaction(
                    "Graph transaction is poisoned due to previous panic; \
                     Session must be dropped and recreated"
                        .to_string(),
                ));
            }
            state.graph_transaction.take()
        };

        if let Some(graph_txn) = graph_txn {
            // PoisonGuard（panic 时标记事务为 poisoned，防止丢失句柄后绕过事务隔离）
            struct PoisonGuard<'a> {
                state: &'a RwLock<SessionState>,
                armed: bool,
            }
            impl<'a> Drop for PoisonGuard<'a> {
                fn drop(&mut self) {
                    if self.armed
                        && let Ok(mut state) = self.state.try_write()
                    {
                        state.graph_txn_poisoned = true;
                    }
                }
            }

            let mut guard = PoisonGuard {
                state: &self.state,
                armed: true,
            };
            // 互斥分支：params.take() 确保参数只被消耗一次（None → execute_cypher / Some → with_params）
            let result = if let Some(p) = params.take() {
                graph_txn.execute_cypher_with_params(cypher, p).await
            } else {
                graph_txn.execute_cypher(cypher).await
            };
            guard.armed = false;

            let mut state = self.state.write().await;
            state.graph_transaction = Some(graph_txn);
            return result;
        }

        // 不在事务中，直接在连接上执行
        let graph = conn.as_graph()?;
        if let Some(p) = params.take() {
            graph.execute_cypher_with_params(cypher, p).await
        } else {
            graph.execute_cypher(cypher).await
        }
    }
}
