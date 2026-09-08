// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 副本路由 — 基于复制 lag 检测的读写分离
//!
//! 提供 `ReplicationLagDetector` trait 和各后端实现（PostgreSQL / MySQL / SQLite），
//! 以及 `ReplicaPool` 副本连接池，根据复制延迟自动决策路由。

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{ConnectionTrait, Statement};

use crate::database::DbPool;
use crate::foundation::{DbError, DbResult};

// ============================================================================
// ReplicationLag — 复制延迟信息
// ============================================================================

/// 复制延迟检测结果
#[derive(Debug, Clone)]
pub struct ReplicationLag {
    /// 字节级延迟（PostgreSQL 可用）
    pub lag_bytes: Option<u64>,
    /// 秒级延迟（MySQL 可用）
    pub lag_seconds: Option<f64>,
    /// 是否已追上主库（lag 未超过阈值即视为已追上，判定采用 `<=`）
    pub is_caught_up: bool,
}

// ============================================================================
// ReplicationLagDetector — 复制 lag 检测 trait
// ============================================================================

/// 复制 lag 检测 trait
///
/// 各数据库后端需实现此 trait，提供特定的 lag 检测查询。
#[async_trait]
pub trait ReplicationLagDetector: Send + Sync {
    /// 检测当前复制延迟
    async fn detect_lag(&self, pool: &DbPool) -> DbResult<ReplicationLag>;
}

// ============================================================================
// PostgreSQL lag 检测
// ============================================================================

/// PostgreSQL 复制 lag 检测器
///
/// 使用 `pg_wal_lsn_diff` 获取字节级复制延迟。
pub struct PostgresLagDetector {
    /// 最大允许延迟（字节），超过则视为未追上
    pub max_lag_bytes: u64,
}

impl Default for PostgresLagDetector {
    fn default() -> Self {
        Self {
            max_lag_bytes: 10 * 1024 * 1024, // 10MB
        }
    }
}

#[async_trait]
impl ReplicationLagDetector for PostgresLagDetector {
    async fn detect_lag(&self, pool: &DbPool) -> DbResult<ReplicationLag> {
        let session = pool.get_session("admin").await?;
        // Session 的 execute_raw 只返回 ExecResult（行数），无法取回查询值，
        // 故通过 session.connection() 拿到底层 SeaORM 连接执行检测查询。
        let conn = session.connection()?;
        // pg_wal_lsn_diff 结果为 numeric，cast 为 text 以便以字符串取出（跨驱动类型安全）
        let sql = "SELECT pg_wal_lsn_diff(pg_current_wal_lsn(), \
                   COALESCE(pg_last_wal_replay_lsn(), pg_current_wal_lsn()))::text AS lag_bytes";
        let row = conn
            .query_one_raw(Statement::from_string(
                conn.get_database_backend(),
                sql.to_owned(),
            ))
            .await
            .map_err(DbError::Connection)?
            // 正常情况下该查询恒返回单行；无行说明后端异常，交由调用方回退主库
            .ok_or_else(|| DbError::Query("pg_wal_lsn_diff query returned no rows".to_string()))?;
        let raw: String = row.try_get_by("lag_bytes").map_err(DbError::Connection)?;
        let lag_bytes = parse_pg_wal_lag(&raw);
        // 解析失败（lag_bytes = None）视为"无法确认已同步"，保守判定未追上
        let is_caught_up = lag_bytes.is_some_and(|lag| lag <= self.max_lag_bytes);
        Ok(ReplicationLag {
            lag_bytes,
            lag_seconds: None,
            is_caught_up,
        })
    }
}

// ============================================================================
// MySQL lag 检测
// ============================================================================

/// MySQL 复制 lag 检测器
///
/// 使用 `SHOW SLAVE STATUS` 解析 `Seconds_Behind_Master`。
pub struct MySqlLagDetector {
    /// 最大允许延迟（秒）
    pub max_lag_seconds: f64,
}

impl Default for MySqlLagDetector {
    fn default() -> Self {
        Self {
            max_lag_seconds: 5.0,
        }
    }
}

