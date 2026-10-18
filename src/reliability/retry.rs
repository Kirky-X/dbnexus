// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 运行时重试 + 指数退避
//!
//! 仅对幂等查询（SELECT / SHOW / EXPLAIN）自动重试，非幂等操作（INSERT / UPDATE / DELETE / DDL）
//! 直接执行不重试，避免副作用重复。

use std::fmt;
use std::future::Future;
use std::time::Duration;

use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::foundation::DbError;

// ============================================================================
// RetryPolicy — 重试策略配置
// ============================================================================

/// 重试策略配置
///
/// 控制重试行为的核心参数：最大重试次数、退避间隔、增长倍数和随机抖动。
///
/// # 示例
///
/// ```rust
/// use dbnexus::RetryPolicy;
///
/// let policy = RetryPolicy {
///     max_retries: 5,
///     initial_backoff_ms: 200,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// 最大重试次数（不含首次执行），默认 3
    pub max_retries: u32,
    /// 初始退避间隔（毫秒），默认 100
    pub initial_backoff_ms: u64,
    /// 最大退避间隔上限（毫秒），默认 5000
    pub max_backoff_ms: u64,
    /// 退避增长倍数，默认 2.0
    pub multiplier: f64,
    /// 是否添加随机抖动（避免 thundering herd），默认 true
    pub jitter: bool,
    /// 整体 wall-clock 超时（毫秒），`None` 表示无超时限制，默认 `None`
    #[serde(default)]
    pub overall_timeout_ms: Option<u64>,
    /// 非 BUSY 写错误一律不重试；BUSY 写错误仅事务外语句在开关开启时重试，默认 false
    ///
    /// BUSY（`database is locked`）意味着语句未生效，语句级重试无副作用重复；
    /// 其他写错误重试可能重复副作用，保持不重试。事务内 BUSY 可能是 WAL 下的
    /// SQLITE_BUSY_SNAPSHOT（读快照失效），必须回滚事务，故事务内永不语句级重试。
    #[serde(default)]
    pub retry_busy_writes: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 5000,
            multiplier: 2.0,
            jitter: true,
            overall_timeout_ms: None,
            retry_busy_writes: false,
        }
    }
}

impl RetryPolicy {
    /// 获取初始退避间隔 as Duration
    pub fn initial_backoff(&self) -> Duration {
        Duration::from_millis(self.initial_backoff_ms)
    }

    /// 获取最大退避间隔 as Duration
    pub fn max_backoff(&self) -> Duration {
        Duration::from_millis(self.max_backoff_ms)
    }
}

// ============================================================================
// RetryError — 重试错误
// ============================================================================

/// 重试过程中的错误类型
#[derive(Debug)]
pub enum RetryError {
    /// 重试次数耗尽，包含最后一次错误
    Exhausted {
        /// 已尝试次数（含首次执行）
        attempts: u32,
        /// 最后一次执行的错误
        last_error: DbError,
    },
    /// 非幂等操作被拒绝重试
    NonRetryable(DbError),
    /// 整体超时
    Timeout {
        /// 超时时间（毫秒）
        timeout_ms: u64,
        /// 最后一次执行的错误
        last_error: DbError,
    },
}

impl fmt::Display for RetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted {
                attempts,
                last_error,
            } => {
                write!(f, "Retry exhausted after {attempts} attempts: {last_error}")
            }
            Self::NonRetryable(err) => write!(f, "Non-retryable operation: {err}"),
            Self::Timeout {
                timeout_ms,
                last_error,
            } => {
                write!(f, "Retry timed out after {timeout_ms}ms: {last_error}")
            }
        }
    }
}

impl std::error::Error for RetryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Exhausted { last_error, .. } => Some(last_error),
            Self::NonRetryable(err) => Some(err),
            Self::Timeout { last_error, .. } => Some(last_error),
        }
    }
}

