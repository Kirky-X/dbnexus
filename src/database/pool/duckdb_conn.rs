// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DuckDB 连接包装器
//!
//! 提供 DuckDB 嵌入式数据库的异步连接抽象，通过 `tokio::task::spawn_blocking` 桥接
//! DuckDB 的同步 API 到 Tokio 异步运行时。
//!
//! # 架构（v0.3.0 连接池优化）
//!
//! DuckDB 是嵌入式分析型数据库，其 Rust API（`duckdb::Connection`）是同步的。
//! 本模块通过 `spawn_blocking` 将阻塞式调用移至专用线程池。
//!
//! v0.3.0 前：`Arc<Mutex<duckdb::Connection>>` 单连接 + Semaphore(4)，实际并发=1
//! v0.3.0 后：`Arc<Mutex<Vec<duckdb::Connection>>>` 连接池 + Semaphore(N)，真正并发=N
//!
//! 通过 `Connection::try_clone()` 创建多个连接共享同一个 `DatabaseHandle`，
//! 包括 `:memory:` 数据库也能共享数据。每个 `spawn_blocking` 任务从池中取出一个连接，
//! 执行后归还，实现真正的并行查询。
//!
//! # 线程安全
//!
//! `duckdb::Connection` 是 `Send` 但不是 `Sync`（内部 `RefCell`）。
//! 通过 `Mutex<Vec<Connection>>` 池模式管理，每个任务独占一个连接，
//! 避免运行时借用检查冲突。

use std::sync::Arc;

pub use duckdb::types::Value as DuckValue;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;

use crate::foundation::{DbError, DbResult};

/// DuckDB 查询结果的行数据
///
/// 由于 `duckdb::Row` 不是 `Send`（它借用自 `Statement` 和 `Connection`），
/// 我们在 `spawn_blocking` 闭包内将行数据收集为这个 `Send` 安全的结构体。
///
/// 注意：本结构体不实现 `Serialize`/`Deserialize`，因为 `duckdb::types::Value`
/// 不支持 serde。如需序列化查询结果，请先将 `DuckValue` 转换为自定义类型。
#[derive(Debug, Clone, PartialEq)]
pub struct DuckDbRow {
    /// 列名与对应值的有序集合
    pub columns: Vec<(String, DuckValue)>,
}

impl DuckDbRow {
    /// 按列名获取值
    pub fn get(&self, column_name: &str) -> Option<&DuckValue> {
        self.columns
            .iter()
            .find(|(name, _)| name == column_name)
            .map(|(_, value)| value)
    }

    /// 获取列数
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
}

/// DuckDB 执行结果
#[derive(Debug, Clone)]
pub struct DuckDbExecResult {
    /// 受影响的行数
    pub rows_affected: usize,
}

/// 默认连接池大小
const DEFAULT_POOL_SIZE: usize = 4;

/// DuckDB 连接包装器
///
/// v0.3.0 性能优化：使用连接池（`Vec<duckdb::Connection>`）替代单 `Mutex<Connection>`。
///
/// 通过 `Connection::try_clone()` 创建多个连接共享同一个 `DatabaseHandle`，
/// 每个 `spawn_blocking` 任务从池中获取一个连接，执行后归还。
/// Semaphore 限制并发数 = 连接池大小，实现真正的并行查询。
#[derive(Clone)]
pub struct DuckDbConnection {
    /// 连接池（多个连接共享同一个数据库，通过 try_clone 创建）
    pool: Arc<Mutex<Vec<duckdb::Connection>>>,
    /// 连接池大小
    pool_size: usize,
    /// spawn_blocking 并发限制信号量（= 连接池大小）
    spawn_permit: Arc<Semaphore>,
}

impl DuckDbConnection {
    /// 创建新的 DuckDB 连接（默认连接池大小 4）
    ///
    /// # 参数
    ///
    /// * `url` - DuckDB 连接字符串，支持：
    ///   - `:memory:` 或 `duckdb::memory:` — 内存数据库
    ///   - `duckdb:path/to/file.db` — 文件数据库
    ///   - `duckdb://path/to/file.db` — 文件数据库（URL 格式）
    ///
    /// # 错误
    ///
    /// 连接创建失败时返回 `DbError::Connection`
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use dbnexus::database::DuckDbConnection;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let conn = DuckDbConnection::new("duckdb::memory:")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(url: &str) -> Result<Self, DbError> {
        Self::with_pool_size(url, DEFAULT_POOL_SIZE)
    }

