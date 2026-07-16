// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Ladybug 图数据库连接
//!
//! 提供 Ladybug（原 Kuzu）嵌入式图数据库的异步连接抽象，通过 `tokio::task::spawn_blocking`
//! 桥接 lbug 的同步 API 到 Tokio 异步运行时。
//!
//! # 架构说明
//!
//! 与 DuckDbConnection 不同，LadybugConnection 不使用 `Vec<Connection>` 连接池模式。
//! 原因：`lbug::Connection<'a>` 的生命周期绑定到 `&'a Database`，无法在同一 struct 中
//! 同时存储 `Arc<Database>` 和 `Vec<Connection<'a>>`（自引用问题）。
//!
//! 替代方案：存储 `Arc<Database>`，每次操作在 `spawn_blocking` 内创建临时 `Connection`。
//! `Connection::new(&db)` 开销低（仅打开同一进程内数据库的会话），配合 `Semaphore`
//! 限制并发数，等效于连接池模式。
//!
//! # 事务模型
//!
//! lbug 0.18.1 无原生事务 API（`begin_txn`/`commit`/`rollback`），事务通过 Cypher 语句
//! `BEGIN TRANSACTION` / `COMMIT` / `ROLLBACK` 控制。显式事务要求所有语句在同一
//! `Connection` 上执行，因此 [`LadybugTransaction`] 使用 actor 模式：通过 channel
//! 将命令发送到持有 `Connection` 的 blocking 线程，确保事务内所有操作使用同一连接。
//!
//! # 线程安全
//!
//! `lbug::Database` 是 `Send + Sync`（unsafe impl），`lbug::Connection` 是 `Send + Sync`。
//! 通过 `Arc<Database>` 共享数据库实例，`Arc<Semaphore>` 限制并发数。

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::database::graph::{
    GraphConnection, GraphExecResult, GraphNode, GraphQueryResult, GraphRel, GraphRow, GraphTransaction, GraphValue,
};
use crate::foundation::{DbError, DbResult};

/// 默认并发连接数
#[cfg(test)]
const DEFAULT_POOL_SIZE: usize = 4;

/// Ladybug 图数据库连接
///
/// 封装 `lbug::Database`，通过 `Semaphore` 限制并发查询数。
/// 每次 `execute_cypher` 在 `spawn_blocking` 内创建临时 `Connection` 执行查询。
///
/// # 示例
///
/// ```ignore
/// use dbnexus::database::LadybugConnection;
///
/// let conn = LadybugConnection::new(":memory:", 4)?;
/// ```
#[derive(Clone)]
pub struct LadybugConnection {
    /// 数据库实例（Arc 共享，Connection 在 spawn_blocking 内按需创建）
    db: Arc<lbug::Database>,
    /// 并发限制信号量（等效于连接池大小）
    spawn_permit: Arc<Semaphore>,
    /// 配置的并发数
    pool_size: usize,
}

impl LadybugConnection {
    /// 创建新的 Ladybug 连接（默认并发数 4）
    ///
    /// # 参数
    ///
    /// * `url` - Ladybug 连接字符串，支持：
    ///   - `:memory:` 或 `ladybug::memory:` — 内存数据库
    ///   - `ladybug:path/to/file` — 文件数据库
    ///   - `ladybug://path/to/file` — 文件数据库（URL 格式）
    ///   - 其他 — 原样作为文件路径处理
    /// * `pool_size` - 并发查询数（Semaphore 许可证数）
    ///
    /// # 错误
    ///
    /// 数据库创建失败时返回 `DbError::Connection`
    pub fn new(url: &str, pool_size: usize) -> DbResult<Self> {
        Self::with_pool_size(url, pool_size)
    }

