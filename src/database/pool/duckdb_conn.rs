// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DuckDB 连接包装器
//!
//! 提供 DuckDB 嵌入式数据库的异步连接抽象，通过 `tokio::task::spawn_blocking` 桥接
//! DuckDB 的同步 API 到 Tokio 异步运行时。
//!
//! # 架构
//!
//! DuckDB 是嵌入式分析型数据库，其 Rust API（`duckdb::Connection`）是同步的。
//! 本模块通过 `spawn_blocking` 将阻塞式调用移至专用线程池，并使用 `Semaphore`
//! 限制并发 `spawn_blocking` 数量，防止线程池饱和。
//!
//! # 线程安全
//!
//! `DuckDbConnection` 内部使用 `Arc<Mutex<duckdb::Connection>>` 保护连接，
//! 确保同一时刻只有一个线程访问底层 DuckDB 连接。

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

/// DuckDB 连接包装器
///
/// 通过 `Arc<Mutex<duckdb::Connection>>` 持有底层连接，使用 `Semaphore` 限制
/// `spawn_blocking` 并发数为 `MAX_SPAWN_BLOCKING`（默认 4），防止线程池饱和。
#[derive(Clone)]
pub struct DuckDbConnection {
    /// 底层 DuckDB 连接（Mutex 保护，确保线程安全）
    inner: Arc<Mutex<duckdb::Connection>>,
    /// spawn_blocking 并发限制信号量
    spawn_permit: Arc<Semaphore>,
}

/// 默认 spawn_blocking 并发上限
const MAX_SPAWN_BLOCKING: usize = 4;

impl DuckDbConnection {
    /// 创建新的 DuckDB 连接
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
        let db_path = Self::parse_url(url);
        let conn = duckdb::Connection::open(&db_path)
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB connection failed: {e}"))))?;
        Ok(Self {
            inner: Arc::new(Mutex::new(conn)),
            spawn_permit: Arc::new(Semaphore::new(MAX_SPAWN_BLOCKING)),
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
    /// 通过 `spawn_blocking` 在专用线程池中执行阻塞式 DuckDB 调用，
    /// 使用 `Semaphore` 限制并发数为 [`MAX_SPAWN_BLOCKING`]。
    ///
    /// # 错误
    ///
    /// SQL 执行失败时返回 `DbError::Connection`
    pub async fn execute(&self, sql: &str) -> DbResult<DuckDbExecResult> {
        let conn = self.inner.clone();
        let sql_owned = sql.to_string();
        let permit = self.acquire_permit().await?;

        let handle: JoinHandle<DbResult<DuckDbExecResult>> = tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let rows_affected = conn
                .execute(&sql_owned, [])
                .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB execute failed: {e}"))))?;
            Ok(DuckDbExecResult { rows_affected })
        });

        // permit 必须在 handle.await 之后 drop：
        // 若提前 drop，信号量在 spawn_blocking 任务完成前释放，失去并发限制作用。
        let result = handle
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("spawn_blocking join failed: {e}"))))?;
        drop(permit);
        result
    }

    /// 执行查询，返回结果行集合
    ///
    /// 在 `spawn_blocking` 闭包内完成 `prepare → bind → query → collect` 全流程，
    /// 将 `duckdb::Row` 转换为 `Send` 安全的 [`DuckDbRow`]。
    ///
    /// # 错误
    ///
    /// 查询失败或列名提取失败时返回 `DbError::Connection`
    pub async fn query(&self, sql: &str) -> DbResult<Vec<DuckDbRow>> {
        let conn = self.inner.clone();
        let sql_owned = sql.to_string();
        let permit = self.acquire_permit().await?;

        let handle: JoinHandle<DbResult<Vec<DuckDbRow>>> = tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let mut stmt = conn
                .prepare(&sql_owned)
                .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("DuckDB prepare failed: {e}"))))?;

            // 使用 query_map 在闭包内通过 row.as_ref() 获取列信息，
            // 避免 stmt 的可变借用冲突（query 返回的 Rows 持有 stmt 的借用）
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
            Ok(result)
        });

        // permit 必须在 handle.await 之后 drop：
        // 若提前 drop，信号量在 spawn_blocking 任务完成前释放，失去并发限制作用。
        let result = handle
            .await
            .map_err(|e| DbError::Connection(sea_orm::DbErr::Custom(format!("spawn_blocking join failed: {e}"))))?;
        drop(permit);
        result
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
            .field("max_concurrency", &MAX_SPAWN_BLOCKING)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_duckdb_connection_create_memory() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create memory connection");
        assert_eq!(MAX_SPAWN_BLOCKING, 4);
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
}