impl crate::i18n::error_ext::LocalizedMsg for RetryError {
    fn message_key(&self) -> &'static str {
        match self {
            Self::Exhausted { .. } => "retry-exhausted",
            Self::NonRetryable(_) => "retry-non-retryable",
            Self::Timeout { .. } => "retry-timeout",
        }
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        match self {
            Self::Exhausted {
                attempts,
                last_error,
            } => vec![
                ("attempts", attempts.to_string()),
                ("last_error", last_error.to_string()),
            ],
            Self::NonRetryable(err) => vec![("error", err.to_string())],
            Self::Timeout {
                timeout_ms,
                last_error,
            } => vec![
                ("timeout_ms", timeout_ms.to_string()),
                ("last_error", last_error.to_string()),
            ],
        }
    }
}

impl From<RetryError> for DbError {
    fn from(err: RetryError) -> Self {
        match err {
            RetryError::Exhausted { last_error, .. } => last_error,
            RetryError::NonRetryable(err) => err,
            RetryError::Timeout { last_error, .. } => last_error,
        }
    }
}

// ============================================================================
// 幂等性判断
// ============================================================================

/// 判断 SQL 操作是否为幂等操作（可安全重试）。
///
/// 通过 SQL 前缀快速判断：`SELECT`、`SHOW`、`EXPLAIN` 为幂等操作，
/// 其余（INSERT / UPDATE / DELETE / DDL / DCL / Transaction）均视为非幂等。
///
/// 未知操作类型默认返回 false（安全侧）。
///
/// # 性能
///
/// 零分配实现：直接字节级前缀比较，无字符串分配。
pub fn is_idempotent_operation(sql: &str) -> bool {
    let trimmed = sql.trim_start().as_bytes();
    trimmed.len() >= 6 && trimmed[..6].eq_ignore_ascii_case(b"SELECT")
        || trimmed.len() >= 4 && trimmed[..4].eq_ignore_ascii_case(b"SHOW")
        || trimmed.len() >= 7 && trimmed[..7].eq_ignore_ascii_case(b"EXPLAIN")
}

/// 判断错误链是否包含数据库 busy/锁定错误（SQLITE_BUSY / SQLITE_LOCKED）。
///
/// 沿 `source()` 链收集 Display 文本做大小写不敏感匹配，不依赖具体驱动
/// 类型——经 sea-orm/sqlx 透明包装的 sqlite 锁错误均可命中。
///
/// 信任假设：子串匹配不校验错误来源，若错误文本内嵌了外部可控内容（如
/// 恶意命名的数据库对象出现在错误消息里），非 BUSY 错误可能被误判为
/// BUSY；误判后果限于多一次重试尝试（仅 `retry_busy_writes` 开启时），
/// 单语句粒度重试无副作用重复。成本：仅错误路径执行（默认开关关闭时
/// 永不触达），两次有界分配。
pub fn is_busy_error(err: &DbError) -> bool {
    const NEEDLES: [&str; 5] = [
        "database is locked",
        "database table is locked",
        "database is busy",
        "sqlite_busy",
        "sqlite_locked",
    ];

    let mut text = String::new();
    let mut current: Option<&dyn std::error::Error> = Some(err);
    while let Some(e) = current {
        text.push_str(&e.to_string());
        text.push('\n');
        current = e.source();
    }
    let lower = text.to_lowercase();
    NEEDLES.iter().any(|needle| lower.contains(needle))
}

/// 写路径 BUSY 重试决策（[`RetryExecutor`] 与 Session 内联重试循环共用语义）。
///
/// 仅「开关开启 + 事务外 + BUSY 错误」同时成立才允许语句级重试：BUSY 意味着
/// 语句未生效；事务内 BUSY 可能是 WAL 下的 SQLITE_BUSY_SNAPSHOT（读快照失效），
/// 语句级重试不安全，必须回滚整个事务。
pub(crate) fn retry_write_on_busy(
    policy: &RetryPolicy,
    in_transaction: bool,
    err: &DbError,
) -> bool {
    policy.retry_busy_writes && !in_transaction && is_busy_error(err)
}

// ============================================================================
// RetryExecutor — 重试执行器
// ============================================================================

/// 重试执行器 — 无状态，所有方法为关联函数。
///
/// 包装异步操作并提供自动重试（仅幂等操作）+ 指数退避。
pub struct RetryExecutor;