    /// 创建指定并发数的 Ladybug 连接
    ///
    /// # 参数
    ///
    /// * `url` - Ladybug 连接字符串（见 [`new`](Self::new)）
    /// * `pool_size` - 并发查询数（最小值 1）
    pub fn with_pool_size(url: &str, pool_size: usize) -> DbResult<Self> {
        let pool_size = pool_size.max(1);
        let db_path = Self::parse_url(url);
        // 路径遍历校验（M-48 修复）：拒绝包含 .. 的路径，防止打开任意文件
        if db_path != ":memory:" && db_path.contains("..") {
            return Err(DbError::Connection(sea_orm::DbErr::Custom(
                "ladybug database path contains path traversal characters '..': rejected for security".to_string(),
            )));
        }
        let db = lbug::Database::new(&db_path, lbug::SystemConfig::default())
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("ladybug Database::new failed: {e}"))))?;
        Ok(Self {
            db: Arc::new(db),
            spawn_permit: Arc::new(Semaphore::new(pool_size)),
            pool_size,
        })
    }

    /// 解析 Ladybug URL 为数据库路径
    ///
    /// 支持的格式：
    /// - `:memory:` → `:memory:`
    /// - `ladybug::memory:` → `:memory:`
    /// - `ladybug:path` → `path`
    /// - `ladybug://path` → `path`
    /// - 其他 → 原样返回（兼容直接文件路径）
    fn parse_url(url: &str) -> String {
        let lower = url.to_lowercase();
        if lower == ":memory:" || lower == "ladybug::memory:" {
            return ":memory:".to_string();
        }
        if let Some(rest) = url.strip_prefix("ladybug:") {
            return rest.trim_start_matches('/').to_string();
        }
        url.to_string()
    }

    /// 获取并发数
    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    /// 获取 Semaphore 许可证，限制 spawn_blocking 并发数
    async fn acquire_permit(&self) -> DbResult<tokio::sync::SemaphorePermit<'_>> {
        self.spawn_permit
            .acquire()
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("Semaphore closed".to_string())))
    }
}

impl std::fmt::Debug for LadybugConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LadybugConnection")
            .field("pool_size", &self.pool_size)
            .field("max_concurrency", &self.pool_size)
            .finish()
    }
}

// ============================================================================
// GraphConnection impl
// ============================================================================

#[async_trait::async_trait]
impl GraphConnection for LadybugConnection {
    async fn execute_cypher(&self, cypher: &str) -> DbResult<GraphExecResult> {
        let permit = self.acquire_permit().await?;
        let db = self.db.clone();
        let cypher_owned = cypher.to_string();
        let handle: JoinHandle<DbResult<GraphExecResult>> = tokio::task::spawn_blocking(move || {
            let conn = lbug::Connection::new(&db)
                .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("ladybug Connection::new: {e}"))))?;
            execute_cypher_on_conn(&conn, &cypher_owned)
        });
        let result = handle
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("spawn_blocking join: {e}"))))?;
        drop(permit);
        result
    }

    /// vuln-0005 修复：使用 lbug prepared statement 执行参数化查询
    ///
    /// 流程：`conn.prepare(cypher)` → `conn.execute(&mut prepared, params)`，
    /// 参数值通过 FFI 以 `lbug::Value` 形式传入，数据库不会将其解析为 Cypher 代码。
    async fn execute_cypher_with_params(
        &self,
        cypher: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> DbResult<GraphExecResult> {
        let permit = self.acquire_permit().await?;
        let db = self.db.clone();
        let cypher_owned = cypher.to_string();
        let handle: JoinHandle<DbResult<GraphExecResult>> = tokio::task::spawn_blocking(move || {
            let conn = lbug::Connection::new(&db)
                .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("ladybug Connection::new: {e}"))))?;
            execute_cypher_with_params_on_conn(&conn, &cypher_owned, &params)
        });
        let result = handle
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("spawn_blocking join: {e}"))))?;
        drop(permit);
        result
    }

    async fn health_check(&self) -> DbResult<()> {
        let result = self.execute_cypher("RETURN 1").await?;
        match result {
            GraphExecResult::Query(q) if !q.rows.is_empty() => Ok(()),
            GraphExecResult::Query(_) => Err(DbError::Connection(sea_orm::DbErr::Custom(
                "ladybug health check returned no rows".to_string(),
            ))),
            GraphExecResult::Write { .. } => Ok(()),
        }
    }

    async fn begin_graph_txn(&self) -> DbResult<Box<dyn GraphTransaction + Send>> {
        // FM-1.6 修复：事务也占用 Semaphore permit，防止并发事务数无上限
        let permit = self
            .spawn_permit
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("Semaphore closed".to_string())))?;

        let db = self.db.clone();
        let (tx, mut rx) = mpsc::channel::<TxnCommand>(8);

        let handle: JoinHandle<DbResult<()>> = tokio::task::spawn_blocking(move || {
            let conn = lbug::Connection::new(&db).map_err(|e| {
                DbError::Connection(sea_orm::DbErr::Custom(format!("ladybug txn Connection::new: {e}")))
            })?;
            conn.query("BEGIN TRANSACTION")
                .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("ladybug BEGIN TRANSACTION: {e}"))))?;

            while let Some(cmd) = rx.blocking_recv() {
                match cmd {
                    TxnCommand::Execute { cypher, reply } => {
                        let result = execute_cypher_on_conn(&conn, &cypher);
                        let _ = reply.send(result);
                    }
                    TxnCommand::ExecuteWithParams { cypher, params, reply } => {
                        let result = execute_cypher_with_params_on_conn(&conn, &cypher, &params);
                        let _ = reply.send(result);
                    }
                    TxnCommand::Commit { reply } => {
                        let result = conn
                            .query("COMMIT")
                            .map(|_| ())
                            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("ladybug COMMIT: {e}"))));
                        let _ = reply.send(result);
                        return Ok(());
                    }
                    TxnCommand::Rollback { reply } => {
                        let result = conn
                            .query("ROLLBACK")
                            .map(|_| ())
                            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("ladybug ROLLBACK: {e}"))));
                        let _ = reply.send(result);
                        return Ok(());
                    }
                }
            }
            // Channel closed without Commit/Rollback — 自动回滚
            let _ = conn.query("ROLLBACK");
            Ok(())
        });

        Ok(Box::new(LadybugTransaction {
            tx,
            handle: Some(handle),
            _permit: permit,
        }))
    }

    fn backend_name(&self) -> &'static str {
        "ladybug"
    }
}

