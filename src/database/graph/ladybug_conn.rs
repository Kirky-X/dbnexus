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
//! # 线程安全
//!
//! `lbug::Database` 是 `Send + Sync`（unsafe impl），`lbug::Connection` 是 `Send + Sync`。
//! 通过 `Arc<Database>` 共享数据库实例，`Arc<Semaphore>` 限制并发数。

use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::database::graph::{GraphConnection, GraphExecResult, GraphTransaction};
use crate::foundation::{DbError, DbResult};

/// 默认并发连接数
#[allow(dead_code)]
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
    #[allow(dead_code)]
    db: Arc<lbug::Database>,
    /// 并发限制信号量（等效于连接池大小）
    #[allow(dead_code)]
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
    ///
    /// 返回的 `SemaphorePermit` 在 drop 时自动释放，确保不会泄漏。
    #[allow(dead_code)]
    pub(crate) async fn acquire_permit(&self) -> DbResult<tokio::sync::SemaphorePermit<'_>> {
        self.spawn_permit
            .acquire()
            .await
            .map_err(|_| DbError::Connection(sea_orm::DbErr::Custom("Semaphore closed".to_string())))
    }

    /// 获取数据库引用（供 T027 的 GraphConnection impl 在 spawn_blocking 内创建 Connection）
    #[allow(dead_code)]
    pub(crate) fn database(&self) -> &lbug::Database {
        &self.db
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
// GraphConnection impl（stub — T027 将替换为真实实现）
// ============================================================================

#[async_trait::async_trait]
impl GraphConnection for LadybugConnection {
    async fn execute_cypher(&self, _cypher: &str) -> DbResult<GraphExecResult> {
        Err(DbError::Config(
            "LadybugConnection::execute_cypher not yet implemented (T027 will implement)".to_string(),
        ))
    }

    async fn health_check(&self) -> DbResult<()> {
        Err(DbError::Config(
            "LadybugConnection::health_check not yet implemented (T027 will implement)".to_string(),
        ))
    }

    async fn begin_graph_txn(&self) -> DbResult<Box<dyn GraphTransaction + Send>> {
        Err(DbError::Config(
            "LadybugConnection::begin_graph_txn not yet implemented (T027 will implement)".to_string(),
        ))
    }

    fn backend_name(&self) -> &'static str {
        "ladybug"
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

    // ===== Clone + Debug 测试 =====

    #[test]
    fn test_ladybug_clone_shares_database() {
        let conn1 = LadybugConnection::new(":memory:", 2).expect("Failed to create connection");
        let conn2 = conn1.clone();
        assert_eq!(conn1.pool_size(), conn2.pool_size());
        // 两个 clone 共享同一个 Arc<Database>，但此处无法直接验证 Arc 指针相等（字段私有）
        // 通过 clone 后 backend_name 仍可用间接验证
        assert_eq!(conn1.backend_name(), conn2.backend_name());
    }

    #[test]
    fn test_ladybug_debug_format() {
        let conn = LadybugConnection::new(":memory:", 4).expect("Failed to create connection");
        let debug_str = format!("{:?}", conn);
        assert!(debug_str.contains("LadybugConnection"));
        assert!(debug_str.contains("pool_size: 4"));
    }

    // ===== GraphConnection stub 测试（T027 将替换）=====

    #[tokio::test]
    async fn test_ladybug_stub_execute_cypher_returns_error() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        let result = conn.execute_cypher("RETURN 1").await;
        assert!(result.is_err(), "stub execute_cypher should return error");
    }

    #[tokio::test]
    async fn test_ladybug_stub_health_check_returns_error() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        let result = conn.health_check().await;
        assert!(result.is_err(), "stub health_check should return error");
    }

    #[tokio::test]
    async fn test_ladybug_stub_begin_graph_txn_returns_error() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        let result = conn.begin_graph_txn().await;
        assert!(result.is_err(), "stub begin_graph_txn should return error");
    }

    #[test]
    fn test_ladybug_backend_name() {
        let conn = LadybugConnection::new(":memory:", 1).expect("Failed to create connection");
        assert_eq!(conn.backend_name(), "ladybug");
    }
}
