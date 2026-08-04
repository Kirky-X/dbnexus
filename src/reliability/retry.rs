// Copyright (c) 2026 Kirky.X
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
            Self::Exhausted { attempts, last_error } => {
                write!(f, "Retry exhausted after {attempts} attempts: {last_error}")
            }
            Self::NonRetryable(err) => write!(f, "Non-retryable operation: {err}"),
            Self::Timeout { timeout_ms, last_error } => {
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
    /// 失败时返回 `RetryError::NonRetryable`。
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
    pub async fn execute_with_retry<F, Fut, T>(policy: &RetryPolicy, operation: F, sql: &str) -> Result<T, RetryError>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = Result<T, DbError>> + Send,
        T: Send,
    {
        // 非幂等操作：直接执行，不重试
        if !is_idempotent_operation(sql) {
            return operation().await.map_err(RetryError::NonRetryable);
        }

        let deadline = policy.overall_timeout_ms.map(Duration::from_millis);
        let start = std::time::Instant::now();

        // 首次执行
        let mut last_error = match operation().await {
            Ok(val) => return Ok(val),
            Err(e) => e,
        };

        // 重试循环
        for attempt in 0..policy.max_retries {
            // 检查整体超时
            if let Some(timeout) = deadline {
                if start.elapsed() >= timeout {
                    return Err(RetryError::Timeout {
                        timeout_ms: timeout.as_millis() as u64,
                        last_error,
                    });
                }
            }

            let backoff = Self::calculate_backoff(policy, attempt);
            tokio::time::sleep(backoff).await;

            match operation().await {
                Ok(val) => return Ok(val),
                Err(e) => last_error = e,
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