// ============================================================================
// LadybugTransaction（事务 actor 模式）
// ============================================================================

/// 事务命令（通过 channel 发送到 blocking 线程）
enum TxnCommand {
    /// 执行 Cypher 查询
    Execute {
        /// Cypher 语句
        cypher: String,
        /// 结果回复通道
        reply: oneshot::Sender<DbResult<GraphExecResult>>,
    },
    /// 执行参数化 Cypher 查询（vuln-0005 修复）
    ExecuteWithParams {
        /// Cypher 语句（含 $param 占位符）
        cypher: String,
        /// 参数映射
        params: HashMap<String, serde_json::Value>,
        /// 结果回复通道
        reply: oneshot::Sender<DbResult<GraphExecResult>>,
    },
    /// 提交事务
    Commit {
        /// 结果回复通道
        reply: oneshot::Sender<DbResult<()>>,
    },
    /// 回滚事务
    Rollback {
        /// 结果回复通道
        reply: oneshot::Sender<DbResult<()>>,
    },
}

/// Ladybug 图数据库事务
///
/// 使用 actor 模式：通过 channel 将命令发送到持有 `Connection` 的 blocking 线程，
/// 确保事务内所有操作使用同一 `Connection`（lbug 事务要求）。
///
/// # Drop 行为
///
/// Drop 时 `tx` 被释放，channel 关闭，blocking 线程的 `blocking_recv` 收到 `None`
/// 后自动执行 `ROLLBACK`。`JoinHandle` 被 detach（blocking 线程继续运行到 ROLLBACK 完成）。
///
/// # 并发限制（FM-1.6 修复）
///
/// `_permit` 持有 `Semaphore` 许可证直到 `commit`/`rollback` 消耗 self，
/// 防止事务数无上限地绕过连接池并发限制。
pub struct LadybugTransaction {
    /// 命令发送通道
    tx: mpsc::Sender<TxnCommand>,
    /// blocking 线程句柄（commit/rollback 时 await）
    handle: Option<JoinHandle<DbResult<()>>>,
    /// Semaphore 许可证（FM-1.6 修复：事务期间持有，commit/rollback 时释放）
    _permit: tokio::sync::OwnedSemaphorePermit,
}