#[async_trait]
impl ReplicationLagDetector for MySqlLagDetector {
    async fn detect_lag(&self, pool: &DbPool) -> DbResult<ReplicationLag> {
        let session = pool.get_session("admin").await?;
        let conn = session.connection()?;
        // 选用 SHOW SLAVE STATUS：兼容 MySQL 5.7/8.0 与 MariaDB。
        // MySQL 8.0.22+ 的新式等价命令为 SHOW REPLICA STATUS（延迟列改名
        // Seconds_Behind_Source），且 8.4+ 移除了旧命令；如需支持 8.4+ 可在此切换，
        // 两种状态行的延迟列解析均由 parse_mysql_seconds_behind 承担。
        let sql = "SHOW SLAVE STATUS";
        let row = conn
            .query_one_raw(Statement::from_string(
                conn.get_database_backend(),
                sql.to_owned(),
            ))
            .await
            .map_err(DbError::Connection)?
            // 空结果说明该节点未配置为主库的副本，无法确认数据一致性，
            // 返回 Err 让 get_read_session 回退主库（绝不假成功）
            .ok_or_else(|| {
                DbError::Query(
                    "SHOW SLAVE STATUS returned no rows: server is not configured as a replica"
                        .to_string(),
                )
            })?;
        // 三态提取（注意：sea-orm 2.0 对 Option<T> 会把"缺列"与 NULL 都压平为
        // Ok(None)，两条运行时路径最终都会落到保守的"未追上"语义，安全等价；
        // Err 分支为列存在但无法按整数解码的异常状态行）
        let raw = match row.try_get_by::<Option<i64>, _>("Seconds_Behind_Master") {
            Ok(Some(v)) => SecondsBehindRaw::Value(v),
            Ok(None) => SecondsBehindRaw::Null,
            Err(_) => SecondsBehindRaw::MissingColumn,
        };
        let lag_seconds = parse_mysql_seconds_behind(raw);
        // 解析失败（lag_seconds = None）视为"无法确认已同步"，保守判定未追上
        let is_caught_up = lag_seconds.is_some_and(|s| s <= self.max_lag_seconds);
        Ok(ReplicationLag {
            lag_bytes: None,
            lag_seconds,
            is_caught_up,
        })
    }
}

// ============================================================================
// lag 值纯函数解析（与具体连接解耦，便于单元测试）
// ============================================================================

/// 解析 PostgreSQL `pg_wal_lsn_diff` 查询结果为字节级延迟
///
/// - 整数字符串（如 "0"、"1048576"）直接解析
/// - 带小数的 numeric 输出（如 "123.7"）截断小数部分
/// - 空串/非数值/负数/NaN 等非法输入返回 `None`（调用方按"未追上"保守处理）
fn parse_pg_wal_lag(row_value: &str) -> Option<u64> {
    let s = row_value.trim();
    // 常规路径：pg_wal_lsn_diff 结果为整数字节差
    if let Ok(v) = s.parse::<u64>() {
        return Some(v);
    }
    // 兜底：numeric 可能输出小数形式，截断为整数字节
    let f = s.parse::<f64>().ok()?;
    if f.is_finite() && f >= 0.0 {
        Some(f as u64)
    } else {
        None
    }
}

/// `SHOW SLAVE STATUS` 结果行中 `Seconds_Behind_Master` 列的原始状态
#[derive(Debug, Clone, PartialEq, Eq)]
enum SecondsBehindRaw {
    /// 列存在且非 NULL（整数秒）
    Value(i64),
    /// 列存在但为 NULL（MySQL 语义：复制中断或 IO 线程未运行）
    Null,
    /// 结果行缺少该列或无法按整数解码（非预期的状态行结构，如新版本命令输出列改名）
    MissingColumn,
}

/// 解析 MySQL `Seconds_Behind_Master` 原始值为秒级延迟
///
/// - `Value(5)`/`Value(0)` → `Some(5.0)`/`Some(0.0)`
/// - `Null`（复制中断）与 `MissingColumn`（缺列）→ `None`，
///   调用方按"未追上"保守处理，绝不路由读请求到状态不明的副本
/// - 负数值非法，返回 `None`
fn parse_mysql_seconds_behind(raw: SecondsBehindRaw) -> Option<f64> {
    match raw {
        SecondsBehindRaw::Value(v) if v >= 0 => Some(v as f64),
        _ => None,
    }
}

// ============================================================================
// SQLite lag 检测（无副本语义）
// ============================================================================

/// SQLite 复制 lag 检测器
///
/// SQLite 无副本语义，始终返回 `is_caught_up = true`。
pub struct SqliteLagDetector;

#[async_trait]
impl ReplicationLagDetector for SqliteLagDetector {
    async fn detect_lag(&self, _pool: &DbPool) -> DbResult<ReplicationLag> {
        Ok(ReplicationLag {
            lag_bytes: None,
            lag_seconds: None,
            is_caught_up: true,
        })
    }
}

// ============================================================================
// ReplicaPool — 副本连接池
// ============================================================================

/// 副本连接池
///
/// 持有副本 DbPool 和 lag 检测器，根据复制延迟决策读请求路由。
/// 当 lag 超过阈值时返回 `None`，调用方应回退到主库。
pub struct ReplicaPool {
    /// 副本连接池
    pool: Arc<DbPool>,
    /// lag 检测器
    lag_detector: Box<dyn ReplicationLagDetector>,
    /// 最大允许延迟（秒）
    ///
    /// 路由判定只依赖检测器折算进 `is_caught_up` 的阈值结果（见 `get_read_session`），
    /// 该字段不再参与判定，仅为保持 `new` 构造签名的 API 兼容而保留。
    #[allow(dead_code)]
    max_lag_seconds: f64,
}

impl ReplicaPool {
    /// 创建副本连接池
    pub fn new(
        pool: Arc<DbPool>,
        lag_detector: Box<dyn ReplicationLagDetector>,
        max_lag_seconds: f64,
    ) -> Self {
        Self {
            pool,
            lag_detector,
            max_lag_seconds,
        }
    }