impl RetryExecutor {
    /// 执行可重试操作（关联函数，非实例方法）。
    ///
    /// 仅当 `sql` 被判定为幂等操作时才自动重试。非幂等操作直接执行一次，
    /// 失败时返回 `RetryError::NonRetryable`；`retry_busy_writes` 开启时，
    /// 事务外 BUSY 写错误（见 `retry_write_on_busy`）除外。
    ///
    /// 调用方契约：本执行器不感知事务，写 BUSY 重试按事务外语义处理——
    /// `operation` 闭包失败返回时不得遗留未回滚的打开事务。
    ///
    /// # 参数
    ///
    /// - `policy`: 重试策略配置
    /// - `operation`: 异步闭包，执行实际操作
    /// - `sql`: SQL 字符串，用于幂等性判断
    ///
    /// # 退避策略
    ///
    /// 第 N 次重试的等待时间 = `min(initial_backoff * multiplier^N, max_backoff)`，
    /// 当 `jitter = true` 时添加 ±25% 的随机抖动。
    pub async fn execute_with_retry<F, Fut, T>(
        policy: &RetryPolicy,
        operation: F,
        sql: &str,
    ) -> Result<T, RetryError>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = Result<T, DbError>> + Send,
        T: Send,
    {
        let idempotent = is_idempotent_operation(sql);
        // 非幂等且未开启 BUSY 写重试：直接执行，不重试
        if !idempotent && !policy.retry_busy_writes {
            return operation().await.map_err(RetryError::NonRetryable);
        }

        let deadline = policy.overall_timeout_ms.map(Duration::from_millis);
        let start = std::time::Instant::now();

        // 首次执行
        let mut last_error = match operation().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                if !idempotent && !retry_write_on_busy(policy, false, &e) {
                    return Err(RetryError::NonRetryable(e));
                }
                e
            }
        };

        for attempt in 0..policy.max_retries {
            if let Some(timeout) = deadline
                && start.elapsed() >= timeout
            {
                return Err(RetryError::Timeout {
                    timeout_ms: timeout.as_millis() as u64,
                    last_error,
                });
            }

            let backoff = Self::calculate_backoff(policy, attempt);
            tokio::time::sleep(backoff).await;

            // 退避睡眠可能已越过 deadline：醒来后必须重查，
            // 否则再执行一次 operation 会使总耗时超出 overall_timeout 最多一个 backoff
            if let Some(timeout) = deadline
                && start.elapsed() >= timeout
            {
                return Err(RetryError::Timeout {
                    timeout_ms: timeout.as_millis() as u64,
                    last_error,
                });
            }

            match operation().await {
                Ok(val) => return Ok(val),
                Err(e) => {
                    if !idempotent && !retry_write_on_busy(policy, false, &e) {
                        return Err(RetryError::NonRetryable(e));
                    }
                    last_error = e;
                }
            }
        }

        Err(RetryError::Exhausted {
            attempts: 1 + policy.max_retries,
            last_error,
        })
    }

    /// 计算第 `attempt` 次重试的退避时间
    fn calculate_backoff(policy: &RetryPolicy, attempt: u32) -> Duration {
        let base_ms = policy.initial_backoff_ms as f64;
        // 防止 `attempt as i32` 溢出：限制在 i32::MAX 以内
        let safe_attempt = attempt.min(i32::MAX as u32);
        let backoff_ms = base_ms * policy.multiplier.powi(safe_attempt as i32);
        let capped_ms = backoff_ms.min(policy.max_backoff_ms as f64);

        if policy.jitter {
            // ±25% 随机抖动
            let jitter_range = capped_ms * 0.25;
            let jitter_offset = (rand::rng().random::<f64>() - 0.5) * 2.0 * jitter_range;
            let final_ms = (capped_ms + jitter_offset).max(1.0);
            Duration::from_millis(final_ms as u64)
        } else {
            Duration::from_millis(capped_ms as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 总体超时必须在退避睡眠醒来后（而非仅 sleep 前）重新检查：
    /// overall_timeout=50ms、初始退避=200ms 时，首次失败后的 sleep 会直接越过
    /// deadline，若不重查就会再执行一次 operation（超时偏差最多一个 backoff）。
    #[tokio::test]
    async fn test_overall_timeout_checked_after_sleep() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff_ms: 200,
            jitter: false,
            overall_timeout_ms: Some(50),
            ..Default::default()
        };
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result = RetryExecutor::execute_with_retry(
            &policy,
            move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<u32, _>(DbError::Query("boom".to_string()))
                }
            },
            "SELECT 1",
        )
        .await;

        match result {
            Err(RetryError::Timeout {
                timeout_ms,
                last_error,
            }) => {
                assert_eq!(timeout_ms, 50);
                assert!(last_error.to_string().contains("boom"));
            }
            other => panic!("expected RetryError::Timeout, got {:?}", other),
        }
        // 仅首次执行 1 次；未修复时 sleep 200ms 后会执行第 2 次
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    /// 回归：未配置总体超时（deadline = None）时，重试循环行为不变
    #[tokio::test]
    async fn test_retry_without_timeout_still_succeeds() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_backoff_ms: 1,
            jitter: false,
            overall_timeout_ms: None,
            ..Default::default()
        };
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result = RetryExecutor::execute_with_retry(
            &policy,
            move || {
                let c = c.clone();
                async move {
                    // 第 1 次失败，第 2 次成功
                    if c.fetch_add(1, Ordering::SeqCst) == 0 {
                        Err(DbError::Query("transient".to_string()))
                    } else {
                        Ok(42u32)
                    }
                }
            },
            "SELECT 1",
        )
        .await;

        assert_eq!(result.expect("should succeed on 2nd attempt"), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    /// RetryError 的 Display / source / i18n 键与参数
    #[test]
    fn test_retry_error_display_source_and_i18n() {
        use crate::i18n::error_ext::LocalizedMsg;

        let last = || DbError::Query("boom".to_string());

        let exhausted = RetryError::Exhausted {
            attempts: 4,
            last_error: last(),
        };
        assert_eq!(
            exhausted.to_string(),
            "Retry exhausted after 4 attempts: Query error: boom"
        );
        assert!(std::error::Error::source(&exhausted).is_some());
        assert_eq!(exhausted.message_key(), "retry-exhausted");
        let args = exhausted.message_args();
        assert!(args.contains(&("attempts", "4".to_string())));
        assert!(
            args.iter()
                .any(|(k, v)| *k == "last_error" && v.contains("boom"))
        );

        let non_retryable = RetryError::NonRetryable(last());
        assert_eq!(
            non_retryable.to_string(),
            "Non-retryable operation: Query error: boom"
        );
        assert!(std::error::Error::source(&non_retryable).is_some());
        assert_eq!(non_retryable.message_key(), "retry-non-retryable");
        assert_eq!(
            non_retryable.message_args(),
            vec![("error", "Query error: boom".to_string())]
        );

        let timeout = RetryError::Timeout {
            timeout_ms: 50,
            last_error: last(),
        };
        assert_eq!(
            timeout.to_string(),
            "Retry timed out after 50ms: Query error: boom"
        );
        assert!(std::error::Error::source(&timeout).is_some());
        assert_eq!(timeout.message_key(), "retry-timeout");
        let args = timeout.message_args();
        assert!(args.contains(&("timeout_ms", "50".to_string())));
        assert!(
            args.iter()
                .any(|(k, v)| *k == "last_error" && v.contains("boom"))
        );
    }

    /// RetryError → DbError 保留底层错误
    #[test]
    fn test_retry_error_into_db_error() {
        let exhausted: DbError = RetryError::Exhausted {
            attempts: 2,
            last_error: DbError::Query("e1".to_string()),
        }
        .into();
        assert!(matches!(exhausted, DbError::Query(_)));

        let non_retryable: DbError =
            RetryError::NonRetryable(DbError::Config("e2".to_string())).into();
        assert!(matches!(non_retryable, DbError::Config(_)));

        let timeout: DbError = RetryError::Timeout {
            timeout_ms: 10,
            last_error: DbError::Transaction("e3".to_string()),
        }
        .into();
        assert!(matches!(timeout, DbError::Transaction(_)));
    }

    /// 幂等性判断：前缀大小写不敏感、trim 前导空白、非查询一律 false
    #[test]
    fn test_is_idempotent_operation() {
        assert!(is_idempotent_operation("SELECT 1"));
        assert!(is_idempotent_operation("  select * from t"));
        assert!(is_idempotent_operation("Show tables"));
        assert!(is_idempotent_operation("\tEXPLAIN SELECT 1"));
        // 截断前缀不成立
        assert!(!is_idempotent_operation("SEL"));
        assert!(!is_idempotent_operation(""));
        assert!(!is_idempotent_operation("INSERT INTO t VALUES (1)"));
        assert!(!is_idempotent_operation("DELETE FROM t"));
        assert!(!is_idempotent_operation("UPDATE t SET a = 1"));
        assert!(!is_idempotent_operation("CREATE TABLE t (id INT)"));
    }

    /// 非幂等操作不重试：失败一次即返回 NonRetryable
    #[tokio::test]
    async fn test_non_idempotent_not_retried() {
        let policy = RetryPolicy::default();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result = RetryExecutor::execute_with_retry(
            &policy,
            move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<u32, _>(DbError::Query("write failed".to_string()))
                }
            },
            "INSERT INTO t VALUES (1)",
        )
        .await;

        assert!(matches!(result, Err(RetryError::NonRetryable(_))));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    /// BUSY/locked 错误各形态命中（大小写不敏感）
    #[test]
    fn test_is_busy_error_matches_lock_variants() {
        let cases = [
            "database is locked",
            "Database Is Locked",
            "database table is locked",
            "database is busy",
            "SQLITE_BUSY: too many writers",
            "SQLITE_LOCKED",
        ];
        for msg in cases {
            assert!(
                is_busy_error(&DbError::Query(msg.to_string())),
                "should match: {msg}"
            );
        }
    }

    /// BUSY 识别沿 source 链下探（DbError::Connection 透明透传 DbErr）
    #[test]
    fn test_is_busy_error_walks_source_chain() {
        let err = DbError::Connection(sea_orm::DbErr::Custom("database is locked".to_string()));
        assert!(is_busy_error(&err));
    }

    /// 非 BUSY 错误不命中
    #[test]
    fn test_is_busy_error_non_busy_not_matched() {
        let cases = [
            "syntax error in \"SELECT\"",
            "permission denied for table t",
            "connection closed",
            "boom",
        ];
        for msg in cases {
            assert!(
                !is_busy_error(&DbError::Query(msg.to_string())),
                "should not match: {msg}"
            );
        }
    }

    /// 开关开启：BUSY 写错误重试至 Exhausted
    #[tokio::test]
    async fn test_busy_write_retried_until_exhausted_when_enabled() {
        let policy = RetryPolicy {
            max_retries: 2,
            initial_backoff_ms: 1,
            jitter: false,
            retry_busy_writes: true,
            ..Default::default()
        };
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result = RetryExecutor::execute_with_retry(
            &policy,
            move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<u32, _>(DbError::Connection(sea_orm::DbErr::Custom(
                        "database is locked".to_string(),
                    )))
                }
            },
            "INSERT INTO t VALUES (1)",
        )
        .await;

        match result {
            Err(RetryError::Exhausted { attempts, .. }) => assert_eq!(attempts, 3),
            other => panic!("expected Exhausted, got {other:?}"),
        }
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    /// 开关开启：非 BUSY 写错误立即 NonRetryable（副作用安全侧）
    #[tokio::test]
    async fn test_non_busy_write_not_retried_when_enabled() {
        let policy = RetryPolicy {
            retry_busy_writes: true,
            ..Default::default()
        };
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result = RetryExecutor::execute_with_retry(
            &policy,
            move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<u32, _>(DbError::Query("syntax error".to_string()))
                }
            },
            "INSERT INTO t VALUES (1)",
        )
        .await;

        assert!(matches!(result, Err(RetryError::NonRetryable(_))));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    /// 开关开启：BUSY 写错误第 2 次成功
    #[tokio::test]
    async fn test_busy_write_succeeds_on_retry_when_enabled() {
        let policy = RetryPolicy {
            initial_backoff_ms: 1,
            retry_busy_writes: true,
            ..Default::default()
        };
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result = RetryExecutor::execute_with_retry(
            &policy,
            move || {
                let c = c.clone();
                async move {
                    if c.fetch_add(1, Ordering::SeqCst) == 0 {
                        Err(DbError::Connection(sea_orm::DbErr::Custom(
                            "database is locked".to_string(),
                        )))
                    } else {
                        Ok(7u32)
                    }
                }
            },
            "UPDATE t SET a = 1",
        )
        .await;

        assert_eq!(result.expect("should succeed on 2nd attempt"), 7);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    /// 开关关闭：BUSY 写错误不重试（与现状一致）
    #[tokio::test]
    async fn test_busy_write_not_retried_when_disabled() {
        let policy = RetryPolicy::default();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result = RetryExecutor::execute_with_retry(
            &policy,
            move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<u32, _>(DbError::Connection(sea_orm::DbErr::Custom(
                        "database is locked".to_string(),
                    )))
                }
            },
            "INSERT INTO t VALUES (1)",
        )
        .await;

        assert!(matches!(result, Err(RetryError::NonRetryable(_))));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    /// retry_busy_writes 缺省 false，旧配置反序列化不受影响
    #[test]
    fn test_retry_busy_writes_defaults_false_and_backcompat() {
        assert!(!RetryPolicy::default().retry_busy_writes);
        // 既有字段均无 serde default（存量行为）：旧配置不含新键即可反序列化
        let policy: RetryPolicy = serde_json::from_str(
            r#"{"max_retries": 5, "initial_backoff_ms": 200,
                "max_backoff_ms": 5000, "multiplier": 2.0, "jitter": true}"#,
        )
        .unwrap();
        assert!(!policy.retry_busy_writes);
    }

    /// 写路径 BUSY 重试决策四象限：开关 × 事务 × BUSY
    #[test]
    fn test_retry_write_on_busy_decision_matrix() {
        let on = RetryPolicy {
            retry_busy_writes: true,
            ..Default::default()
        };
        let off = RetryPolicy::default();
        let busy = || DbError::Connection(sea_orm::DbErr::Custom("database is locked".to_string()));
        let plain = || DbError::Query("boom".to_string());

        assert!(retry_write_on_busy(&on, false, &busy()));
        // 事务内永不语句级重试（SQLITE_BUSY_SNAPSHOT 安全约束）
        assert!(!retry_write_on_busy(&on, true, &busy()));
        // 非 BUSY 写错误不重试
        assert!(!retry_write_on_busy(&on, false, &plain()));
        // 开关关闭维持现状
        assert!(!retry_write_on_busy(&off, false, &busy()));
    }

    /// 重试耗尽：attempts = 1 + max_retries
    #[tokio::test]
    async fn test_retry_exhausted_attempts_count() {
        let policy = RetryPolicy {
            max_retries: 2,
            initial_backoff_ms: 1,
            jitter: false,
            overall_timeout_ms: None,
            ..Default::default()
        };
        let result = RetryExecutor::execute_with_retry(
            &policy,
            || async { Err::<u32, _>(DbError::Query("always".to_string())) },
            "SELECT 1",
        )
        .await;

        match result {
            Err(RetryError::Exhausted { attempts, .. }) => assert_eq!(attempts, 3),
            other => panic!("expected Exhausted, got {:?}", other),
        }
    }

    /// jitter=true 的退避值仍落在 [1, max_backoff] 内
    #[test]
    fn test_calculate_backoff_jitter_bounds() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 400,
            jitter: true,
            overall_timeout_ms: None,
            ..Default::default()
        };
        for attempt in 0..4u32 {
            let d = RetryExecutor::calculate_backoff(&policy, attempt);
            // 抖动在 cap 之后叠加：上界 = max_backoff × 1.25
            let upper = (policy.max_backoff_ms as f64 * 1.25) as u128;
            assert!(
                d.as_millis() >= 1 && d.as_millis() <= upper,
                "attempt {attempt} backoff of {}ms is out of bounds",
                d.as_millis()
            );
        }
    }
}