#[async_trait::async_trait]
impl GraphTransaction for LadybugTransaction {
    async fn commit(mut self: Box<Self>) -> DbResult<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(TxnCommand::Commit { reply: reply_tx })
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("txn thread already closed".to_string())))?;
        let result = reply_rx
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("txn reply dropped".to_string())))?;
        // 等待 blocking 线程退出
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
        result
    }

    async fn rollback(mut self: Box<Self>) -> DbResult<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(TxnCommand::Rollback { reply: reply_tx })
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("txn thread already closed".to_string())))?;
        let result = reply_rx
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("txn reply dropped".to_string())))?;
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
        result
    }

    async fn execute_cypher(&self, cypher: &str) -> DbResult<GraphExecResult> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(TxnCommand::Execute {
                cypher: cypher.to_string(),
                reply: reply_tx,
            })
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("txn thread already closed".to_string())))?;
        reply_rx
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("txn reply dropped".to_string())))?
    }

    /// vuln-0005 修复：通过 actor channel 转发参数化查询到 blocking 线程
    ///
    /// blocking 线程在事务上下文内调用 `conn.prepare` + `conn.execute`，
    /// 确保事务内所有操作使用同一 `Connection`（lbug 事务要求）。
    async fn execute_cypher_with_params(
        &self,
        cypher: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> DbResult<GraphExecResult> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(TxnCommand::ExecuteWithParams {
                cypher: cypher.to_string(),
                params,
                reply: reply_tx,
            })
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("txn thread already closed".to_string())))?;
        reply_rx
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("txn reply dropped".to_string())))?
    }
}

// ============================================================================
// 内部辅助函数
// ============================================================================

/// 在 blocking 线程内通过指定 Connection 执行 Cypher 查询
fn execute_cypher_on_conn(conn: &lbug::Connection, cypher: &str) -> DbResult<GraphExecResult> {
    let mut result = conn
        .query(cypher)
        .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("ladybug query: {e}"))))?;
    let column_names = result.get_column_names();
    let num_tuples = result.get_num_tuples();
    let mut rows = Vec::with_capacity(num_tuples as usize);
    for row_values in &mut result {
        let columns = column_names
            .iter()
            .zip(row_values.iter())
            .map(|(name, value)| (name.clone(), map_lbug_value(value)))
            .collect();
        rows.push(GraphRow { columns });
    }
    Ok(GraphExecResult::Query(GraphQueryResult { rows, rows_affected: 0 }))
}

/// 在 blocking 线程内通过指定 Connection 执行参数化 Cypher 查询（vuln-0005 修复）
///
/// 使用 `conn.prepare(cypher)` + `conn.execute(&mut prepared, params)` 走 prepared statement
/// 路径，参数值以 `lbug::Value` 形式通过 FFI 传入，数据库不会将其解析为 Cypher 代码，
/// 从根本上防止 Cypher 注入。
fn execute_cypher_with_params_on_conn(
    conn: &lbug::Connection,
    cypher: &str,
    params: &HashMap<String, serde_json::Value>,
) -> DbResult<GraphExecResult> {
    let mut prepared = conn
        .prepare(cypher)
        .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("ladybug prepare: {e}"))))?;

    // 将 serde_json::Value 转换为 lbug::Value（按 (key, value) 对的 vec 传入）
    let lbug_params: Vec<(&str, lbug::Value)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), json_to_lbug_value(v)))
        .collect();

    let mut result = conn
        .execute(&mut prepared, lbug_params)
        .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("ladybug execute (prepared): {e}"))))?;

    let column_names = result.get_column_names();
    let num_tuples = result.get_num_tuples();
    let mut rows = Vec::with_capacity(num_tuples as usize);
    for row_values in &mut result {
        let columns = column_names
            .iter()
            .zip(row_values.iter())
            .map(|(name, value)| (name.clone(), map_lbug_value(value)))
            .collect();
        rows.push(GraphRow { columns });
    }
    Ok(GraphExecResult::Query(GraphQueryResult { rows, rows_affected: 0 }))
}