    /// 获取读 session（lag 感知路由）
    ///
    /// 先检测复制 lag：
    /// - `is_caught_up = true`（lag 未超过阈值）→ 返回副本 session
    /// - 其余一律返回 `None`，调用方回退主库：延迟超过阈值、检测失败（`Err`）
    ///   或结果不明确（解析失败按"未追上"保守判定）均走此分支
    pub async fn get_read_session(&self, role: &str) -> Option<crate::Session> {
        match self.lag_detector.detect_lag(&self.pool).await {
            Ok(lag) if lag.is_caught_up => self.pool.get_session(role).await.ok(),
            // 延迟超过阈值（is_caught_up = false）或检测失败/不确定 → 回退主库，
            // 绝不把读请求路由到状态不明的副本
            _ => None,
        }
    }

    /// 获取底层副本连接池引用
    pub fn pool(&self) -> &Arc<DbPool> {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== parse_pg_wal_lag 测试 =====

    #[test]
    fn test_parse_pg_wal_lag_zero() {
        assert_eq!(parse_pg_wal_lag("0"), Some(0));
    }

    #[test]
    fn test_parse_pg_wal_lag_positive_integer() {
        assert_eq!(parse_pg_wal_lag("1048576"), Some(1048576));
    }

    #[test]
    fn test_parse_pg_wal_lag_trims_whitespace() {
        assert_eq!(parse_pg_wal_lag("  42\n"), Some(42));
    }

    #[test]
    fn test_parse_pg_wal_lag_fractional_truncates() {
        // numeric 带小数输出，截断为整数字节
        assert_eq!(parse_pg_wal_lag("123.7"), Some(123));
    }

    #[test]
    fn test_parse_pg_wal_lag_negative_invalid() {
        assert_eq!(parse_pg_wal_lag("-1"), None);
    }

    #[test]
    fn test_parse_pg_wal_lag_garbage_invalid() {
        assert_eq!(parse_pg_wal_lag("abc"), None);
    }

    #[test]
    fn test_parse_pg_wal_lag_empty_invalid() {
        assert_eq!(parse_pg_wal_lag(""), None);
    }

    #[test]
    fn test_parse_pg_wal_lag_nan_invalid() {
        assert_eq!(parse_pg_wal_lag("NaN"), None);
    }

    // ===== parse_mysql_seconds_behind 测试 =====

    #[test]
    fn test_parse_mysql_seconds_behind_numeric() {
        assert_eq!(
            parse_mysql_seconds_behind(SecondsBehindRaw::Value(5)),
            Some(5.0)
        );
    }

    #[test]
    fn test_parse_mysql_seconds_behind_zero() {
        assert_eq!(
            parse_mysql_seconds_behind(SecondsBehindRaw::Value(0)),
            Some(0.0)
        );
    }

    #[test]
    fn test_parse_mysql_seconds_behind_null_broken_replication() {
        // NULL 表示复制中断，无法确认已同步
        assert_eq!(parse_mysql_seconds_behind(SecondsBehindRaw::Null), None);
    }

    #[test]
    fn test_parse_mysql_seconds_behind_missing_column() {
        assert_eq!(
            parse_mysql_seconds_behind(SecondsBehindRaw::MissingColumn),
            None
        );
    }

    #[test]
    fn test_parse_mysql_seconds_behind_negative_invalid() {
        assert_eq!(
            parse_mysql_seconds_behind(SecondsBehindRaw::Value(-3)),
            None
        );
    }

    // ===== is_caught_up 阈值判定语义测试 =====

    #[test]
    fn test_pg_caught_up_respects_max_lag_bytes() {
        // 与 detect_lag 中的判定逻辑一致：lag <= max_lag_bytes 才算追上
        let detector = PostgresLagDetector::default();
        let lag = parse_pg_wal_lag("1048576").unwrap(); // 1MB < 10MB
        assert!(lag <= detector.max_lag_bytes);
        let huge = parse_pg_wal_lag("20971520").unwrap(); // 20MB > 10MB
        assert!(huge > detector.max_lag_bytes);
        // 解析失败 → 保守判定未追上
        assert!(!parse_pg_wal_lag("bad").is_some_and(|lag| lag <= detector.max_lag_bytes));
    }

    #[test]
    fn test_mysql_caught_up_respects_max_lag_seconds() {
        // 与 detect_lag 中的判定逻辑一致：秒数 <= max_lag_seconds 才算追上
        let detector = MySqlLagDetector::default();
        let ok = parse_mysql_seconds_behind(SecondsBehindRaw::Value(3)).unwrap();
        assert!(ok <= detector.max_lag_seconds);
        let late = parse_mysql_seconds_behind(SecondsBehindRaw::Value(30)).unwrap();
        assert!(late > detector.max_lag_seconds);
        // NULL/缺列 → None → 保守判定未追上（get_read_session 回退主库）
        assert!(
            !parse_mysql_seconds_behind(SecondsBehindRaw::Null)
                .is_some_and(|s| s <= detector.max_lag_seconds)
        );
    }
}
