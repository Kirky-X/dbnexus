// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 运行时重试 + 指数退避示例
//!
//! 演示 `RetryExecutor` 的使用：
//! - 配置 `RetryPolicy`（最大重试、退避策略、抖动）
//! - 幂等查询自动重试（SELECT / SHOW / EXPLAIN）
//! - 非幂等操作拒绝重试（INSERT / UPDATE / DELETE）
//! - 整体超时控制
//! - `is_idempotent_operation` 判断逻辑
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --bin retry
//! ```

use dbnexus::is_idempotent_operation;
use dbnexus::{RetryExecutor, RetryPolicy};
use std::sync::atomic::{AtomicU32, Ordering};

/// 模拟可失败的操作
async fn simulate_operation(
    attempt_counter: &AtomicU32,
    fail_times: u32,
    success_value: &str,
) -> Result<String, dbnexus::foundation::DbError> {
    let attempt = attempt_counter.fetch_add(1, Ordering::SeqCst);
    if attempt < fail_times {
        Err(dbnexus::foundation::DbError::Query(format!(
            "simulated failure (attempt {})",
            attempt + 1
        )))
    } else {
        Ok(success_value.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔄 DBNexus runtime retry + exponential backoff example");
    println!("========================================\n");

    // 幂等性判断
    println!("--- 1. idempotency detection ---\n");

    let test_cases = [
        ("SELECT * FROM users", true),
        ("  select count(*) from orders", true),
        ("SHOW TABLES", true),
        ("EXPLAIN SELECT * FROM users", true),
        ("INSERT INTO users VALUES (1)", false),
        ("UPDATE users SET name = 'x'", false),
        ("DELETE FROM users", false),
        ("CREATE TABLE foo (id INT)", false),
        ("DROP TABLE foo", false),
        ("ALTER TABLE foo ADD COLUMN bar INT", false),
    ];

    println!("  ┌──────────────────────────────────────┬──────────┐");
    println!("  │ SQL                                  │ retryable? │");
    println!("  ├──────────────────────────────────────┼──────────┤");
    for (sql, expected) in &test_cases {
        let result = is_idempotent_operation(sql);
        let icon = if result { "✅ yes" } else { "❌ no" };
        let display_sql = if sql.len() > 36 { &sql[..36] } else { sql };
        println!("  │ {:<36} │ {} │", display_sql, icon);
        assert_eq!(result, *expected, "idempotency check failed: {}", sql);
    }
    println!("  └──────────────────────────────────────┴──────────┘");
    println!();

    // 默认重试策略 — 幂等查询最终成功
    println!("--- 2. idempotent query retry (succeeding after 2 failures) ---\n");

    let policy = RetryPolicy::default();
    println!("  retry strategy:");
    println!("  - max_retries      : {}", policy.max_retries);
    println!("  - initial_backoff  : {} ms", policy.initial_backoff_ms);
    println!("  - max_backoff      : {} ms", policy.max_backoff_ms);
    println!("  - multiplier       : {:.1}", policy.multiplier);
    println!("  - jitter           : {}", policy.jitter);
    println!();

    let counter = AtomicU32::new(0);
    let result = RetryExecutor::execute_with_retry(
        &policy,
        || {
            let counter = &counter;
            async move { simulate_operation(counter, 2, "query succeeded").await }
        },
        "SELECT * FROM users",
    )
    .await;

    match &result {
        Ok(val) => {
            let attempts = counter.load(Ordering::SeqCst);
            println!("  ✅ retry succeeded!");
            println!("  - return value     : {}", val);
            println!("  - total attempts : {}", attempts);
        }
        Err(e) => println!("  ❌ failed: {}", e),
    }
    println!();

    // 非幂等操作 — 直接失败不重试
    println!("--- 3. non-idempotent operation (INSERT, not retried) ---\n");

    let counter = AtomicU32::new(0);
    let result = RetryExecutor::execute_with_retry(
        &policy,
        || {
            let counter = &counter;
            async move { simulate_operation(counter, 5, "should not be reached").await }
        },
        "INSERT INTO users VALUES (1)",
    )
    .await;

    match &result {
        Ok(_) => println!("  ❌ should not have succeeded"),
        Err(e) => {
            let attempts = counter.load(Ordering::SeqCst);
            println!("  ✅ correctly rejected the retry!");
            println!("  - error type   : RetryError::NonRetryable");
            println!("  - error message   : {}", e);
            println!("  - actual attempts  : {} (only 1)", attempts);
        }
    }
    println!();

    // 重试耗尽 — 超过最大重试次数
    println!("--- 4. retry exhausted (10 failures, max 3 retries) ---\n");

    let counter = AtomicU32::new(0);
    let result = RetryExecutor::execute_with_retry(
        &policy,
        || {
            let counter = &counter;
            async move { simulate_operation(counter, 10, "will not succeed").await }
        },
        "SELECT * FROM large_table",
    )
    .await;

    match &result {
        Ok(_) => println!("  ❌ should not have succeeded"),
        Err(e) => {
            let attempts = counter.load(Ordering::SeqCst);
            println!("  ✅ correctly returned the Exhausted error!");
            println!("  - error type   : RetryError::Exhausted");
            println!(
                "  - total attempts : {} (1 initial + {} retries)",
                attempts, policy.max_retries
            );
            println!("  - error message   : {}", e);
        }
    }
    println!();

    // 自定义重试策略
    println!("--- 5. custom retry strategy ---\n");

    let custom_policy = RetryPolicy {
        max_retries: 5,
        initial_backoff_ms: 50,
        max_backoff_ms: 1000,
        multiplier: 1.5,
        jitter: false,
        overall_timeout_ms: Some(5000),
        retry_busy_writes: false,
    };

    println!("  custom strategy:");
    println!("  - max_retries      : {}", custom_policy.max_retries);
    println!(
        "  - initial_backoff  : {} ms",
        custom_policy.initial_backoff_ms
    );
    println!("  - max_backoff      : {} ms", custom_policy.max_backoff_ms);
    println!("  - multiplier       : {:.1}", custom_policy.multiplier);
    println!("  - jitter           : {}", custom_policy.jitter);
    println!(
        "  - overall_timeout  : {:?} ms",
        custom_policy.overall_timeout_ms
    );
    println!();

    println!("  backoff sequence (no jitter):");
    let mut backoff = custom_policy.initial_backoff_ms as f64;
    for i in 0..custom_policy.max_retries {
        let capped = backoff.min(custom_policy.max_backoff_ms as f64);
        println!("    retry #{}: {:.0} ms", i + 1, capped);
        backoff *= custom_policy.multiplier;
    }
    println!();

    let counter = AtomicU32::new(0);
    let result = RetryExecutor::execute_with_retry(
        &custom_policy,
        || {
            let counter = &counter;
            async move { simulate_operation(counter, 4, "custom strategy succeeded").await }
        },
        "SELECT count(*) FROM orders",
    )
    .await;

    match &result {
        Ok(val) => {
            let attempts = counter.load(Ordering::SeqCst);
            println!("  ✅ the custom strategy retry succeeded!");
            println!("  - return value     : {}", val);
            println!("  - total attempts : {}", attempts);
        }
        Err(e) => println!("  ❌ failed: {}", e),
    }
    println!();

    // 首次即成功 — 无需重试
    println!("--- 6. success on the first try (no retry needed) ---\n");

    let counter = AtomicU32::new(0);
    let result = RetryExecutor::execute_with_retry(
        &policy,
        || {
            let counter = &counter;
            async move { simulate_operation(counter, 0, "succeeded on the first attempt").await }
        },
        "SELECT 1",
    )
    .await;

    match &result {
        Ok(val) => {
            let attempts = counter.load(Ordering::SeqCst);
            println!("  ✅ succeeded on the first try!");
            println!("  - return value     : {}", val);
            println!("  - total attempts : {} (no retry)", attempts);
        }
        Err(e) => println!("  ❌ failed: {}", e),
    }
    println!();

    // RetryError 类型展示
    println!("--- 7. RetryError types ---\n");
    println!("  ┌─────────────────┬──────────────────────────────────────┐");
    println!("  │ variant           │ description                                 │");
    println!("  ├─────────────────┼──────────────────────────────────────┤");
    println!("  │ Exhausted       │ retries exhausted, contains the last error     │");
    println!("  │ NonRetryable    │ a non-idempotent operation is not retried     │");
    println!("  │ Timeout         │ the overall wall-clock timeout               │");
    println!("  └─────────────────┴──────────────────────────────────────┘");
    println!();

    println!("========================================");
    println!("✨ Runtime retry example complete!");
    println!("========================================");
    println!("\n📚 Key APIs:");
    println!("  - RetryPolicy {{ max_retries, initial_backoff_ms, .. }}");
    println!("  - RetryExecutor::execute_with_retry(&policy, closure, sql)");
    println!("  - is_idempotent_operation(sql) -> bool");
    Ok(())
}
