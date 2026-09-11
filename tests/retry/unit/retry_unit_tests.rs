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
    assert!(dbnexus::reliability::is_idempotent_operation(
        "SELECT * FROM users"
    ));
    assert!(dbnexus::reliability::is_idempotent_operation(
        "select count(*) from orders"
    ));
    assert!(dbnexus::reliability::is_idempotent_operation("  SELECT 1"));
}

#[test]
fn test_is_idempotent_show() {
    assert!(dbnexus::reliability::is_idempotent_operation("SHOW TABLES"));
    assert!(dbnexus::reliability::is_idempotent_operation(
        "show databases"
    ));
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
    assert!(!dbnexus::reliability::is_idempotent_operation(
        "DROP TABLE test"
    ));
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

    let result =
        RetryExecutor::execute_with_retry(&policy, || async { Ok::<i32, _>(42) }, "SELECT 1").await;
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
                    Err(dbnexus::foundation::DbError::Query(
                        "mock connection error".to_string(),
                    ))
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
                Err::<i32, _>(dbnexus::foundation::DbError::Query(
                    "persistent error".to_string(),
                ))
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

// ============================================================================
// T410：Retry 自动接线（Session 执行路径）
// ============================================================================
//
// SELECT/SHOW/EXPLAIN（幂等）类查询经 RetryPolicy 自动重试；写类（INSERT/
// UPDATE/DELETE/DDL）不重试。接线位置：Session::execute_raw（rc3 既有）与
// Session::query_rows（T410 补齐）。通过退避耗尽的时间下界观察重试是否生效
// （jitter=false → 退避确定性）。

#[cfg(all(
    feature = "sqlite",
    feature = "runtime-tokio-rustls",
    feature = "sql-parser"
))]
mod t410_session_retry_wiring_tests {
    use dbnexus::{DbConfig, DbPool, RetryPolicy};
    use std::time::{Duration, Instant};

    fn config_with_policy(url: String, max_retries: u32, backoff_ms: u64) -> DbConfig {
        DbConfig {
            url,
            retry_policy: Some(RetryPolicy {
                max_retries,
                initial_backoff_ms: backoff_ms,
                max_backoff_ms: backoff_ms,
                multiplier: 2.0,
                jitter: false,
                overall_timeout_ms: None,
            }),
            ..Default::default()
        }
    }

    fn temp_db_url(tag: &str) -> (String, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("dbnexus_t410_{}_{}.db", tag, std::process::id()));
        (format!("sqlite:{}?mode=rwc", path.display()), path)
    }

    /// SELECT 查询失败时经退避重试：耗时下界 = max_retries 次退避之和
    #[tokio::test]
    async fn test_t410_query_rows_idempotent_select_is_retried() {
        let (url, path) = temp_db_url("qr_select");
        let config = config_with_policy(url.clone(), 1, 250);
        let pool = DbPool::with_config(config).await.unwrap();

        let start = Instant::now();
        let err = pool
            .query_rows("SELECT * FROM missing_table_t410", "admin")
            .await
            .unwrap_err();
        let elapsed = start.elapsed();
        // max_retries=1 → 首次执行 + 1 次退避(250ms) + 重试 = 至少 250ms
        assert!(
            elapsed >= Duration::from_millis(250),
            "SELECT 应经重试退避（耗时 >= 250ms），实际 {:?}，err={}",
            elapsed,
            err
        );

        // 对照：无重试策略的同一查询应立即失败（< 250ms）
        let (url2, path2) = temp_db_url("qr_select_nop");
        let pool2 = DbPool::with_config(config_with_policy_no_retry(url2.clone())).await.unwrap();
        let start2 = Instant::now();
        let _ = pool2
            .query_rows("SELECT * FROM missing_table_t410", "admin")
            .await
            .unwrap_err();
        assert!(
            start2.elapsed() < Duration::from_millis(250),
            "无策略时不应退避"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&path2);
    }

    /// INSERT（非幂等）失败不重试：若错误重试将耗时 >= 900ms，正确行为 ~0ms
    #[tokio::test]
    async fn test_t410_execute_raw_write_is_not_retried() {
        let (url, path) = temp_db_url("insert");
        let config = config_with_policy(url.clone(), 2, 300);
        let pool = DbPool::with_config(config).await.unwrap();
        let session = pool.get_session("admin").await.unwrap();

        let start = Instant::now();
        let _err = session
            .execute_raw("INSERT INTO missing_table_t410 VALUES (1)")
            .await;
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_millis(900),
            "写类操作不应重试（正确耗时 ~0ms，若错误重试 >= 900ms），实际 {:?}",
            elapsed
        );

        let _ = std::fs::remove_file(&path);
    }

    fn config_with_policy_no_retry(url: String) -> DbConfig {
        DbConfig {
            url,
            retry_policy: None,
            ..Default::default()
        }
    }
}
