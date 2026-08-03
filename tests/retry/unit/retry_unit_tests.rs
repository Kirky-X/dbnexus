// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 重试模块单元测试

use dbnexus::{RetryExecutor, RetryPolicy};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

// ============================================================================
// RetryPolicy 默认值测试
// ============================================================================

#[test]
fn test_retry_policy_default_values() {
    let policy = RetryPolicy::default();
    assert_eq!(policy.max_retries, 3);
    assert_eq!(policy.initial_backoff_ms, 100);
    assert_eq!(policy.max_backoff_ms, 5000);
    assert!((policy.multiplier - 2.0).abs() < f64::EPSILON);
    assert!(policy.jitter);
}

#[test]
fn test_retry_policy_custom_values() {
    let policy = RetryPolicy {
        max_retries: 5,
        initial_backoff_ms: 200,
        max_backoff_ms: 10000,
        multiplier: 3.0,
        jitter: false,
        overall_timeout_ms: None,
    };
    assert_eq!(policy.max_retries, 5);
    assert_eq!(policy.initial_backoff_ms, 200);
    assert_eq!(policy.max_backoff_ms, 10000);
    assert!((policy.multiplier - 3.0).abs() < f64::EPSILON);
    assert!(!policy.jitter);
}

#[test]
fn test_retry_policy_duration_accessors() {
    let policy = RetryPolicy::default();
    assert_eq!(policy.initial_backoff(), Duration::from_millis(100));
    assert_eq!(policy.max_backoff(), Duration::from_secs(5));
}

// ============================================================================
// 幂等性判断测试
// ============================================================================

#[test]
fn test_is_idempotent_select() {
    assert!(dbnexus::reliability::is_idempotent_operation("SELECT * FROM users"));
    assert!(dbnexus::reliability::is_idempotent_operation(
        "select count(*) from orders"
    ));
    assert!(dbnexus::reliability::is_idempotent_operation("  SELECT 1"));
}

#[test]
fn test_is_idempotent_show() {
    assert!(dbnexus::reliability::is_idempotent_operation("SHOW TABLES"));
    assert!(dbnexus::reliability::is_idempotent_operation("show databases"));
}

#[test]
fn test_is_idempotent_explain() {
    assert!(dbnexus::reliability::is_idempotent_operation(
        "EXPLAIN SELECT * FROM users"
    ));
    assert!(dbnexus::reliability::is_idempotent_operation(
        "explain analyze SELECT 1"
    ));
}

#[test]
fn test_not_idempotent_insert() {
    assert!(!dbnexus::reliability::is_idempotent_operation(
        "INSERT INTO users VALUES (1, 'test')"
    ));
    assert!(!dbnexus::reliability::is_idempotent_operation(
        "insert into logs values (1)"
    ));
}

#[test]
fn test_not_idempotent_update() {
    assert!(!dbnexus::reliability::is_idempotent_operation(
        "UPDATE users SET name = 'test' WHERE id = 1"
    ));
}

#[test]
fn test_not_idempotent_delete() {
    assert!(!dbnexus::reliability::is_idempotent_operation(
        "DELETE FROM users WHERE id = 1"
    ));
}

#[test]
fn test_not_idempotent_ddl() {
    assert!(!dbnexus::reliability::is_idempotent_operation(
        "CREATE TABLE test (id INT)"
    ));
    assert!(!dbnexus::reliability::is_idempotent_operation("DROP TABLE test"));
    assert!(!dbnexus::reliability::is_idempotent_operation(
        "ALTER TABLE test ADD COLUMN name VARCHAR(100)"
    ));
}

#[test]
fn test_not_idempotent_unknown() {
    // 未知操作类型默认返回 false（安全侧）
    assert!(!dbnexus::reliability::is_idempotent_operation(
        "GRANT SELECT ON users TO admin"
    ));
    assert!(!dbnexus::reliability::is_idempotent_operation("COMMIT"));
    assert!(!dbnexus::reliability::is_idempotent_operation(""));
}

