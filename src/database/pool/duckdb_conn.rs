// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! DuckDB 连接包装器
//!
//! 提供 DuckDB 嵌入式数据库的异步连接抽象，通过 `tokio::task::spawn_blocking` 桥接
//! DuckDB 的同步 API 到 Tokio 异步运行时。
//!
//! # 架构
//!
//! DuckDB 是嵌入式分析型数据库，其 Rust API（`duckdb::Connection`）是同步的。
//! 本模块通过 `spawn_blocking` 将阻塞式调用移至专用线程池。
//!
//! 改进前：`Arc<Mutex<duckdb::Connection>>` 单连接 + Semaphore(4)，实际并发=1
//! 改进后：`Arc<Mutex<Vec<duckdb::Connection>>>` 连接池 + Semaphore(N)，真正并发=N
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
use std::sync::Mutex as SyncMutex;

use crate::i18n;

pub use duckdb::types::Value as DuckValue;
use tokio::sync::{Mutex, MutexGuard, Semaphore};
use tokio::task::JoinHandle;

use crate::foundation::{DbError, DbResult};

/// serde_json 值 → DuckDB 绑定值
///
/// 与 Session 侧 `json_to_sea_value` 同构（跨后端统一的 JSON→绑定值映射口径）：
/// Null → `DuckValue::Null`；整数字段绑定 BigInt（避免 Int32 溢出回退）、
/// 浮点绑定 Double；数组/对象序列化为 JSON 字符串（与 sqlite 路径的
/// 存储形态一致）。供 batch_insert 构建产出的 JSON 参数序列对接
/// `execute_with_params` / `execute_duckdb_transaction` 等参数化通道。
pub fn json_to_duck_value(v: &serde_json::Value) -> DuckValue {
    match v {
        serde_json::Value::Null => DuckValue::Null,
        serde_json::Value::Bool(b) => DuckValue::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                DuckValue::BigInt(i)
            } else {
                DuckValue::Double(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => DuckValue::Text(s.clone()),
        other => {
            DuckValue::Text(serde_json::to_string(other).unwrap_or_else(|_| "null".to_string()))
        }
    }
}

/// spawn_blocking 闭包内的池连接归还 guard
///
/// 连接持有于 guard，闭包正常结束、语句失败、闭包 panic（栈展开）乃至
/// await 侧被取消导致输出被丢弃——任何路径下 guard 都在 blocking 线程内
/// 把连接推回池（池锁为不跨 await 的短锁，同步取用安全）。
/// 此前实现把连接放进闭包输出元组、await 后归还：一旦 await 侧被
/// 超时/abort 取消，输出无人接收，连接随输出 drop 关闭，池容量永久 -1
/// （单连接部署下一次取消即全池耗尽）。
struct PoolConnGuard {
    conn: Option<duckdb::Connection>,
    pool: Arc<SyncMutex<Vec<duckdb::Connection>>>,
}

impl PoolConnGuard {
    /// 借出连接执行语句（连接在 drop 前始终存活）
    fn conn_mut(&mut self) -> &mut duckdb::Connection {
        self.conn
            .as_mut()
            .expect("connection present until PoolConnGuard drop")
    }
}

impl Drop for PoolConnGuard {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.pool
                .lock()
                .expect("DuckDB pool mutex poisoned")
                .push(conn);
        }
    }
}

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
/// 性能优化：使用连接池（`Vec<duckdb::Connection>`）替代单 `Mutex<Connection>`。
///
/// 通过 `Connection::try_clone()` 创建多个连接共享同一个 `DatabaseHandle`，
/// 每个 `spawn_blocking` 任务从池中获取一个连接，执行后归还。
/// Semaphore 限制并发数 = 连接池大小，实现真正的并行查询。
#[derive(Clone)]
pub struct DuckDbConnection {
    /// 连接池（多个连接共享同一个数据库，通过 try_clone 创建）。
    /// 同步短锁：临界区内不跨 await，仅在取/还连接时瞬间持有。
    pool: Arc<SyncMutex<Vec<duckdb::Connection>>>,
    /// 连接池大小
    pool_size: usize,
    /// spawn_blocking 并发限制信号量（= 连接池大小）
    spawn_permit: Arc<Semaphore>,
    /// 串行写闸：`Some` 时全部写路径在执行前互斥持闸（写回串行，读路径
    /// 不受影响）；`None`（默认）维持池化并发写。Clone 句柄经 Arc 共享
    /// 同一闸。
    serialized_write_gate: Option<Arc<Mutex<()>>>,
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
        let primary = duckdb::Connection::open(&db_path).map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(format!(
                "DuckDB connection failed: {e}"
            )))
        })?;

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
            pool: Arc::new(SyncMutex::new(pool)),
            pool_size,
            spawn_permit: Arc::new(Semaphore::new(pool_size)),
            serialized_write_gate: None,
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
            pool: Arc::new(SyncMutex::new(pool)),
            pool_size,
            spawn_permit: Arc::new(Semaphore::new(pool_size)),
            serialized_write_gate: None,
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
    /// 连接池模式：从池中取出连接 → spawn_blocking 执行 → 归还连接。
    /// **成败皆归还**：语句失败不损坏连接对象，先归还再透传错误（否则
    /// 单连接架构下一次失败即令池永久耗尽，见 pool.rs 的 max_connections=1）。
    pub async fn execute(&self, sql: &str) -> DbResult<DuckDbExecResult> {
        let _write_gate = self.acquire_write_gate().await;
        let permit = self.acquire_permit().await?;

        // 短锁：从池中取出连接
        let conn = {
            let mut pool = self.pool.lock().expect("DuckDB pool mutex poisoned");
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(
                    "DuckDB pool exhausted: no connection available".to_string(),
                ))
            })?
        };

        let sql_owned = sql.to_string();
        let pool = self.pool.clone();
        // 连接经 guard 归还：成败/闭包 panic/await 侧被取消（输出被丢弃）皆归还
        let handle: JoinHandle<DbResult<DuckDbExecResult>> =
            tokio::task::spawn_blocking(move || {
                let mut guard = PoolConnGuard {
                    conn: Some(conn),
                    pool,
                };
                (|| {
                    let conn = guard.conn_mut();
                    let rows_affected = conn.execute(&sql_owned, []).map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB execute failed: {e}"
                        )))
                    })?;
                    Ok(DuckDbExecResult { rows_affected })
                })()
            });

        // permit 必须在 handle.await 之后 drop
        let exec_result = handle.await.map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(format!(
                "spawn_blocking join failed: {e}"
            )))
        })?;
        drop(permit);

        exec_result
    }

    /// 执行多语句 SQL 批（DDL/DML 混合，不绑定参数）
    ///
    /// 连接池模式：从池中取出连接 → spawn_blocking 执行 → 归还连接。
    /// **成败皆归还**（同 [`Self::execute`]）：批中任一语句失败不损坏连接对象，
    /// 先归还再透传错误。
    ///
    /// # 行数语义
    ///
    /// `rows_affected` **恒为 0**：DuckDB 原生批量执行接口
    /// （`duckdb::Connection::execute_batch`）不返回受影响行数，本方法不自行
    /// 统计。需要逐语句行数时，应拆分后逐条调用 [`Self::execute`] 或
    /// [`Self::execute_with_params`]。
    ///
    /// # 原子性
    ///
    /// 不自动包裹事务，不保证原子性。需要多语句原子提交/回滚时使用
    /// [`Self::execute_transaction`]。
    pub async fn execute_batch(&self, sql: &str) -> DbResult<DuckDbExecResult> {
        let _write_gate = self.acquire_write_gate().await;
        let permit = self.acquire_permit().await?;

        // 短锁：从池中取出连接
        let conn = {
            let mut pool = self.pool.lock().expect("DuckDB pool mutex poisoned");
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(
                    "DuckDB pool exhausted: no connection available".to_string(),
                ))
            })?
        };

        let sql_owned = sql.to_string();
        let pool = self.pool.clone();
        // 连接经 guard 归还：成败/闭包 panic/await 侧被取消（输出被丢弃）皆归还
        let handle: JoinHandle<DbResult<DuckDbExecResult>> =
            tokio::task::spawn_blocking(move || {
                let mut guard = PoolConnGuard {
                    conn: Some(conn),
                    pool,
                };
                (|| {
                    let conn = guard.conn_mut();
                    conn.execute_batch(&sql_owned).map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                            "duckdb-execute-batch-failed",
                            &[("error", e.to_string())],
                        )))
                    })?;
                    Ok(DuckDbExecResult { rows_affected: 0 })
                })()
            });

        // permit 必须在 handle.await 之后 drop
        let exec_result = handle.await.map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(format!(
                "spawn_blocking join failed: {e}"
            )))
        })?;
        drop(permit);

        exec_result
    }

    /// 执行查询，返回结果行集合
    ///
    /// 连接池模式：从池中取出连接 → spawn_blocking 执行 → 归还连接
    pub async fn query(&self, sql: &str) -> DbResult<Vec<DuckDbRow>> {
        let permit = self.acquire_permit().await?;

        // 短锁：从池中取出连接
        let conn = {
            let mut pool = self.pool.lock().expect("DuckDB pool mutex poisoned");
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(
                    "DuckDB pool exhausted: no connection available".to_string(),
                ))
            })?
        };

        let sql_owned = sql.to_string();
        let pool = self.pool.clone();
        // 连接经 guard 归还：成败/闭包 panic/await 侧被取消（输出被丢弃）皆归还
        let handle: JoinHandle<DbResult<Vec<DuckDbRow>>> = tokio::task::spawn_blocking(move || {
            let mut guard = PoolConnGuard {
                conn: Some(conn),
                pool,
            };
            (|| {
                let conn = guard.conn_mut();
                let mut stmt = conn.prepare(&sql_owned).map_err(|e| {
                    DbError::Connection(sea_orm::DbErr::Custom(format!(
                        "DuckDB prepare failed: {e}"
                    )))
                })?;

                // 使用 query_map 在闭包内通过 row.as_ref() 获取列信息
                let rows = stmt
                    .query_map([], |row| {
                        let stmt_ref = row.as_ref();
                        let column_count = stmt_ref.column_count();
                        let column_names: Vec<String> = (0..column_count)
                            .map(|i| {
                                stmt_ref
                                    .column_name(i)
                                    .ok()
                                    .map(|s| s.to_string())
                                    .unwrap_or_default()
                            })
                            .collect();

                        let mut columns = Vec::with_capacity(column_count);
                        for (i, name) in column_names.iter().enumerate() {
                            let value: DuckValue = row.get(i).unwrap_or(DuckValue::Null);
                            columns.push((name.clone(), value));
                        }
                        Ok(DuckDbRow { columns })
                    })
                    .map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB query failed: {e}"
                        )))
                    })?;

                let mut result = Vec::new();
                for row_result in rows {
                    let row = row_result.map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB row fetch failed: {e}"
                        )))
                    })?;
                    result.push(row);
                }
                drop(stmt);
                Ok(result)
            })()
        });

        // permit 必须在 handle.await 之后 drop
        let rows = handle.await.map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(format!(
                "spawn_blocking join failed: {e}"
            )))
        })?;
        drop(permit);

        rows
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
    pub async fn execute_with_params(
        &self,
        sql: &str,
        params: Vec<DuckValue>,
    ) -> DbResult<DuckDbExecResult> {
        let _write_gate = self.acquire_write_gate().await;
        let permit = self.acquire_permit().await?;

        let conn = {
            let mut pool = self.pool.lock().expect("DuckDB pool mutex poisoned");
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(
                    "DuckDB pool exhausted: no connection available".to_string(),
                ))
            })?
        };

        let sql_owned = sql.to_string();
        let pool = self.pool.clone();
        // 连接经 guard 归还：成败/闭包 panic/await 侧被取消（输出被丢弃）皆归还
        let handle: JoinHandle<DbResult<DuckDbExecResult>> =
            tokio::task::spawn_blocking(move || {
                let mut guard = PoolConnGuard {
                    conn: Some(conn),
                    pool,
                };
                (|| {
                    let conn = guard.conn_mut();
                    let rows_affected = conn
                        .execute(&sql_owned, duckdb::params_from_iter(params))
                        .map_err(|e| {
                            DbError::Connection(sea_orm::DbErr::Custom(format!(
                                "DuckDB execute_with_params failed: {e}"
                            )))
                        })?;
                    Ok(DuckDbExecResult { rows_affected })
                })()
            });

        let exec_result = handle.await.map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(format!(
                "spawn_blocking join failed: {e}"
            )))
        })?;
        drop(permit);

        exec_result
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
    pub async fn query_with_params(
        &self,
        sql: &str,
        params: Vec<DuckValue>,
    ) -> DbResult<Vec<DuckDbRow>> {
        let permit = self.acquire_permit().await?;

        let conn = {
            let mut pool = self.pool.lock().expect("DuckDB pool mutex poisoned");
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(
                    "DuckDB pool exhausted: no connection available".to_string(),
                ))
            })?
        };

        let sql_owned = sql.to_string();
        let pool = self.pool.clone();
        // 连接经 guard 归还：成败/闭包 panic/await 侧被取消（输出被丢弃）皆归还
        let handle: JoinHandle<DbResult<Vec<DuckDbRow>>> = tokio::task::spawn_blocking(move || {
            let mut guard = PoolConnGuard {
                conn: Some(conn),
                pool,
            };
            (|| {
                let conn = guard.conn_mut();
                let mut stmt = conn.prepare(&sql_owned).map_err(|e| {
                    DbError::Connection(sea_orm::DbErr::Custom(format!(
                        "DuckDB prepare failed: {e}"
                    )))
                })?;

                let rows = stmt
                    .query_map(duckdb::params_from_iter(params), |row| {
                        let stmt_ref = row.as_ref();
                        let column_count = stmt_ref.column_count();
                        let column_names: Vec<String> = (0..column_count)
                            .map(|i| {
                                stmt_ref
                                    .column_name(i)
                                    .ok()
                                    .map(|s| s.to_string())
                                    .unwrap_or_default()
                            })
                            .collect();

                        let mut columns = Vec::with_capacity(column_count);
                        for (i, name) in column_names.iter().enumerate() {
                            let value: DuckValue = row.get(i).unwrap_or(DuckValue::Null);
                            columns.push((name.clone(), value));
                        }
                        Ok(DuckDbRow { columns })
                    })
                    .map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB query failed: {e}"
                        )))
                    })?;

                let mut result = Vec::new();
                for row_result in rows {
                    let row = row_result.map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB row fetch failed: {e}"
                        )))
                    })?;
                    result.push(row);
                }
                drop(stmt);
                Ok(result)
            })()
        });

        let rows = handle.await.map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(format!(
                "spawn_blocking join failed: {e}"
            )))
        })?;
        drop(permit);

        rows
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
        let _write_gate = self.acquire_write_gate().await;
        let permit = self.acquire_permit().await?;

        let conn = {
            let mut pool = self.pool.lock().expect("DuckDB pool mutex poisoned");
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(i18n::t_simple(
                    "duckdb-pool-exhausted",
                )))
            })?
        };

        let pool = self.pool.clone();
        // 连接经 guard 归还：成败/事务失败/闭包 panic/await 侧被取消皆归还
        let handle: JoinHandle<DbResult<Vec<DuckDbExecResult>>> =
            tokio::task::spawn_blocking(move || {
                let mut guard = PoolConnGuard {
                    conn: Some(conn),
                    pool,
                };
                // 事务中途失败（含 ROLLBACK）不损坏连接对象
                (|| {
                    let conn = guard.conn_mut();
                    let tx = conn.transaction().map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                            "duckdb-txn-begin-failed",
                            &[("error", e.to_string())],
                        )))
                    })?;
                    let mut results = Vec::with_capacity(statements.len());
                    for (sql, params) in statements {
                        let rows_affected = tx
                            .execute(&sql, duckdb::params_from_iter(params))
                            .map_err(|e| {
                                DbError::Connection(sea_orm::DbErr::Custom(format!(
                                    "DuckDB transaction statement failed: {e}"
                                )))
                            })?;
                        results.push(DuckDbExecResult { rows_affected });
                    }
                    tx.commit().map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                            "duckdb-txn-commit-failed",
                            &[("error", e.to_string())],
                        )))
                    })?;
                    Ok(results)
                })()
            });

        let results = handle.await.map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                "duckdb-spawn-blocking-join-failed",
                &[("error", e.to_string())],
            )))
        })?;
        drop(permit);

        results
    }

    /// 在单个事务中执行泛型闭包，返回其结果（仅 DuckDB 连接可用）
    ///
    /// 连接池模式：从池中取出连接 → spawn_blocking 内 `BEGIN` → 执行闭包 →
    /// 成败皆归还连接。
    ///
    /// # 事务语义
    ///
    /// - 闭包通过 `duckdb::Transaction`（Deref 到 `Connection`）执行语句，
    ///   **事务内可读**：可见本事务未提交的写入（read-your-own-writes）。
    /// - 闭包返回 `Ok` → 提交（`COMMIT`）；commit 失败时错误原样透传，
    ///   残留事务由 `Transaction::drop` 的默认 Rollback 行为清理。
    /// - 闭包返回 `Err`（含事务内语句失败）→ 回滚（`ROLLBACK`），透传
    ///   闭包原始错误；回滚自身失败不掩盖原始错误（drop 再兜底一次）。
    ///
    /// # 参数与返回
    ///
    /// * `f` - 事务闭包，接收 `&duckdb::Transaction`，返回 `DbResult<R>`
    /// * 返回闭包的成功值；`Send + 'static` 约束源于跨 `spawn_blocking` 传值
    pub async fn with_transaction<R, F>(&self, f: F) -> DbResult<R>
    where
        F: for<'a> FnOnce(&'a duckdb::Transaction<'a>) -> DbResult<R> + Send + 'static,
        R: Send + 'static,
    {
        let _write_gate = self.acquire_write_gate().await;
        let permit = self.acquire_permit().await?;

        // 短锁（不跨 await）：从池中取出连接
        let conn = {
            let mut pool = self.pool.lock().expect("DuckDB pool mutex poisoned");
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(i18n::t_simple(
                    "duckdb-pool-exhausted",
                )))
            })?
        };

        let pool = self.pool.clone();
        // 连接经 guard 归还：成败/回滚/闭包 panic/await 侧被取消（含施加
        // timeout/abort 的场景）皆归还，池容量不因取消而永久损失
        let handle: JoinHandle<DbResult<R>> = tokio::task::spawn_blocking(move || {
            let mut guard = PoolConnGuard {
                conn: Some(conn),
                pool,
            };
            (|| {
                let conn = guard.conn_mut();
                let tx = conn.transaction().map_err(|e| {
                    DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                        "duckdb-txn-begin-failed",
                        &[("error", e.to_string())],
                    )))
                })?;
                match f(&tx) {
                    Ok(value) => {
                        tx.commit().map_err(|e| {
                            DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                                "duckdb-txn-commit-failed",
                                &[("error", e.to_string())],
                            )))
                        })?;
                        Ok(value)
                    }
                    Err(e) => {
                        // 回滚吞错：保留闭包原始错误；残留事务由 drop 兜底回滚
                        let _ = tx.rollback();
                        Err(e)
                    }
                }
            })()
        });

        // permit 必须在 handle.await 之后 drop
        let result = handle.await.map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                "duckdb-spawn-blocking-join-failed",
                &[("error", e.to_string())],
            )))
        })?;
        drop(permit);

        result
    }

    /// 执行 COPY FROM 文件语句并返回导入行数（`COPY` 封装的传输原语）
    ///
    /// 执行形如 `COPY "t" ("c1") FROM '/path/data.csv' (FORMAT CSV, ...)`
    /// 的语句（语句构建见 `copy` 模块的 `CopyStatement::build_from_file`），
    /// 从 DuckDB COPY 结果集的首行首列（`Count`）读取导入行数。
    /// 写路径语义：持串行写闸（若启用）+ spawn 许可，成败皆归还连接。
    ///
    /// # 契约
    ///
    /// 仅接受 COPY FROM 语句（表/列标识符在语句构建期经白名单校验，
    /// 路径经单引号转义）；数据文件由调用方创建并清理。
    pub async fn copy_from_file(&self, copy_sql: &str) -> DbResult<u64> {
        let _write_gate = self.acquire_write_gate().await;
        let permit = self.acquire_permit().await?;

        let conn = {
            let mut pool = self.pool.lock().expect("DuckDB pool mutex poisoned");
            pool.pop().ok_or_else(|| {
                DbError::Connection(sea_orm::DbErr::Custom(i18n::t_simple(
                    "duckdb-pool-exhausted",
                )))
            })?
        };

        let sql_owned = copy_sql.to_string();
        let pool = self.pool.clone();
        // 连接经 guard 归还：成败/闭包 panic/await 侧被取消皆归还
        let handle: JoinHandle<DbResult<u64>> = tokio::task::spawn_blocking(move || {
            let mut guard = PoolConnGuard {
                conn: Some(conn),
                pool,
            };
            (|| {
                let conn = guard.conn_mut();
                let mut stmt = conn.prepare(&sql_owned).map_err(|e| {
                    DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                        "duckdb-copy-prepare-failed",
                        &[("error", e.to_string())],
                    )))
                })?;
                let mut rows = stmt.query([]).map_err(|e| {
                    DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                        "duckdb-copy-execute-failed",
                        &[("error", e.to_string())],
                    )))
                })?;
                // COPY 结果集恒为一行一列（BIGINT Count）；空结果按 0 行
                // 导入处理（防御方言差异，不臆测成功）
                match rows.next().map_err(|e| {
                    DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                        "duckdb-copy-fetch-failed",
                        &[("error", e.to_string())],
                    )))
                })? {
                    Some(row) => {
                        let count: i64 = row.get(0).map_err(|e| {
                            DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                                "duckdb-copy-count-read-failed",
                                &[("error", e.to_string())],
                            )))
                        })?;
                        Ok(count.max(0) as u64)
                    }
                    None => Ok(0),
                }
            })()
        });

        let result = handle.await.map_err(|e| {
            DbError::Connection(sea_orm::DbErr::Custom(i18n::t(
                "duckdb-spawn-blocking-join-failed",
                &[("error", e.to_string())],
            )))
        })?;
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

    /// 获取连接池大小
    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    /// 启用串行写闸
    ///
    /// 启用后全部写路径（[`Self::execute`]、[`Self::execute_with_params`]、
    /// [`Self::execute_batch`]、[`Self::execute_transaction`]、
    /// [`Self::with_transaction`]）在执行前互斥持闸，写回串行；读路径
    /// （[`Self::query`]、[`Self::query_with_params`]）不受影响，连接池的
    /// 并发收益保留在读侧。默认不启用（池化并发写）。写入场景出现并发
    /// 冲突（write-write conflict）时压测后再定是否启用。
    ///
    /// # 取消边界
    ///
    /// 串行承诺仅覆盖未被取消的写路径：写调用自身被取消（超时/abort）时，
    /// 闸随 async future 立即释放，而被取消的 blocking 写事务仍会跑完
    /// （其在 blocking 线程内完整提交或回滚，无半开事务），此刻可能与
    /// 下一个进闸的写并发——恰是本闸要防的 write-write conflict 场景。
    /// 对写路径施加超时/取消的调用方不应依赖启用本闸获得互斥保证。
    pub fn with_serialized_writes(mut self) -> Self {
        // 幂等：已启用时复用现有闸，避免 Clone 句柄各自持新闸却共享同一池、
        // 写互斥静默失效
        if self.serialized_write_gate.is_none() {
            self.serialized_write_gate = Some(Arc::new(Mutex::new(())));
        }
        self
    }

    /// 写路径获取串行写闸（未启用时返回 `None`，不产生任何阻塞）
    ///
    /// 返回的 guard 在写路径完成（含连接归还）后 drop，失败路径同样释放。
    async fn acquire_write_gate(&self) -> Option<MutexGuard<'_, ()>> {
        match &self.serialized_write_gate {
            Some(gate) => Some(gate.lock().await),
            None => None,
        }
    }

    /// 获取 Semaphore 许可证，限制 spawn_blocking 并发数
    ///
    /// 返回的 `SemaphorePermit` 在 drop 时自动释放，确保不会泄漏。
    async fn acquire_permit(&self) -> DbResult<tokio::sync::SemaphorePermit<'_>> {
        self.spawn_permit.acquire().await.map_err(|_| {
            DbError::Connection(sea_orm::DbErr::Custom("Semaphore closed".to_string()))
        })
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_duckdb_connection_create_memory() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create memory connection");
        assert_eq!(conn.pool_size(), DEFAULT_POOL_SIZE);
        assert_eq!(DEFAULT_POOL_SIZE, 4);
        let _ = conn;
    }

    /// 回归测试：语句失败后连接必须归还池（成败皆归还）。
    ///
    /// 此前实现经 `DbResult<(Connection, T)>` 在语句失败时把连接随 Err 一起
    /// drop——单连接架构（alphalloy max_connections=1）下一次失败即令池
    /// 永久耗尽、全库 500。本测试在单连接池上先打一条失败语句，再验证
    /// execute / query / execute_with_params / query_with_params /
    /// execute_transaction 五条路径均仍可用。
    #[tokio::test]
    async fn test_connection_returned_after_failed_statement() {
        let conn = DuckDbConnection::with_pool_size(":memory:", 1)
            .expect("Failed to create single-connection pool");

        // 1) execute 失败 → 后续语句仍可用
        assert!(
            conn.execute("THIS IS NOT SQL").await.is_err(),
            "非法 SQL 应失败"
        );
        conn.execute("CREATE TABLE t (id INTEGER)")
            .await
            .expect("execute 失败后连接应已归还池");

        // 2) query 失败 → 后续查询仍可用
        assert!(conn.query("SELECT * FROM no_such_table").await.is_err());
        let rows = conn
            .query("SELECT 1 AS one")
            .await
            .expect("query 失败后连接应已归还池");
        assert_eq!(rows.len(), 1);

        // 3) execute_with_params 失败 → 仍可用
        assert!(
            conn.execute_with_params(
                "INSERT INTO no_such_table VALUES (?)",
                vec![DuckValue::Int(1)]
            )
            .await
            .is_err()
        );
        conn.execute_with_params("INSERT INTO t VALUES (?)", vec![DuckValue::Int(7)])
            .await
            .expect("execute_with_params 失败后连接应已归还池");

        // 4) query_with_params 失败 → 仍可用
        assert!(
            conn.query_with_params(
                "SELECT * FROM no_such_table WHERE id = ?",
                vec![DuckValue::Int(1)]
            )
            .await
            .is_err()
        );
        let rows = conn
            .query_with_params("SELECT id FROM t WHERE id = ?", vec![DuckValue::Int(7)])
            .await
            .expect("query_with_params 失败后连接应已归还池");
        assert_eq!(rows.len(), 1);

        // 5) 事务中途失败（ROLLBACK）→ 连接归还且事务未落库
        let statements = vec![
            (
                "INSERT INTO t VALUES (?)".to_string(),
                vec![DuckValue::Int(8)],
            ),
            ("THIS IS NOT SQL".to_string(), vec![]),
        ];
        assert!(conn.execute_transaction(statements).await.is_err());
        let rows = conn
            .query("SELECT COUNT(*) AS n FROM t")
            .await
            .expect("事务失败后连接应已归还池");
        let n = match &rows[0].columns[0].1 {
            DuckValue::Int(v) => *v,
            DuckValue::BigInt(v) => *v as i32,
            other => panic!("意外列值: {other:?}"),
        };
        // 事务前已有 1 行（id=7）；失败事务中的 id=8 已回滚，不应出现
        assert_eq!(n, 1, "回滚后不应有事务内新行（仅保留事务前的 id=7）");
    }

    #[tokio::test]
    async fn test_duckdb_connection_create_via_url() {
        let conn =
            DuckDbConnection::new("duckdb::memory:").expect("Failed to create connection via URL");
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
        assert_eq!(
            DuckDbConnection::parse_url("/absolute/path.db"),
            "/absolute/path.db"
        );
    }

    #[tokio::test]
    async fn test_duckdb_concurrent_execute_respects_semaphore() {
        let conn =
            Arc::new(DuckDbConnection::new(":memory:").expect("Failed to create connection"));
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

    /// 连接池优化验证：try_clone 创建的多个连接共享 :memory: 数据库
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

    /// 连接池优化验证：自定义连接池大小
    #[tokio::test]
    async fn test_duckdb_custom_pool_size() {
        let conn = DuckDbConnection::with_pool_size(":memory:", 2)
            .expect("Failed to create connection with pool size 2");
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

    /// 连接池优化验证：并发查询使用不同连接
    ///
    /// 验证连接池模式下多任务可以真正并行（而非串行等待单 Mutex）
    #[tokio::test]
    async fn test_duckdb_pool_concurrent_queries_use_different_connections() {
        let conn = Arc::new(
            DuckDbConnection::with_pool_size(":memory:", 4)
                .expect("Failed to create connection with pool size 4"),
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
            DuckValue::Double(f) => {
                assert!((*f - (-0.001)).abs() < 1e-10, "first should be -0.001")
            }
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
        conn.execute("INSERT INTO row_test VALUES (42)")
            .await
            .expect("insert");

        let rows = conn.query("SELECT id FROM row_test").await.expect("query");
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0].get("nonexistent").is_none(),
            "get() with unknown column should return None"
        );
        assert!(
            rows[0].get("").is_none(),
            "get() with empty string should return None"
        );
    }

    #[tokio::test]
    async fn test_duckdb_row_column_count_empty_result() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        conn.execute("CREATE TABLE empty_test (a INTEGER, b VARCHAR, c DOUBLE)")
            .await
            .expect("create table");

        // 查询空表 — 0 行但列结构已知
        let rows = conn
            .query("SELECT a, b, c FROM empty_test")
            .await
            .expect("query");
        assert_eq!(rows.len(), 0, "empty table should return 0 rows");
    }

    // ===== Debug 输出测试 =====

    #[tokio::test]
    async fn test_duckdb_connection_debug_format() {
        let conn =
            DuckDbConnection::with_pool_size(":memory:", 3).expect("Failed to create connection");
        let debug_str = format!("{:?}", conn);
        assert!(
            debug_str.contains("DuckDbConnection"),
            "Debug should contain struct name"
        );
        assert!(
            debug_str.contains("pool_size: 3"),
            "Debug should contain pool_size"
        );
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
        let conn = Arc::new(
            DuckDbConnection::with_pool_size(":memory:", 1).expect("Failed to create connection"),
        );
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

    // ===== execute_batch 测试 =====

    #[tokio::test]
    async fn test_duckdb_execute_batch_multi_statement() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        let result = conn
            .execute_batch(
                "CREATE TABLE batch_test (id INTEGER PRIMARY KEY, val VARCHAR);
                 INSERT INTO batch_test VALUES (1, 'a'), (2, 'b'), (3, 'c');",
            )
            .await
            .expect("batch should succeed");
        // 行数语义锁定：DuckDB 原生批量执行接口不提供受影响行数，恒为 0；
        // 需要逐语句行数时应拆分后走 execute / execute_with_params
        assert_eq!(result.rows_affected, 0);

        // 批内全部语句均已生效
        let rows = conn
            .query("SELECT COUNT(*) AS cnt FROM batch_test")
            .await
            .expect("query after batch");
        let count = rows[0].get("cnt").expect("should have cnt");
        match count {
            DuckValue::BigInt(n) => assert_eq!(*n, 3, "批内两条语句应均已执行"),
            other => panic!("Expected BigInt, got {:?}", other),
        }
    }

    /// 成败皆归还：批中途失败的语句不损坏连接对象，单连接池不被抽干
    #[tokio::test]
    async fn test_connection_returned_after_failed_execute_batch() {
        let conn = DuckDbConnection::with_pool_size(":memory:", 1)
            .expect("Failed to create single-connection pool");

        assert!(
            conn.execute_batch("CREATE TABLE ok_table (id INTEGER); THIS IS NOT SQL")
                .await
                .is_err(),
            "含非法语句的批应失败"
        );
        // 失败批之后连接已归还池：后续 execute_batch / execute 均仍可用
        conn.execute_batch("CREATE TABLE ok_table (id INTEGER); INSERT INTO ok_table VALUES (1)")
            .await
            .expect("失败批后连接应已归还池");
        conn.execute("INSERT INTO ok_table VALUES (2)")
            .await
            .expect("execute 仍可用");
    }

    #[tokio::test]
    async fn test_duckdb_execute_batch_syntax_error_returns_error() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        let result = conn.execute_batch("CREAT TABL broken (id INTEGER)").await;
        assert!(result.is_err(), "语法错误应返回错误");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("execute_batch"),
            "错误信息应标注来源方法: {msg}"
        );
    }

    // ===== with_transaction 测试 =====

    /// 成功路径：闭包返回 Ok → commit → 落库；事务内可读到本事务的写入
    #[tokio::test]
    async fn test_with_transaction_commit_persists_and_readable_in_tx() {
        let conn =
            DuckDbConnection::with_pool_size(":memory:", 1).expect("Failed to create connection");
        conn.execute("CREATE TABLE tx_test (id INTEGER)")
            .await
            .expect("create table");

        let written: i64 = conn
            .with_transaction(|tx| {
                tx.execute("INSERT INTO tx_test VALUES (1)", [])
                    .map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB tx insert failed: {e}"
                        )))
                    })?;
                // 事务内可读：能看到本事务未提交的写入
                let n: i64 = tx
                    .query_row("SELECT COUNT(*) FROM tx_test", [], |r| r.get(0))
                    .map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB tx read failed: {e}"
                        )))
                    })?;
                Ok(n)
            })
            .await
            .expect("事务应提交成功");
        assert_eq!(written, 1, "事务内应读到本事务写入");

        // 提交后对池上后续语句可见
        let rows = conn
            .query("SELECT COUNT(*) AS cnt FROM tx_test")
            .await
            .expect("query after commit");
        match rows[0].get("cnt").expect("should have cnt") {
            DuckValue::BigInt(n) => assert_eq!(*n, 1, "commit 后应落库"),
            other => panic!("Expected BigInt, got {:?}", other),
        }
    }

    /// 语句失败路径：事务内任一语句失败 → 整体回滚 → 单连接池不被抽干
    #[tokio::test]
    async fn test_with_transaction_rolls_back_on_statement_failure() {
        let conn =
            DuckDbConnection::with_pool_size(":memory:", 1).expect("Failed to create connection");
        conn.execute("CREATE TABLE rb_test (id INTEGER)")
            .await
            .expect("create table");
        conn.execute("INSERT INTO rb_test VALUES (1)")
            .await
            .expect("seed row");

        let result: DbResult<()> = conn
            .with_transaction(|tx| {
                tx.execute("INSERT INTO rb_test VALUES (2)", [])
                    .map_err(|e| {
                        DbError::Connection(sea_orm::DbErr::Custom(format!(
                            "DuckDB tx insert failed: {e}"
                        )))
                    })?;
                tx.execute("THIS IS NOT SQL", []).map_err(|e| {
                    DbError::Connection(sea_orm::DbErr::Custom(format!(
                        "DuckDB tx statement failed: {e}"
                    )))
                })?;
                Ok(())
            })
            .await;
        assert!(result.is_err(), "事务内语句失败应返回错误");

        // 回滚生效（事务内 INSERT 不落库）且连接已归还池（后续查询可用）
        let rows = conn
            .query("SELECT COUNT(*) AS cnt FROM rb_test")
            .await
            .expect("语句失败后连接应已归还池");
        match rows[0].get("cnt").expect("should have cnt") {
            DuckValue::BigInt(n) => assert_eq!(*n, 1, "失败事务应整体回滚,仅保留事务前的 1 行"),
            other => panic!("Expected BigInt, got {:?}", other),
        }
    }

    /// commit 失败路径：事务内手动 ROLLBACK 后 commit 必失败（已无活动事务），
    /// with_transaction 应透传错误且连接归还
    #[tokio::test]
    async fn test_with_transaction_commit_failure_returns_error() {
        let conn =
            DuckDbConnection::with_pool_size(":memory:", 1).expect("Failed to create connection");
        conn.execute("CREATE TABLE cf_test (id INTEGER)")
            .await
            .expect("create table");

        let result: DbResult<usize> = conn
            .with_transaction(|tx| {
                // 构造 commit 失败：先手动 ROLLBACK，事务结束后 COMMIT 必报错
                tx.execute_batch("ROLLBACK").map_err(|e| {
                    DbError::Connection(sea_orm::DbErr::Custom(format!(
                        "manual rollback failed: {e}"
                    )))
                })?;
                Ok(7)
            })
            .await;
        assert!(result.is_err(), "commit 失败应返回错误");

        // 连接归还：后续语句仍可用
        conn.execute("INSERT INTO cf_test VALUES (1)")
            .await
            .expect("commit 失败后连接应已归还池");
    }

    // =====================================================================
    // 串行写闸（with_serialized_writes）测试
    // =====================================================================

    /// 默认不启用写闸（维持池化并发写）；启用后 Clone 句柄经 Arc 共享同一闸
    #[tokio::test]
    async fn test_serialized_writes_default_off_and_clone_shares_gate() {
        let conn = DuckDbConnection::new(":memory:").expect("Failed to create connection");
        assert!(conn.serialized_write_gate.is_none(), "默认不应启用串行写闸");

        let enabled = conn.with_serialized_writes();
        let cloned = enabled.clone();
        assert!(enabled.serialized_write_gate.is_some());
        assert!(
            Arc::ptr_eq(
                enabled.serialized_write_gate.as_ref().unwrap(),
                cloned.serialized_write_gate.as_ref().unwrap(),
            ),
            "Clone 句柄应共享同一写闸"
        );

        // 幂等:对已启用句柄(含 Clone 出的)重复启用,必须复用现有闸而非
        // 各自新建——否则两句柄共享池却各持一把闸,写互斥静默失效
        let re_enabled = cloned.with_serialized_writes();
        assert!(
            Arc::ptr_eq(
                enabled.serialized_write_gate.as_ref().unwrap(),
                re_enabled.serialized_write_gate.as_ref().unwrap(),
            ),
            "重复启用应复用同一写闸(幂等)"
        );
    }

    /// 写互斥:默认池化并发下写事务可重叠(max>=2);启用串行写闸后
    /// 同一时刻至多一个写事务在执行(max==1)
    #[tokio::test]
    async fn test_serialized_writes_gate_excludes_concurrent_write_transactions() {
        let peak_inflight = |conn: Arc<DuckDbConnection>| async move {
            let cur = Arc::new(AtomicUsize::new(0));
            let max_inflight = Arc::new(AtomicUsize::new(0));
            let mut handles = Vec::new();
            for _ in 0..4 {
                let conn = conn.clone();
                let cur = cur.clone();
                let max_inflight = max_inflight.clone();
                handles.push(tokio::spawn(async move {
                    conn.with_transaction(move |_tx| {
                        let now = cur.fetch_add(1, Ordering::SeqCst) + 1;
                        max_inflight.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(std::time::Duration::from_millis(30));
                        cur.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    })
                    .await
                    .expect("写事务应成功");
                }));
            }
            for handle in handles {
                handle.await.expect("Task panicked");
            }
            max_inflight.load(Ordering::SeqCst)
        };

        // 默认(未启用闸):池化并发写,事务可重叠
        let pooled = Arc::new(
            DuckDbConnection::with_pool_size(":memory:", 4).expect("Failed to create connection"),
        );
        let pooled_max = peak_inflight(pooled).await;
        assert!(
            pooled_max >= 2,
            "默认池化并发下写事务应可重叠,实际 max={pooled_max}"
        );

        // 启用串行写闸:互斥,峰值并发=1
        let serialized = Arc::new(
            DuckDbConnection::with_pool_size(":memory:", 4)
                .expect("Failed to create connection")
                .with_serialized_writes(),
        );
        let serialized_max = peak_inflight(serialized).await;
        assert_eq!(serialized_max, 1, "串行写闸下写事务应互斥");
    }

    /// 读并发不受限:写事务持闸期间,读路径不排队,在写事务窗口内完成。
    /// 同步点:写事务闭包入口置 entered 标志,主任务轮询该标志确认写已持闸
    /// (替代固定 sleep,消除调度时序假设);写窗口 1s 为慢 CI 留足余量。
    #[tokio::test]
    async fn test_serialized_writes_gate_does_not_block_reads() {
        use std::sync::atomic::AtomicBool;

        let conn = Arc::new(
            DuckDbConnection::with_pool_size(":memory:", 2)
                .expect("Failed to create connection")
                .with_serialized_writes(),
        );
        conn.execute("CREATE TABLE rg (id INTEGER)")
            .await
            .expect("create table");

        let entered = Arc::new(AtomicBool::new(false));
        let writer_entered = entered.clone();
        let writer_conn = conn.clone();
        let writer = tokio::spawn(async move {
            writer_conn
                .with_transaction(move |_tx| {
                    writer_entered.store(true, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                    Ok(())
                })
                .await
                .expect("写事务应成功");
        });

        // 轮询等写事务进入持闸窗口(最多 2s)
        let mut in_window = false;
        for _ in 0..1000 {
            if entered.load(Ordering::SeqCst) {
                in_window = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        assert!(in_window, "写事务应已进入持闸窗口");

        // 读路径在写持闸窗口内完成(若读被写闸阻塞,只能等写结束,下方断言必失败)
        let reader_conn = conn.clone();
        let read = tokio::spawn(async move {
            reader_conn
                .query("SELECT 1 AS one")
                .await
                .expect("读应成功")
        });
        let _rows = read.await.expect("读任务不应 panic");
        assert!(
            !writer.is_finished(),
            "读完成时写事务仍应持闸,证明读未被写闸阻塞"
        );

        writer.await.expect("写任务不应 panic");
    }

    /// 取消安全：with_transaction 被超时取消后，spawn_blocking 任务跑完时
    /// 连接仍经 guard 归还池——单连接池不因一次取消而永久耗尽
    #[tokio::test]
    async fn test_cancelled_with_transaction_returns_connection_to_pool() {
        let conn = Arc::new(DuckDbConnection::with_pool_size(":memory:", 1).expect("create pool"));
        conn.execute("CREATE TABLE cc (id INTEGER)")
            .await
            .expect("create table");

        // 50ms 掐断一个 300ms 的长写事务
        let txn_conn = conn.clone();
        let cancelled = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            txn_conn.with_transaction(move |_tx| {
                std::thread::sleep(std::time::Duration::from_millis(300));
                Ok(())
            }),
        )
        .await;
        assert!(cancelled.is_err(), "长事务应在 50ms 处超时取消");

        // blocking 任务跑完(约 300ms)后连接已归还:单连接池上后续语句可用。
        // query 在连接归还前会因池空立即报错,故轮询等待归还完成。
        let mut recovered = false;
        for _ in 0..200 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            if conn.query("SELECT 1 AS one").await.is_ok() {
                recovered = true;
                break;
            }
        }
        assert!(recovered, "取消后连接应归还池(2s 内恢复可用)");
    }

    /// 机制哨兵：commit(mut self) 按值消耗 Transaction,但其 Err 返回路径上
    /// self 作为函数局部值仍会 drop → Drop::drop → finish_ 兜底回滚——
    /// with_transaction doc 中「commit 失败时残留事务由 Transaction::drop
    /// 的默认 Rollback 行为清理」论据的机制依据。以 DropBehavior::Panic 把
    /// drop 路径变成可观察 panic;若未来 duckdb-rs 改为消耗后不触发 drop,
    /// 本测试先红,上述注释须同步修订。
    #[test]
    fn commit_err_path_runs_drop_fallback() {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        let mut conn = duckdb::Connection::open_in_memory().expect("open in-memory db");
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut tx = conn.transaction().expect("begin transaction");
            tx.set_drop_behavior(duckdb::DropBehavior::Panic);
            tx.execute_batch("ROLLBACK").expect("manual rollback");
            let r = tx.commit();
            assert!(r.is_err(), "手动回滚后 COMMIT 应失败");
            "no-drop"
        }));
        match outcome {
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| payload.downcast_ref::<&'static str>().copied())
                    .unwrap_or_default();
                assert!(
                    msg.contains("Transaction dropped unexpectedly"),
                    "panic 应来自 finish_ 的 Panic 分支,实际: {msg}"
                );
            }
            Ok(_) => panic!("commit Err 路径未触发 Drop 兜底:doc 的 drop 清理论据失效,须同步修订"),
        }
    }
}