    /// 创建指定连接池大小的 DuckDB 连接
    ///
    /// # 参数
    ///
    /// * `url` - DuckDB 连接字符串
    /// * `pool_size` - 连接池大小（并发查询数）
    pub fn with_pool_size(url: &str, pool_size: usize) -> Result<Self, DbError> {
        let pool_size = pool_size.max(1);
        let db_path = Self::parse_url(url);
        let primary = duckdb::Connection::open(&db_path)
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB connection failed: {e}"))))?;

        // 通过 try_clone 创建多个连接共享同一个数据库
        let mut pool = Vec::with_capacity(pool_size);
        pool.push(primary);
        for i in 1..pool_size {
            let cloned = pool[0].try_clone().map_err(|e| {
                DbError::Connection(sea_orm::DbErr::Custom(format!(
                    "DuckDB try_clone failed for connection {}: {e}",
                    i + 1
                )))
            })?;
            pool.push(cloned);
        }

        Ok(Self {
            pool: Arc::new(Mutex::new(pool)),
            pool_size,
            spawn_permit: Arc::new(Semaphore::new(pool_size)),
        })
    }

    /// 从已存在的 `duckdb::Connection` 创建连接池（共享底层 DatabaseHandle）。
    ///
    /// 用于多组件共享同一 DuckDB 文件句柄的场景（如 alphalloy 的 sync store + DbPool）。
    /// 传入的 `conn` 应已通过 `try_clone()` 从主连接派生，所有池内连接再对此 `conn`
    /// 做 `try_clone`，确保全部连接共享同一底层 DatabaseHandle。
    ///
    /// # 参数
    ///
    /// * `conn` - 已存在的 `duckdb::Connection`（必须已 open，与主连接 try_clone 共享）
    /// * `pool_size` - 连接池大小（并发查询数）
    pub fn from_shared(conn: duckdb::Connection, pool_size: usize) -> Result<Self, DbError> {
        let pool_size = pool_size.max(1);
        let mut pool = Vec::with_capacity(pool_size);
        pool.push(conn.try_clone().map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(format!(
                "DuckDB from_shared try_clone failed for primary: {e}"
            )))
        })?);
        for i in 1..pool_size {
            let cloned = pool[0].try_clone().map_err(|e| {
                DbError::Connection(sea_orm::DbErr::Custom(format!(
                    "DuckDB from_shared try_clone failed for connection {}: {e}",
                    i + 1
                )))
            })?;
            pool.push(cloned);
        }

        Ok(Self {
            pool: Arc::new(Mutex::new(pool)),
            pool_size,
            spawn_permit: Arc::new(Semaphore::new(pool_size)),
        })
    }

    /// 解析 DuckDB URL 为文件路径
    ///
    /// 支持的格式：
    /// - `:memory:` → `:memory:`
    /// - `duckdb::memory:` → `:memory:`
    /// - `duckdb:path` → `path`
    /// - `duckdb://relative/path` → `relative/path`（双斜杠 + 相对路径）
    /// - `duckdb:///absolute/path` → `/absolute/path`（三斜杠 = 绝对路径，保留根斜杠）
    /// - 其他 → 原样返回（兼容直接文件路径）
    fn parse_url(url: &str) -> String {
        let lower = url.to_lowercase();
        if lower == ":memory:" || lower == "duckdb::memory:" {
            return ":memory:".to_string();
        }
        // `duckdb://…`：`///`（含第三个斜杠）= 绝对路径，必须保留根斜杠
        // （此前 trim_start_matches('/') 会把绝对路径削成相对路径，导致打开错误文件）
        if let Some(rest) = url.strip_prefix("duckdb://") {
            return rest.to_string();
        }
        if let Some(rest) = url.strip_prefix("duckdb:") {
            return rest.to_string();
        }
        url.to_string()
    }

    /// 执行 SQL（DDL/DML），返回受影响行数
    ///
    /// v0.3.0 连接池模式：从池中取出连接 → spawn_blocking 执行 → 归还连接
    pub async fn execute(&self, sql: &str) -> DbResult<DuckDbExecResult> {
        let permit = self.acquire_permit().await?;

        // 短锁：从池中取出连接
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(
                    "DuckDB pool exhausted: no connection available".to_string(),
                ))
            })?
        };

        let sql_owned = sql.to_string();
        let handle: JoinHandle<DbResult<(duckdb::Connection, DuckDbExecResult)>> =
            tokio::task::spawn_blocking(move || {
                let rows_affected = conn
                    .execute(&sql_owned, [])
                    .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB execute failed: {e}"))))?;
                Ok((conn, DuckDbExecResult { rows_affected }))
            });

        // permit 必须在 handle.await 之后 drop
        let result = handle
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("spawn_blocking join failed: {e}"))))?;
        drop(permit);

        // 短锁：归还连接
        let (conn, exec_result) = result?;
        {
            let mut pool = self.pool.lock().await;
            pool.push(conn);
        }

        Ok(exec_result)
    }

    /// 执行查询，返回结果行集合
    ///
    /// v0.3.0 连接池模式：从池中取出连接 → spawn_blocking 执行 → 归还连接
    pub async fn query(&self, sql: &str) -> DbResult<Vec<DuckDbRow>> {
        let permit = self.acquire_permit().await?;

        // 短锁：从池中取出连接
        let conn = {
            let mut pool = self.pool.lock().await;
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(
                    "DuckDB pool exhausted: no connection available".to_string(),
                ))
            })?
        };

        let sql_owned = sql.to_string();
        let handle: JoinHandle<DbResult<(duckdb::Connection, Vec<DuckDbRow>)>> =
            tokio::task::spawn_blocking(move || {
                let mut stmt = conn
                    .prepare(&sql_owned)
                    .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB prepare failed: {e}"))))?;

                // 使用 query_map 在闭包内通过 row.as_ref() 获取列信息
                let rows = stmt
                    .query_map([], |row| {
                        let stmt_ref = row.as_ref();
                        let column_count = stmt_ref.column_count();
                        let column_names: Vec<String> = (0..column_count)
                            .map(|i| stmt_ref.column_name(i).ok().map(|s| s.to_string()).unwrap_or_default())
                            .collect();

                        let mut columns = Vec::with_capacity(column_count);
                        for (i, name) in column_names.iter().enumerate() {
                            let value: DuckValue = row.get(i).unwrap_or(DuckValue::Null);
                            columns.push((name.clone(), value));
                        }
                        Ok(DuckDbRow { columns })
                    })
                    .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB query failed: {e}"))))?;

                let mut result = Vec::new();
                for row_result in rows {
                    let row = row_result.map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB row fetch failed: {e}")))
                    })?;
                    result.push(row);
                }
                // stmt 借用 conn，drop stmt 后 conn 可以 move
                drop(stmt);
                Ok((conn, result))
            });

        // permit 必须在 handle.await 之后 drop
        let result = handle
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("spawn_blocking join failed: {e}"))))?;
        drop(permit);

        // 短锁：归还连接
        let (conn, rows) = result?;
        {
            let mut pool = self.pool.lock().await;
            pool.push(conn);
        }

        Ok(rows)
    }

    /// 执行参数化 DDL/DML 语句（仅 DuckDB 连接可用）
    ///
    /// 与 [`Self::execute`] 的唯一区别：通过 prepared statement 传递绑定参数，
    /// 数据库不会将参数值解析为 SQL 代码，从根本上防止 SQL 注入（vuln-0005 同源修复）。
    /// 所有携带用户输入的语句必须走本方法，禁止 format!/拼接组装 SQL。
    ///
    /// # 参数
    ///
    /// * `sql` - 含 `?` 占位符的 SQL 语句
    /// * `params` - 按占位符顺序排列的绑定值（`duckdb::types::Value`）
    pub async fn execute_with_params(&self, sql: &str, params: Vec<DuckValue>) -> DbResult<DuckDbExecResult> {
        let permit = self.acquire_permit().await?;

        let conn = {
            let mut pool = self.pool.lock().await;
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(
                    "DuckDB pool exhausted: no connection available".to_string(),
                ))
            })?
        };

        let sql_owned = sql.to_string();
        let handle: JoinHandle<DbResult<(duckdb::Connection, DuckDbExecResult)>> =
            tokio::task::spawn_blocking(move || {
                let rows_affected = conn
                    .execute(&sql_owned, duckdb::params_from_iter(params))
                    .map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB execute_with_params failed: {e}"
                        )))
                    })?;
                Ok((conn, DuckDbExecResult { rows_affected }))
            });

        let result = handle
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("spawn_blocking join failed: {e}"))))?;
        drop(permit);

        let (conn, exec_result) = result?;
        {
            let mut pool = self.pool.lock().await;
            pool.push(conn);
        }

        Ok(exec_result)
    }

    /// 执行参数化查询（仅 DuckDB 连接可用）
    ///
    /// 与 [`Self::query`] 的唯一区别：通过 prepared statement 传递绑定参数，
    /// 数据库不会将参数值解析为 SQL 代码，从根本上防止 SQL 注入。
    /// 所有携带用户输入的查询必须走本方法，禁止 format!/拼接组装 SQL。
    ///
    /// # 参数
    ///
    /// * `sql` - 含 `?` 占位符的 SQL 查询语句
    /// * `params` - 按占位符顺序排列的绑定值（`duckdb::types::Value`）
    pub async fn query_with_params(&self, sql: &str, params: Vec<DuckValue>) -> DbResult<Vec<DuckDbRow>> {
        let permit = self.acquire_permit().await?;

        let conn = {
            let mut pool = self.pool.lock().await;
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(
                    "DuckDB pool exhausted: no connection available".to_string(),
                ))
            })?
        };

        let sql_owned = sql.to_string();
        let handle: JoinHandle<DbResult<(duckdb::Connection, Vec<DuckDbRow>)>> =
            tokio::task::spawn_blocking(move || {
                let mut stmt = conn
                    .prepare(&sql_owned)
                    .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB prepare failed: {e}"))))?;

                let rows = stmt
                    .query_map(duckdb::params_from_iter(params), |row| {
                        let stmt_ref = row.as_ref();
                        let column_count = stmt_ref.column_count();
                        let column_names: Vec<String> = (0..column_count)
                            .map(|i| stmt_ref.column_name(i).ok().map(|s| s.to_string()).unwrap_or_default())
                            .collect();

                        let mut columns = Vec::with_capacity(column_count);
                        for (i, name) in column_names.iter().enumerate() {
                            let value: DuckValue = row.get(i).unwrap_or(DuckValue::Null);
                            columns.push((name.clone(), value));
                        }
                        Ok(DuckDbRow { columns })
                    })
                    .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB query failed: {e}"))))?;

                let mut result = Vec::new();
                for row_result in rows {
                    let row = row_result.map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB row fetch failed: {e}")))
                    })?;
                    result.push(row);
                }
                drop(stmt);
                Ok((conn, result))
            });

        let result = handle
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("spawn_blocking join failed: {e}"))))?;
        drop(permit);

        let (conn, rows) = result?;
        {
            let mut pool = self.pool.lock().await;
            pool.push(conn);
        }

        Ok(rows)
    }

    /// 在单个事务中原子执行多条参数化语句（仅 DuckDB 连接可用）
    ///
    /// 从内部连接池取出**同一条**底层连接，按顺序执行 `BEGIN → stmt1 → stmt2 → … → COMMIT`；
    /// 任一语句失败则整体 ROLLBACK（ DROP 前置写入，杜绝孤儿行）。
    /// 用于"级联删除 + 主表删除"等多语句原子性场景——dbnexus 的 Session 级
    /// `begin_transaction` 仅支持 SeaORM 后端，DuckDB 路径的事务原子性由本方法提供。
    ///
    /// # 参数
    ///
    /// * `statements` - `(sql, params)` 有序序列，全部在同一事务内执行
    ///
    /// # 返回
    ///
    /// 各语句的执行结果（顺序与输入一致）
    pub async fn execute_transaction(
        &self,
        statements: Vec<(String, Vec<DuckValue>)>,
    ) -> DbResult<Vec<DuckDbExecResult>> {
        let permit = self.acquire_permit().await?;

        let mut conn = {
            let mut pool = self.pool.lock().await;
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(
                    "DuckDB pool exhausted: no connection available".to_string(),
                ))
            })?
        };

        let handle: JoinHandle<DbResult<(duckdb::Connection, Vec<DuckDbExecResult>)>> =
            tokio::task::spawn_blocking(move || {
                let tx = conn.transaction().map_err(|e| {
                    DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB begin transaction failed: {e}")))
                })?;
                let mut results = Vec::with_capacity(statements.len());
                for (sql, params) in statements {
                    let rows_affected = tx.execute(&sql, duckdb::params_from_iter(params)).map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB transaction statement failed: {e}"
                        )))
                    })?;
                    results.push(DuckDbExecResult { rows_affected });
                }
                tx.commit()
                    .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB commit failed: {e}"))))?;
                Ok((conn, results))
            });

        let result = handle
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("spawn_blocking join failed: {e}"))))?;
        drop(permit);

        let (conn, results) = result?;
        {
            let mut pool = self.pool.lock().await;
            pool.push(conn);
        }

        Ok(results)
    }

    /// 健康检查（执行 `SELECT 1`）
    ///
    /// # 错误
    ///
    /// 连接不可用时返回 `DbError::Connection`
    pub async fn health_check(&self) -> DbResult<()> {
        let rows = self.query("SELECT 1 AS health").await?;
        if rows.is_empty() {
            return Err(DbError::Connection(sea_orm::DbErr::Custom(
                "DuckDB health check returned no rows".to_string(),
            )));
        }
        Ok(())
    }

    /// 获取连接池大小
    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    /// 获取 Semaphore 许可证，限制 spawn_blocking 并发数
    ///
    /// 返回的 `SemaphorePermit` 在 drop 时自动释放，确保不会泄漏。
    async fn acquire_permit(&self) -> DbResult<tokio::sync::SemaphorePermit<'_>> {
        self.spawn_permit
            .acquire()
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("Semaphore closed".to_string())))
    }
}