/// 将 `serde_json::Value` 转换为 `lbug::Value`（用于 prepared statement 参数绑定）
///
/// 转换规则：
/// - `Null` → `lbug::Value::Null(LogicalType::Any)`（不指定类型）
/// - `Bool` → `lbug::Value::Bool`
/// - 整数（i64 范围内）→ `lbug::Value::Int64`
/// - 浮点 → `lbug::Value::Double`
/// - 字符串 → `lbug::Value::String`
/// - 其他（数组/对象/大整数）→ `lbug::Value::Json`（保留原始 JSON）
fn json_to_lbug_value(value: &serde_json::Value) -> lbug::Value {
    match value {
        serde_json::Value::Null => lbug::Value::Null(lbug::LogicalType::Any),
        serde_json::Value::Bool(b) => lbug::Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                lbug::Value::Int64(i)
            } else if let Some(u) = n.as_u64() {
                // u64 超出 i64 范围时，通过 JSON 保留精度
                if u <= i64::MAX as u64 {
                    lbug::Value::Int64(u as i64)
                } else {
                    lbug::Value::Json(value.clone())
                }
            } else if let Some(f) = n.as_f64() {
                lbug::Value::Double(f)
            } else {
                lbug::Value::Json(value.clone())
            }
        }
        serde_json::Value::String(s) => lbug::Value::String(s.clone()),
        // 数组/对象/其他复杂类型以 JSON 形式传入（lbug 原生支持 JSON 类型）
        other => lbug::Value::Json(other.clone()),
    }
}

/// 将 `lbug::Value` 映射为 [`GraphValue`]
fn map_lbug_value(value: &lbug::Value) -> GraphValue {
    match value {
        lbug::Value::Node(node) => GraphValue::Node(map_lbug_node(node)),
        lbug::Value::Rel(rel) => GraphValue::Rel(map_lbug_rel(rel)),
        lbug::Value::RecursiveRel { nodes, rels: _ } => {
            let path_nodes: Vec<GraphNode> = nodes.iter().map(map_lbug_node).collect();
            GraphValue::Path(path_nodes)
        }
        // 标量值
        lbug::Value::Null(_) => GraphValue::Scalar(serde_json::Value::Null),
        lbug::Value::Bool(b) => GraphValue::Scalar(serde_json::json!(b)),
        lbug::Value::Int8(i) => GraphValue::Scalar(serde_json::json!(i)),
        lbug::Value::Int16(i) => GraphValue::Scalar(serde_json::json!(i)),
        lbug::Value::Int32(i) => GraphValue::Scalar(serde_json::json!(i)),
        lbug::Value::Int64(i) => GraphValue::Scalar(serde_json::json!(i)),
        lbug::Value::UInt8(i) => GraphValue::Scalar(serde_json::json!(i)),
        lbug::Value::UInt16(i) => GraphValue::Scalar(serde_json::json!(i)),
        lbug::Value::UInt32(i) => GraphValue::Scalar(serde_json::json!(i)),
        lbug::Value::UInt64(i) => GraphValue::Scalar(serde_json::json!(i)),
        lbug::Value::Int128(i) => GraphValue::Scalar(serde_json::json!(i.to_string())),
        lbug::Value::Float(f) => GraphValue::Scalar(serde_json::json!(f)),
        lbug::Value::Double(f) => GraphValue::Scalar(serde_json::json!(f)),
        lbug::Value::String(s) => GraphValue::Scalar(serde_json::json!(s)),
        lbug::Value::Json(v) => GraphValue::Scalar(v.clone()),
        lbug::Value::Blob(b) => GraphValue::Scalar(serde_json::json!(b)),
        // 日期/时间 → ISO 字符串
        lbug::Value::Date(d) => GraphValue::Scalar(serde_json::json!(d.to_string())),
        lbug::Value::Interval(d) => GraphValue::Scalar(serde_json::json!(d.to_string())),
        lbug::Value::Timestamp(t)
        | lbug::Value::TimestampTz(t)
        | lbug::Value::TimestampNs(t)
        | lbug::Value::TimestampMs(t)
        | lbug::Value::TimestampSec(t) => GraphValue::Scalar(serde_json::json!(t.to_string())),
        lbug::Value::InternalID(id) => GraphValue::Scalar(serde_json::json!(format!("{}:{}", id.table_id, id.offset))),
        lbug::Value::UUID(u) => GraphValue::Scalar(serde_json::json!(u.to_string())),
        lbug::Value::Decimal(d) => GraphValue::Scalar(serde_json::json!(d.to_string())),
        // 复合类型 → JSON
        lbug::Value::List(_, items) | lbug::Value::Array(_, items) => {
            let arr: Vec<serde_json::Value> = items
                .iter()
                .map(|v| graph_value_to_json_scalar(&map_lbug_value(v)))
                .collect();
            GraphValue::Scalar(serde_json::Value::Array(arr))
        }
        lbug::Value::Struct(fields) => {
            let mut map = serde_json::Map::new();
            for (name, value) in fields {
                map.insert(name.clone(), graph_value_to_json_scalar(&map_lbug_value(value)));
            }
            GraphValue::Scalar(serde_json::Value::Object(map))
        }
        lbug::Value::Map(_, entries) => {
            let mut map = serde_json::Map::new();
            for (key, value) in entries {
                let key_str = match key {
                    lbug::Value::String(s) => s.clone(),
                    _ => format!("{key}"),
                };
                map.insert(key_str, graph_value_to_json_scalar(&map_lbug_value(value)));
            }
            GraphValue::Scalar(serde_json::Value::Object(map))
        }
        lbug::Value::Union { value, .. } => map_lbug_value(value),
    }
}

