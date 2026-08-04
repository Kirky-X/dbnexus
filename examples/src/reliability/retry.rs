// Copyright (c) 2026 Kirky.X
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
            "模拟失败（第 {} 次尝试）",
            attempt + 1
        )))
    } else {
        Ok(success_value.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🔄 DBNexus 运行时重试 + 指数退避示例");
    println!("========================================\n");

    // ============================================
    // 1. 幂等性判断
    // ============================================
    println!("--- 1. 幂等性判断 ---\n");

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
    println!("  │ SQL                                  │ 可重试？ │");
    println!("  ├──────────────────────────────────────┼──────────┤");
    for (sql, expected) in &test_cases {
        let result = is_idempotent_operation(sql);
        let icon = if result { "✅ 是" } else { "❌ 否" };
        let display_sql = if sql.len() > 36 { &sql[..36] } else { sql };
        println!("  │ {:<36} │ {} │", display_sql, icon);
        assert_eq!(result, *expected, "幂等性判断失败: {}", sql);
    }
    println!("  └──────────────────────────────────────┴──────────┘");
    println!();

    // ============================================
    // 2. 默认重试策略 — 幂等查询最终成功
    // ============================================
    println!("--- 2. 幂等查询重试（失败 2 次后成功） ---\n");

    let policy = RetryPolicy::default();
    println!("  重试策略:");
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
            async move { simulate_operation(counter, 2, "查询成功").await }
        },
        "SELECT * FROM users",
    )
    .await;

    match &result {
        Ok(val) => {
            let attempts = counter.load(Ordering::SeqCst);
            println!("  ✅ 重试成功！");
            println!("  - 返回值     : {}", val);
            println!("  - 总尝试次数 : {}", attempts);
        }
        Err(e) => println!("  ❌ 失败: {}", e),
    }
    println!();

    // ============================================
    // 3. 非幂等操作 — 直接失败不重试
    // ============================================
    println!("--- 3. 非幂等操作（INSERT，不重试） ---\n");

    let counter = AtomicU32::new(0);
    let result = RetryExecutor::execute_with_retry(
        &policy,
        || {
            let counter = &counter;
            async move { simulate_operation(counter, 5, "不应该到达").await }
        },
        "INSERT INTO users VALUES (1)",
    )
    .await;

    match &result {
        Ok(_) => println!("  ❌ 不应该成功"),
        Err(e) => {
            let attempts = counter.load(Ordering::SeqCst);
            println!("  ✅ 正确拒绝重试！");
            println!("  - 错误类型   : RetryError::NonRetryable");
            println!("  - 错误信息   : {}", e);
            println!("  - 实际尝试   : {} 次（仅 1 次）", attempts);
        }
    }
    println!();

    // ============================================
    // 4. 重试耗尽 — 超过最大重试次数
    // ============================================
    println!("--- 4. 重试耗尽（失败 10 次，最大重试 3 次） ---\n");

    let counter = AtomicU32::new(0);
    let result = RetryExecutor::execute_with_retry(
        &policy,
        || {
            let counter = &counter;
            async move { simulate_operation(counter, 10, "不会成功").await }
        },
        "SELECT * FROM large_table",
    )
    .await;

    match &result {
        Ok(_) => println!("  ❌ 不应该成功"),
        Err(e) => {
            let attempts = counter.load(Ordering::SeqCst);
            println!("  ✅ 正确返回 Exhausted 错误！");
            println!("  - 错误类型   : RetryError::Exhausted");
            println!("  - 总尝试次数 : {} (1 首次 + {} 重试)", attempts, policy.max_retries);
            println!("  - 错误信息   : {}", e);
        }
    }
    println!();

    // ============================================
    // 5. 自定义重试策略
    // ============================================
    println!("--- 5. 自定义重试策略 ---\n");

    let custom_policy = RetryPolicy {
        max_retries: 5,
        initial_backoff_ms: 50,
        max_backoff_ms: 1000,
        multiplier: 1.5,
        jitter: false,
        overall_timeout_ms: Some(5000),
    };

    println!("  自定义策略:");
    println!("  - max_retries      : {}", custom_policy.max_retries);
    println!("  - initial_backoff  : {} ms", custom_policy.initial_backoff_ms);
    println!("  - max_backoff      : {} ms", custom_policy.max_backoff_ms);
    println!("  - multiplier       : {:.1}", custom_policy.multiplier);
    println!("  - jitter           : {}", custom_policy.jitter);
    println!("  - overall_timeout  : {:?} ms", custom_policy.overall_timeout_ms);
    println!();

    // 展示退避时间序列
    println!("  退避时间序列（无 jitter）:");
    let mut backoff = custom_policy.initial_backoff_ms as f64;
    for i in 0..custom_policy.max_retries {
        let capped = backoff.min(custom_policy.max_backoff_ms as f64);
        println!("    第 {} 次重试: {:.0} ms", i + 1, capped);
        backoff *= custom_policy.multiplier;
    }
    println!();

    let counter = AtomicU32::new(0);
    let result = RetryExecutor::execute_with_retry(
        &custom_policy,
        || {
            let counter = &counter;
            async move { simulate_operation(counter, 4, "自定义策略成功").await }
        },
        "SELECT count(*) FROM orders",
    )
    .await;

    match &result {
        Ok(val) => {
            let attempts = counter.load(Ordering::SeqCst);
            println!("  ✅ 自定义策略重试成功！");
            println!("  - 返回值     : {}", val);
            println!("  - 总尝试次数 : {}", attempts);
        }
        Err(e) => println!("  ❌ 失败: {}", e),
    }
    println!();

    // ============================================
    // 6. 首次即成功 — 无需重试
    // ============================================
    println!("--- 6. 首次即成功（无需重试） ---\n");

    let counter = AtomicU32::new(0);
    let result = RetryExecutor::execute_with_retry(
        &policy,
        || {
            let counter = &counter;
            async move { simulate_operation(counter, 0, "首次成功").await }
        },
        "SELECT 1",
    )
    .await;

    match &result {
        Ok(val) => {
            let attempts = counter.load(Ordering::SeqCst);
            println!("  ✅ 首次执行即成功！");
            println!("  - 返回值     : {}", val);
            println!("  - 总尝试次数 : {}（无重试）", attempts);
        }
        Err(e) => println!("  ❌ 失败: {}", e),
    }
    println!();

    // ============================================
    // 7. RetryError 类型展示
    // ============================================
    println!("--- 7. RetryError 类型 ---\n");
    println!("  ┌─────────────────┬──────────────────────────────────────┐");
    println!("  │ 变体            │ 说明                                 │");
    println!("  ├─────────────────┼──────────────────────────────────────┤");
    println!("  │ Exhausted       │ 重试次数耗尽，包含最后一次错误       │");
    println!("  │ NonRetryable    │ 非幂等操作被拒绝重试                 │");
    println!("  │ Timeout         │ 整体 wall-clock 超时                 │");
    println!("  └─────────────────┴──────────────────────────────────────┘");
    println!();

    println!("========================================");
    println!("✨ 运行时重试示例完成！");
    println!("========================================");
    println!("\n📚 关键 API:");
    println!("  - RetryPolicy {{ max_retries, initial_backoff_ms, .. }}");
    println!("  - RetryExecutor::execute_with_retry(&policy, closure, sql)");
    println!("  - is_idempotent_operation(sql) -> bool");
    Ok(())
}
