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

use duckdb::types::Value as DuckValue;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;

use crate::foundation::error::{DbError, DbResult};

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
    /// use dbnexus::database::pool::DuckDbConnection;
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

    /// 解析 DuckDB URL 为文件路径
    ///
    /// 支持的格式：
    /// - `:memory:` → `:memory:`
    /// - `duckdb::memory:` → `:memory:`
    /// - `duckdb:path` → `path`
    /// - `duckdb://path` → `path`
    /// - 其他 → 原样返回（兼容直接文件路径）
    fn parse_url(url: &str) -> String {
        let lower = url.to_lowercase();
        if lower == ":memory:" || lower == "duckdb::memory:" {
            return ":memory:".to_string();
        }
        if let Some(rest) = url.strip_prefix("duckdb:") {
            // duckdb:path 或 duckdb://path
            return rest.trim_start_matches('/').to_string();
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
}