/// 将 `lbug::NodeVal` 映射为 [`GraphNode`]
fn map_lbug_node(node: &lbug::NodeVal) -> GraphNode {
    let mut map = serde_json::Map::new();
    for (key, value) in node.get_properties() {
        map.insert(key.clone(), graph_value_to_json_scalar(&map_lbug_value(value)));
    }
    GraphNode {
        label: node.get_label_name().clone(),
        properties: serde_json::Value::Object(map),
    }
}

/// 将 `lbug::RelVal` 映射为 [`GraphRel`]
fn map_lbug_rel(rel: &lbug::RelVal) -> GraphRel {
    let mut map = serde_json::Map::new();
    for (key, value) in rel.get_properties() {
        map.insert(key.clone(), graph_value_to_json_scalar(&map_lbug_value(value)));
    }
    GraphRel {
        rel_type: rel.get_label_name().clone(),
        src_id: rel.get_src_node().offset as i64,
        dst_id: rel.get_dst_node().offset as i64,
        properties: serde_json::Value::Object(map),
    }
}

/// 将 GraphValue 转换为 JSON 标量（用于嵌套在属性/列表中）
fn graph_value_to_json_scalar(value: &GraphValue) -> serde_json::Value {
    match value {
        GraphValue::Scalar(s) => s.clone(),
        GraphValue::Node(n) => serde_json::to_value(n).unwrap_or(serde_json::Value::Null),
        GraphValue::Rel(r) => serde_json::to_value(r).unwrap_or(serde_json::Value::Null),
        GraphValue::Path(p) => serde_json::to_value(p).unwrap_or(serde_json::Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== new / with_pool_size 测试 =====

    #[test]
    fn test_ladybug_new_memory_default_pool_size() {
        let conn = LadybugConnection::new(":memory:", DEFAULT_POOL_SIZE).expect("Failed to create memory connection");
        assert_eq!(conn.pool_size(), DEFAULT_POOL_SIZE);
        assert_eq!(DEFAULT_POOL_SIZE, 4);
    }

    #[test]
    fn test_ladybug_new_memory_via_url() {
        let conn = LadybugConnection::new("ladybug::memory:", 4).expect("Failed to create connection via URL");
        assert_eq!(conn.pool_size(), 4);
    }

    #[test]
    fn test_ladybug_with_pool_size_minimum_1() {
        let conn =
            LadybugConnection::with_pool_size(":memory:", 0).expect("Failed to create connection with pool_size=0");
        assert_eq!(conn.pool_size(), 1, "pool_size should be clamped to minimum 1");
    }

    #[test]
    fn test_ladybug_with_pool_size_custom() {
        let conn =
            LadybugConnection::with_pool_size(":memory:", 8).expect("Failed to create connection with pool_size=8");
        assert_eq!(conn.pool_size(), 8);
    }

    // ===== parse_url 测试 =====

    #[test]
    fn test_ladybug_parse_url_memory() {
        assert_eq!(LadybugConnection::parse_url(":memory:"), ":memory:");
    }

    #[test]
    fn test_ladybug_parse_url_ladybug_memory() {
        assert_eq!(LadybugConnection::parse_url("ladybug::memory:"), ":memory:");
    }

    #[test]
    fn test_ladybug_parse_url_ladybug_path() {
        assert_eq!(LadybugConnection::parse_url("ladybug:test.db"), "test.db");
    }

    #[test]
    fn test_ladybug_parse_url_ladybug_scheme() {
        assert_eq!(
            LadybugConnection::parse_url("ladybug://path/to/file.db"),
            "path/to/file.db"
        );
    }

    #[test]
    fn test_ladybug_parse_url_raw_path() {
        assert_eq!(LadybugConnection::parse_url("/absolute/path.db"), "/absolute/path.db");
    }

    #[test]
    fn test_ladybug_rejects_path_traversal() {
        // M-48 修复：路径遍历校验，拒绝包含 .. 的路径
        let result = LadybugConnection::with_pool_size("ladybug:../../etc/passwd", 2);
        assert!(result.is_err(), "path traversal should be rejected");
        let result = LadybugConnection::with_pool_size("../../etc/passwd", 2);
        assert!(result.is_err(), "path traversal should be rejected");
    }

    // ===== Clone + Debug 测试 =====

    #[test]
    fn test_ladybug_clone_shares_database() {
        let conn1 = LadybugConnection::new(":memory:", 2).expect("Failed to create connection");
        let conn2 = conn1.clone();
        assert_eq!(conn1.pool_size(), conn2.pool_size());
        assert_eq!(conn1.backend_name(), conn2.backend_name());
    }

    #[test]
    fn test_ladybug_debug_format() {
        let conn = LadybugConnection::new(":memory:", 4).expect("Failed to create connection");
        let debug_str = format!("{:?}", conn);
        assert!(debug_str.contains("LadybugConnection"));
        assert!(debug_str.contains("pool_size: 4"));
    }

    // ===== GraphConnection execute_cypher 测试 =====

    #[tokio::test]
    async fn test_ladybug_execute_return_1() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        let result = conn
            .execute_cypher("RETURN 1")
            .await
            .expect("execute_cypher should succeed");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should return 1 row");
                let value = &q.rows[0].columns[0].1;
                match value {
                    GraphValue::Scalar(s) => assert_eq!(s, &serde_json::json!(1)),
                    other => panic!("expected Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    #[tokio::test]
    async fn test_ladybug_execute_create_node_table() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        // DDL 语句在 lbug 中可能返回状态行或空结果集，关键是执行不报错
        let result = conn
            .execute_cypher("CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))")
            .await
            .expect("create node table should succeed");
        // lbug 所有语句都返回 Query 变体
        assert!(
            matches!(result, GraphExecResult::Query(_)),
            "expected Query variant, got {result:?}"
        );
        // 验证 DDL 生效：插入数据并查询
        conn.execute_cypher("CREATE (:Person {name: 'Test', age: 1})")
            .await
            .expect("create node should succeed");
        let match_result = conn
            .execute_cypher("MATCH (p:Person) RETURN p.name AS name")
            .await
            .expect("match should succeed");
        match match_result {
            GraphExecResult::Query(q) => assert_eq!(q.rows.len(), 1, "should see 1 person after DDL+CREATE"),
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    #[tokio::test]
    async fn test_ladybug_execute_create_and_match() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        conn.execute_cypher("CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))")
            .await
            .expect("create table");
        conn.execute_cypher("CREATE (:Person {name: 'Alice', age: 25})")
            .await
            .expect("create alice");
        conn.execute_cypher("CREATE (:Person {name: 'Bob', age: 30})")
            .await
            .expect("create bob");

        let result = conn
            .execute_cypher("MATCH (p:Person) RETURN p.name AS name, p.age AS age ORDER BY name")
            .await
            .expect("match should succeed");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 2, "should return 2 persons");
                let name0 = &q.rows[0].columns[0].1;
                match name0 {
                    GraphValue::Scalar(serde_json::Value::String(s)) => assert_eq!(s, "Alice"),
                    other => panic!("expected String Scalar, got {other:?}"),
                }
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    #[tokio::test]
    async fn test_ladybug_execute_invalid_cypher_returns_error() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        let result = conn.execute_cypher("INVALID CYPHER").await;
        assert!(result.is_err(), "invalid cypher should return error");
    }

    // ===== health_check 测试 =====

    #[tokio::test]
    async fn test_ladybug_health_check_passes() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        conn.health_check().await.expect("health check should pass");
    }

    // ===== backend_name 测试 =====

    #[test]
    fn test_ladybug_backend_name() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        assert_eq!(conn.backend_name(), "ladybug");
    }

    // ===== 事务测试 =====

    #[tokio::test]
    async fn test_ladybug_txn_commit() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        conn.execute_cypher("CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))")
            .await
            .expect("create table");

        let txn = conn.begin_graph_txn().await.expect("begin txn should succeed");
        txn.execute_cypher("CREATE (:Person {name: 'Alice'})")
            .await
            .expect("create in txn");
        txn.commit().await.expect("commit should succeed");

        // 验证事务提交后数据可见
        let result = conn
            .execute_cypher("MATCH (p:Person) RETURN p.name AS name")
            .await
            .expect("match after commit");
        match result {
            GraphExecResult::Query(q) => assert_eq!(q.rows.len(), 1, "should see 1 person after commit"),
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    #[tokio::test]
    async fn test_ladybug_txn_rollback() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        conn.execute_cypher("CREATE NODE TABLE Person(name STRING, PRIMARY KEY(name))")
            .await
            .expect("create table");

        let txn = conn.begin_graph_txn().await.expect("begin txn should succeed");
        txn.execute_cypher("CREATE (:Person {name: 'Alice'})")
            .await
            .expect("create in txn");
        txn.rollback().await.expect("rollback should succeed");

        // 验证事务回滚后数据不可见
        let result = conn
            .execute_cypher("MATCH (p:Person) RETURN p.name AS name")
            .await
            .expect("match after rollback");
        match result {
            GraphExecResult::Query(q) => assert_eq!(q.rows.len(), 0, "should see 0 persons after rollback"),
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
    }

    #[tokio::test]
    async fn test_ladybug_txn_execute_cypher_returns_result() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        conn.execute_cypher("CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name))")
            .await
            .expect("create table");

        let txn = conn.begin_graph_txn().await.expect("begin txn");
        txn.execute_cypher("CREATE (:Person {name: 'Alice', age: 25})")
            .await
            .expect("create alice");

        let result = txn
            .execute_cypher("MATCH (p:Person) RETURN p.name AS name, p.age AS age")
            .await
            .expect("match in txn");
        match result {
            GraphExecResult::Query(q) => {
                assert_eq!(q.rows.len(), 1, "should see 1 person in txn");
            }
            GraphExecResult::Write { .. } => panic!("expected Query variant"),
        }
        txn.commit().await.expect("commit");
    }

    // ===== 值映射测试 =====

    #[test]
    fn test_map_lbug_value_null() {
        let val = lbug::Value::Null(lbug::LogicalType::String);
        let mapped = map_lbug_value(&val);
        assert_eq!(mapped, GraphValue::Scalar(serde_json::Value::Null));
    }

    #[test]
    fn test_map_lbug_value_bool() {
        let val = lbug::Value::Bool(true);
        let mapped = map_lbug_value(&val);
        assert_eq!(mapped, GraphValue::Scalar(serde_json::json!(true)));
    }

    #[test]
    fn test_map_lbug_value_int64() {
        let val = lbug::Value::Int64(42);
        let mapped = map_lbug_value(&val);
        assert_eq!(mapped, GraphValue::Scalar(serde_json::json!(42)));
    }

    #[test]
    fn test_map_lbug_value_string() {
        let val = lbug::Value::String("hello".to_string());
        let mapped = map_lbug_value(&val);
        assert_eq!(mapped, GraphValue::Scalar(serde_json::json!("hello")));
    }

    #[test]
    fn test_map_lbug_value_internal_id() {
        let val = lbug::Value::InternalID(lbug::InternalID { offset: 5, table_id: 2 });
        let mapped = map_lbug_value(&val);
        assert_eq!(mapped, GraphValue::Scalar(serde_json::json!("2:5")));
    }

    #[test]
    fn test_map_lbug_value_list() {
        let val = lbug::Value::List(
            lbug::LogicalType::Int64,
            vec![lbug::Value::Int64(1), lbug::Value::Int64(2)],
        );
        let mapped = map_lbug_value(&val);
        match mapped {
            GraphValue::Scalar(serde_json::Value::Array(arr)) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0], serde_json::json!(1));
                assert_eq!(arr[1], serde_json::json!(2));
            }
            other => panic!("expected Array scalar, got {other:?}"),
        }
    }
}
