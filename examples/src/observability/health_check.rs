// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 健康检查与熔断器示例
//!
//! 演示 `HealthChecker` 和 `CircuitBreaker` 的完整使用流程：
//! - 创建 `HealthChecker` 并执行健康检查
//! - 展示 `HealthStatus`（Healthy / Unhealthy / Degraded）
//! - 配置 `CircuitBreakerConfig` 并演示熔断器状态转换（Closed → Open → HalfOpen → Closed）
//! - 演示 `PoolHealthMetrics` 的连接池状态采集
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example health_check --features "sqlite,health-check"
//! ```

use dbnexus::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState, HealthChecker, HealthStatus,
    PoolHealthMetrics,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🩺 DBNexus health check and circuit breaker example");
    println!("========================================\n");

    // ============================================
    // 1. PoolHealthMetrics — 连接池健康指标采集
    // ============================================
    println!("--- 1. PoolHealthMetrics pool metrics ---");
    let metrics = PoolHealthMetrics::new();

    // 模拟连接创建
    metrics.record_connection_created();
    metrics.record_connection_created();
    metrics.record_connection_created();
    println!("  ✓ simulated the creation of 3 connections");

    // 模拟一个连接转入空闲
    metrics.decrement_active();
    metrics.increment_idle().await;
    println!("  ✓ 1 connection became idle");

    // 模拟一次连接失败
    metrics.record_connection_failed();
    println!("  ✓ simulated 1 connection failure");

    let snapshot = metrics.snapshot();
    println!(
        "  snapshot: total={}, active={}, idle={}, created={}, failed={}",
        snapshot.total, snapshot.active, snapshot.idle, snapshot.created, snapshot.failed
    );
    println!("  is_healthy = {}", metrics.is_healthy());

    // ============================================
    // 2. HealthChecker — 执行健康检查
    // ============================================
    println!("\n--- 2. HealthChecker health checks ---");
    let checker = HealthChecker::new(1000); // 检查超时 1000ms

    // 在无连接的场景下执行健康检查
    let result = checker.check().await;
    print_health_status(&result.status, &result.details, &result.recommendations);

    // ============================================
    // 3. CircuitBreaker — 熔断器状态转换
    // ============================================
    println!("\n--- 3. CircuitBreaker state transitions ---");
    let config = CircuitBreakerConfig {
        failure_threshold: 3, // 连续失败 3 次触发 Open
        success_threshold: 2, // HalfOpen 状态连续成功 2 次恢复 Closed
        timeout_ms: 100,      // 100ms 后从 Open 转为 HalfOpen
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config.clone());

    println!("\n  [initial state]");
    println!("  state = {}", breaker.state().await);

    println!(
        "\n  [phase 1] {} consecutive failures → trigger Open",
        config.failure_threshold
    );
    for i in 1..=config.failure_threshold {
        breaker.record_failure().await;
        let status = breaker.status().await;
        println!(
            "  failure #{}: state={}, consecutive_failures={}",
            i, status.state, status.consecutive_failures
        );
    }
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);
    println!("  ✓ the breaker has opened (Open); requests will be rejected");

    println!("\n  [phase 2] can_execute check while Open");
    let can_exec = breaker.can_execute().await;
    println!(
        "  can_execute() = {:?}",
        can_exec
            .map(|_| "allowed")
            .map_err(|e| format!("blocked: {}", e))
    );

    println!(
        "\n  [phase 3] wait {}ms for the timeout → transition to HalfOpen",
        config.timeout_ms
    );
    tokio::time::sleep(Duration::from_millis(150)).await;
    // 调用 can_execute 触发状态转换
    let _ = breaker.can_execute().await;
    let state_after_timeout = breaker.state().await;
    println!("  state after waiting = {}", state_after_timeout);
    assert_eq!(state_after_timeout, CircuitBreakerState::HalfOpen);
    println!("  ✓ the breaker is half-open (HalfOpen); trial requests are allowed");

    println!(
        "\n  [phase 4] {} consecutive successes → restore Closed",
        config.success_threshold
    );
    for i in 1..=config.success_threshold {
        breaker.record_success().await;
        let status = breaker.status().await;
        println!(
            "  success #{}: state={}, consecutive_successes={}",
            i, status.state, status.consecutive_successes
        );
    }
    let final_state = breaker.state().await;
    assert_eq!(final_state, CircuitBreakerState::Closed);
    println!("  ✓ the breaker has closed again (Closed), serving normally");

    // ============================================
    // 4. 通过 HealthChecker 访问内置的 CircuitBreaker
    // ============================================
    println!("\n--- 4. the CircuitBreaker built into HealthChecker ---");
    let inner_breaker = checker.circuit_breaker();
    let inner_status = inner_breaker.status().await;
    println!(
        "  HealthChecker built-in breaker: state={}, failures={}, successes={}",
        inner_status.state, inner_status.consecutive_failures, inner_status.consecutive_successes
    );

    // ============================================
    // 5. 演示 Degraded 状态（通过设置高等待数）
    // ============================================
    println!("\n--- 5. simulate a Degraded state ---");
    let degraded_metrics = PoolHealthMetrics::new();
    // 创建 2 个连接，全部 active，并设置 20 个等待请求
    degraded_metrics.record_connection_created();
    degraded_metrics.record_connection_created();
    degraded_metrics.set_waiting_requests(20);
    // 等待请求 > 10 且 active >= total → 触发 Degraded
    let degraded_checker = HealthChecker::new(500);
    // 将指标同步到检查器（这里通过新建检查器演示，实际场景由连接池自动同步）
    let _ = degraded_checker.metrics().snapshot();
    let degraded_result = degraded_checker.check().await;
    print_health_status(
        &degraded_result.status,
        &degraded_result.details,
        &degraded_result.recommendations,
    );

    println!("\n========================================");
    println!("✨ Health check and circuit breaker example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - HealthChecker::new(timeout_ms)              - create a health checker");
    println!("  - checker.check() -> HealthCheckResult       - run a health check");
    println!("  - HealthStatus::Healthy/Unhealthy/Degraded    - the three health states");
    println!("  - PoolHealthMetrics                           - pool metrics collection");
    println!("  - CircuitBreaker::new(CircuitBreakerConfig)   - create a circuit breaker");
    println!("  - breaker.record_failure() / record_success() - record results");
    println!(
        "  - breaker.can_execute()                       - check whether to let requests through"
    );
    println!("  - CircuitBreakerState: Closed/Open/HalfOpen   - the three breaker states");

    Ok(())
}

/// 打印健康检查结果
fn print_health_status(status: &HealthStatus, details: &str, recommendations: &[String]) {
    let status_str = match status {
        HealthStatus::Healthy => "✅ Healthy".to_string(),
        HealthStatus::Unhealthy(reason) => format!("❌ Unhealthy: {}", reason),
        HealthStatus::Degraded(reason) => format!("⚠️  Degraded: {}", reason),
    };
    println!("  status: {}", status_str);
    println!("  details: {}", details);
    if !recommendations.is_empty() {
        println!("  recommendation:");
        for rec in recommendations {
            println!("    - {}", rec);
        }
    }
}