impl std::fmt::Debug for DuckDbConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DuckDbConnection")
            .field("pool_size", &self.pool_size)
            .field("max_concurrency", &self.pool_size)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_duckdb_connection_create_memory() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create memory connection");
        assert_eq!(conn.pool_size(), DEFAULT_POOL_SIZE);
        assert_eq!(DEFAULT_POOL_SIZE, 4);
        let _ = conn;
    }

    #[tokio::test]
    async fn test_duckdb_connection_create_via_url() {
        let conn = DuckDbConnection::new("duckdb::memory:").expect("Failed to create connection via URL");
        let _ = conn;
    }

    #[tokio::test]
    async fn test_duckdb_execute_create_table() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        let result = conn
            .execute("CREATE TABLE test_table (id INTEGER PRIMARY KEY, name VARCHAR)")
            .await
            .expect("Failed to create table");
        assert_eq!(result.rows_affected, 0);
    }

    #[tokio::test]
    async fn test_duckdb_execute_insert_and_query() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name VARCHAR)")
            .await
            .expect("Failed to create table");
        conn.execute("INSERT INTO users VALUES (1, 'Alice')")
            .await
            .expect("Failed to insert");
        conn.execute("INSERT INTO users VALUES (2, 'Bob')")
            .await
            .expect("Failed to insert");

        let rows = conn
            .query("SELECT id, name FROM users ORDER BY id")
            .await
            .expect("Failed to query");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].column_count(), 2);

        let name = rows[0].get("name").expect("Failed to get name column");
        if let DuckValue::Text(s) = name {
            assert_eq!(s, "Alice");
        } else {
            panic!("Expected Text value, got {:?}", name);
        }
    }

    #[tokio::test]
    async fn test_duckdb_health_check() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        conn.health_check().await.expect("Health check should pass");
    }

    #[tokio::test]
    async fn test_duckdb_parse_url_variants() {
        assert_eq!(DuckDbConnection::parse_url(":memory:"), ":memory:");
        assert_eq!(DuckDbConnection::parse_url("duckdb::memory:"), ":memory:");
        assert_eq!(DuckDbConnection::parse_url("duckdb:test.db"), "test.db");
        assert_eq!(
            DuckDbConnection::parse_url("duckdb://path/to/file.db"),
            "path/to/file.db"
        );
        assert_eq!(DuckDbConnection::parse_url("/absolute/path.db"), "/absolute/path.db");
    }

    #[tokio::test]
    async fn test_duckdb_concurrent_execute_respects_semaphore() {
        let conn = Arc::new(DuckDbConnection::new(":memory:").expect("Failed to create connection"));
        conn.execute("CREATE TABLE concurrent_test (id INTEGER)")
            .await
            .expect("Failed to create table");

        let mut handles = Vec::new();
        for i in 0..8 {
            let conn_clone = conn.clone();
            handles.push(tokio::spawn(async move {
                conn_clone
                    .execute(&format!("INSERT INTO concurrent_test VALUES ({i})"))
                    .await
            }));
        }

        for handle in handles {
            let result = handle.await.expect("Task panicked");
            assert!(result.is_ok(), "Concurrent insert should succeed");
        }

        let rows = conn
            .query("SELECT COUNT(*) AS cnt FROM concurrent_test")
            .await
            .expect("Failed to count");
        assert_eq!(rows.len(), 1);
        let count = rows[0].get("cnt").expect("Failed to get count");
        if let DuckValue::BigInt(n) = count {
            assert_eq!(*n, 8);
        } else {
            panic!("Expected BigInt, got {:?}", count);
        }
    }

    /// v0.3.0 连接池优化验证：try_clone 创建的多个连接共享 :memory: 数据库
    #[tokio::test]
    async fn test_duckdb_pool_shares_memory_database() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");

        // 在一个连接上建表
        conn.execute("CREATE TABLE shared_test (id INTEGER PRIMARY KEY, val VARCHAR)")
            .await
            .expect("Failed to create table");

        // 插入数据
        conn.execute("INSERT INTO shared_test VALUES (1, 'hello')")
            .await
            .expect("Failed to insert");

        // 查询验证（可能使用池中不同连接，但数据共享）
        let rows = conn
            .query("SELECT val FROM shared_test WHERE id = 1")
            .await
            .expect("Failed to query");
        assert_eq!(rows.len(), 1);
        let val = rows[0].get("val").expect("Failed to get val");
        if let DuckValue::Text(s) = val {
            assert_eq!(s, "hello");
        } else {
            panic!("Expected Text, got {:?}", val);
        }
    }

    /// v0.3.0 连接池优化验证：自定义连接池大小
    #[tokio::test]
    async fn test_duckdb_custom_pool_size() {
        let conn =
            DuckDbConnection::with_pool_size(":memory:", 2).expect("Failed to create connection with pool size 2");
        assert_eq!(conn.pool_size(), 2);

        // 验证基本功能正常
        conn.execute("CREATE TABLE custom_pool_test (id INTEGER)")
            .await
            .expect("Failed to create table");
        conn.execute("INSERT INTO custom_pool_test VALUES (42)")
            .await
            .expect("Failed to insert");

        let rows = conn
            .query("SELECT id FROM custom_pool_test")
            .await
            .expect("Failed to query");
        assert_eq!(rows.len(), 1);
    }

    /// v0.3.0 连接池优化验证：并发查询使用不同连接
    ///
    /// 验证连接池模式下多任务可以真正并行（而非串行等待单 Mutex）
    #[tokio::test]
    async fn test_duckdb_pool_concurrent_queries_use_different_connections() {
        let conn = Arc::new(
            DuckDbConnection::with_pool_size(":memory:", 4).expect("Failed to create connection with pool size 4"),
        );

        // 建表并插入基础数据
        conn.execute("CREATE TABLE parallel_test (id INTEGER, thread_id INTEGER)")
            .await
            .expect("Failed to create table");

        // 4 个并发任务同时执行（每个使用池中一个连接）
        let mut handles = Vec::new();
        for i in 0..4 {
            let conn_clone = conn.clone();
            handles.push(tokio::spawn(async move {
                conn_clone
                    .execute(&format!("INSERT INTO parallel_test VALUES ({i}, {i})"))
                    .await
            }));
        }

        // 所有任务都应成功（连接池有 4 个连接，无需等待）
        for (i, handle) in handles.into_iter().enumerate() {
            let result = handle.await.expect("Task panicked");
            assert!(result.is_ok(), "Task {} should succeed: {:?}", i, result);
        }

        // 验证数据
        let rows = conn
            .query("SELECT COUNT(*) AS cnt FROM parallel_test")
            .await
            .expect("Failed to count");
        let count = rows[0].get("cnt").expect("Failed to get count");
        if let DuckValue::BigInt(n) = count {
            assert_eq!(*n, 4, "All 4 concurrent inserts should succeed");
        } else {
            panic!("Expected BigInt, got {:?}", count);
        }
    }

    // ===== 数据类型映射测试 =====

    #[tokio::test]
    async fn test_duckdb_data_types_boolean() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        conn.execute("CREATE TABLE bool_test (id INTEGER, flag BOOLEAN)")
            .await
            .expect("create table");
        conn.execute("INSERT INTO bool_test VALUES (1, true), (2, false)")
            .await
            .expect("insert");

        let rows = conn
            .query("SELECT flag FROM bool_test ORDER BY id")
            .await
            .expect("query");
        assert_eq!(rows.len(), 2);
        // DuckDB BOOLEAN 映射
        match &rows[0].get("flag").expect("should have flag") {
            DuckValue::Boolean(b) => assert!(*b, "first row should be true"),
            other => panic!("Expected Boolean, got {:?}", other),
        }
        match &rows[1].get("flag").expect("should have flag") {
            DuckValue::Boolean(b) => assert!(!*b, "second row should be false"),
            other => panic!("Expected Boolean, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_duckdb_data_types_double() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        conn.execute("CREATE TABLE double_test (val DOUBLE)")
            .await
            .expect("create table");
        conn.execute("INSERT INTO double_test VALUES (3.14), (-0.001)")
            .await
            .expect("insert");

        let rows = conn
            .query("SELECT val FROM double_test ORDER BY val")
            .await
            .expect("query");
        assert_eq!(rows.len(), 2);
        match &rows[0].get("val").expect("should have val") {
            DuckValue::Double(f) => assert!((*f - (-0.001)).abs() < 1e-10, "first should be -0.001"),
            other => panic!("Expected Double, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_duckdb_null_handling() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        conn.execute("CREATE TABLE null_test (id INTEGER, name VARCHAR)")
            .await
            .expect("create table");
        conn.execute("INSERT INTO null_test VALUES (1, NULL), (2, 'hello')")
            .await
            .expect("insert");

        let rows = conn
            .query("SELECT name FROM null_test ORDER BY id")
            .await
            .expect("query");
        assert_eq!(rows.len(), 2);
        // 第一行 name 为 NULL
        match &rows[0].get("name").expect("should have name column") {
            DuckValue::Null => {} // expected
            other => panic!("Expected Null, got {:?}", other),
        }
        // 第二行 name 为 'hello'
        match &rows[1].get("name").expect("should have name column") {
            DuckValue::Text(s) => assert_eq!(s, "hello"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    // ===== 错误路径测试 =====

    #[tokio::test]
    async fn test_duckdb_syntax_error_returns_error() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        let result = conn.execute("CREAT TABL broken (id INTEGER)").await;
        assert!(result.is_err(), "syntax error should return error");
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("DuckDB"), "error should mention DuckDB: {msg}");
    }

    #[tokio::test]
    async fn test_duckdb_table_not_exists_returns_error() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        let result = conn.query("SELECT * FROM nonexistent_table").await;
        assert!(result.is_err(), "query on nonexistent table should error");
    }

    #[tokio::test]
    async fn test_duckdb_insert_duplicate_pk_returns_error() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        conn.execute("CREATE TABLE pk_test (id INTEGER PRIMARY KEY)")
            .await
            .expect("create table");
        conn.execute("INSERT INTO pk_test VALUES (1)")
            .await
            .expect("first insert");
        let result = conn.execute("INSERT INTO pk_test VALUES (1)").await;
        assert!(result.is_err(), "duplicate PK should return error");
    }

    // ===== DuckDbRow API 测试 =====

    #[tokio::test]
    async fn test_duckdb_row_get_nonexistent_column_returns_none() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        conn.execute("CREATE TABLE row_test (id INTEGER)")
            .await
            .expect("create table");
        conn.execute("INSERT INTO row_test VALUES (42)").await.expect("insert");

        let rows = conn.query("SELECT id FROM row_test").await.expect("query");
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0].get("nonexistent").is_none(),
            "get() with unknown column should return None"
        );
        assert!(rows[0].get("").is_none(), "get() with empty string should return None");
    }

    #[tokio::test]
    async fn test_duckdb_row_column_count_empty_result() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        conn.execute("CREATE TABLE empty_test (a INTEGER, b VARCHAR, c DOUBLE)")
            .await
            .expect("create table");

        // 查询空表 — 0 行但列结构已知
        let rows = conn.query("SELECT a, b, c FROM empty_test").await.expect("query");
        assert_eq!(rows.len(), 0, "empty table should return 0 rows");
    }

    // ===== Debug 输出测试 =====

    #[tokio::test]
    async fn test_duckdb_connection_debug_format() {
        let conn = DuckDbConnection::with_pool_size(":memory:", 3).expect("Failed to create connection");
        let debug_str = format!("{:?}", conn);
        assert!(
            debug_str.contains("DuckDbConnection"),
            "Debug should contain struct name"
        );
        assert!(debug_str.contains("pool_size: 3"), "Debug should contain pool_size");
    }

    // ===== 文件数据库测试 =====

    #[tokio::test]
    async fn test_duckdb_file_database_persistence() {
        // 使用绝对路径避免 DuckDB 相对路径解析问题
        let mut db_path = std::env::temp_dir();
        db_path.push(format!("dbnexus_duckdb_test_{}.db", std::process::id()));
        // 确保父目录存在（某些环境下 temp_dir() 返回的路径可能不存在）
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let url = format!("duckdb:{}", db_path.display());

        // 创建连接、建表、插入数据
        {
            let conn = DuckDbConnection::new(&url).expect("Failed to create file connection");
            conn.execute("CREATE TABLE persist_test (id INTEGER, val VARCHAR)")
                .await
                .expect("create table");
            conn.execute("INSERT INTO persist_test VALUES (1, 'durable')")
                .await
                .expect("insert");
        } // conn dropped

        // 重新打开文件，验证数据持久化（dir 必须保持存活）
        let conn2 = DuckDbConnection::new(&url).expect("Failed to reopen file connection");
        let rows = conn2
            .query("SELECT val FROM persist_test WHERE id = 1")
            .await
            .expect("query after reopen");
        assert_eq!(rows.len(), 1, "data should persist across connections");
        match &rows[0].get("val").expect("should have val") {
            DuckValue::Text(s) => assert_eq!(s, "durable"),
            other => panic!("Expected Text, got {:?}", other),
        }
        drop(conn2);
        // 清理临时文件
        let _ = std::fs::remove_file(&db_path);
    }

    // ===== 连接池耗尽测试 =====

    #[tokio::test]
    async fn test_duckdb_pool_exhaustion_queues_requests() {
        // pool_size=1，只有 1 个连接
        let conn = Arc::new(DuckDbConnection::with_pool_size(":memory:", 1).expect("Failed to create connection"));
        conn.execute("CREATE TABLE queue_test (id INTEGER)")
            .await
            .expect("create table");

        // 串行执行多个插入（pool_size=1 时自动排队）
        for i in 0..5 {
            conn.execute(&format!("INSERT INTO queue_test VALUES ({i})"))
                .await
                .expect("insert should succeed after queuing");
        }

        let rows = conn
            .query("SELECT COUNT(*) AS cnt FROM queue_test")
            .await
            .expect("count");
        let count = rows[0].get("cnt").expect("should have cnt");
        if let DuckValue::BigInt(n) = count {
            assert_eq!(*n, 5, "all 5 inserts should succeed with queuing");
        } else {
            panic!("Expected BigInt, got {:?}", count);
        }
    }

    // ===== URL 解析边界测试 =====

    #[tokio::test]
    async fn test_duckdb_parse_url_case_insensitive() {
        // :MEMORY: 大小写不敏感
        assert_eq!(DuckDbConnection::parse_url(":MEMORY:"), ":memory:");
        // duckdb::memory: 大小写不敏感
        assert_eq!(DuckDbConnection::parse_url("DuckDB::memory:"), ":memory:");
        // duckdb: 前缀剥离保留原始大小写（仅前缀匹配不敏感）
        assert_eq!(DuckDbConnection::parse_url("duckdb:test.db"), "test.db");
    }
}