// ============================================================================
// RetryExecutor 测试
// ============================================================================

#[tokio::test]
async fn test_execute_with_retry_success_first_try() {
    let policy = RetryPolicy {
        max_retries: 3,
        initial_backoff_ms: 10,
        max_backoff_ms: 100,
        multiplier: 2.0,
        jitter: false,
        overall_timeout_ms: None,
    };

    let result = RetryExecutor::execute_with_retry(&policy, || async { Ok::<i32, _>(42) }, "SELECT 1").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[tokio::test]
async fn test_execute_with_retry_success_after_failures() {
    let policy = RetryPolicy {
        max_retries: 3,
        initial_backoff_ms: 10,
        max_backoff_ms: 100,
        multiplier: 2.0,
        jitter: false,
        overall_timeout_ms: None,
    };

    let attempt = Arc::new(AtomicU32::new(0));
    let attempt_clone = attempt.clone();

    let result = RetryExecutor::execute_with_retry(
        &policy,
        move || {
            let attempt = attempt_clone.clone();
            async move {
                let count = attempt.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    // 前 2 次失败
                    Err(dbnexus::foundation::DbError::Query("mock connection error".to_string()))
                } else {
                    // 第 3 次成功
                    Ok(99)
                }
            }
        },
        "SELECT count(*) FROM users",
    )
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 99);
    assert_eq!(attempt.load(Ordering::SeqCst), 3); // 总共执行了 3 次
}

#[tokio::test]
async fn test_execute_with_retry_exhausted() {
    let policy = RetryPolicy {
        max_retries: 2,
        initial_backoff_ms: 10,
        max_backoff_ms: 100,
        multiplier: 2.0,
        jitter: false,
        overall_timeout_ms: None,
    };

    let attempt = Arc::new(AtomicU32::new(0));
    let attempt_clone = attempt.clone();

    let result = RetryExecutor::execute_with_retry(
        &policy,
        move || {
            let attempt = attempt_clone.clone();
            async move {
                attempt.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(dbnexus::foundation::DbError::Query("persistent error".to_string()))
            }
        },
        "SELECT 1",
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(attempt.load(Ordering::SeqCst), 3); // 1 次初始 + 2 次重试
    assert!(err.to_string().contains("exhausted"));
}

#[tokio::test]
async fn test_non_idempotent_no_retry() {
    let policy = RetryPolicy {
        max_retries: 3,
        initial_backoff_ms: 10,
        max_backoff_ms: 100,
        multiplier: 2.0,
        jitter: false,
        overall_timeout_ms: None,
    };

    let attempt = Arc::new(AtomicU32::new(0));
    let attempt_clone = attempt.clone();

    let result = RetryExecutor::execute_with_retry(
        &policy,
        move || {
            let attempt = attempt_clone.clone();
            async move {
                attempt.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(dbnexus::foundation::DbError::Query("error".to_string()))
            }
        },
        "INSERT INTO users VALUES (1, 'test')",
    )
    .await;

    assert!(result.is_err());
    assert_eq!(attempt.load(Ordering::SeqCst), 1); // 只执行了 1 次，不重试
    assert!(result.unwrap_err().to_string().contains("Non-retryable"));
}

#[tokio::test]
async fn test_retry_policy_zero_retries() {
    let policy = RetryPolicy {
        max_retries: 0,
        initial_backoff_ms: 10,
        max_backoff_ms: 100,
        multiplier: 2.0,
        jitter: false,
        overall_timeout_ms: None,
    };

    let attempt = Arc::new(AtomicU32::new(0));
    let attempt_clone = attempt.clone();

    let result = RetryExecutor::execute_with_retry(
        &policy,
        move || {
            let attempt = attempt_clone.clone();
            async move {
                attempt.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(dbnexus::foundation::DbError::Query("error".to_string()))
            }
        },
        "SELECT 1",
    )
    .await;

    assert!(result.is_err());
    assert_eq!(attempt.load(Ordering::SeqCst), 1); // max_retries=0，只执行 1 次
}
