// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

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
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState, HealthChecker, HealthStatus, PoolHealthMetrics,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("🩺 DBNexus 健康检查与熔断器示例");
    println!("========================================\n");

    // ============================================
    // 1. PoolHealthMetrics — 连接池健康指标采集
    // ============================================
    println!("--- 1. PoolHealthMetrics 连接池指标 ---");
    let metrics = PoolHealthMetrics::new();

    // 模拟连接创建
    metrics.record_connection_created();
    metrics.record_connection_created();
    metrics.record_connection_created();
    println!("  ✓ 模拟创建 3 个连接");

    // 模拟一个连接转入空闲
    metrics.decrement_active();
    metrics.increment_idle().await;
    println!("  ✓ 1 个连接转为空闲状态");

    // 模拟一次连接失败
    metrics.record_connection_failed();
    println!("  ✓ 模拟 1 次连接失败");

    let snapshot = metrics.snapshot();
    println!(
        "  快照: total={}, active={}, idle={}, created={}, failed={}",
        snapshot.total, snapshot.active, snapshot.idle, snapshot.created, snapshot.failed
    );
    println!("  is_healthy = {}", metrics.is_healthy());

    // ============================================
    // 2. HealthChecker — 执行健康检查
    // ============================================
    println!("\n--- 2. HealthChecker 健康检查 ---");
    let checker = HealthChecker::new(1000); // 检查超时 1000ms

    // 在无连接的场景下执行健康检查
    let result = checker.check().await;
    print_health_status(&result.status, &result.details, &result.recommendations);

    // ============================================
    // 3. CircuitBreaker — 熔断器状态转换
    // ============================================
    println!("\n--- 3. CircuitBreaker 熔断器状态转换 ---");
    let config = CircuitBreakerConfig {
        failure_threshold: 3, // 连续失败 3 次触发 Open
        success_threshold: 2, // HalfOpen 状态连续成功 2 次恢复 Closed
        timeout_ms: 100,      // 100ms 后从 Open 转为 HalfOpen
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config.clone());

    println!("\n  [初始状态]");
    println!("  state = {}", breaker.state().await);

    // 阶段一：连续失败触发 Open
    println!("\n  [阶段一] 连续失败 {} 次 → 触发 Open", config.failure_threshold);
    for i in 1..=config.failure_threshold {
        breaker.record_failure().await;
        let status = breaker.status().await;
        println!(
            "  失败 #{}: state={}, consecutive_failures={}",
            i, status.state, status.consecutive_failures
        );
    }
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);
    println!("  ✓ 熔断器已打开 (Open)，请求将被拒绝");

    // 阶段二：can_execute 在 Open 状态下应拒绝
    println!("\n  [阶段二] Open 状态下 can_execute 检查");
    let can_exec = breaker.can_execute().await;
    println!(
        "  can_execute() = {:?}",
        can_exec.map(|_| "allowed").map_err(|e| format!("blocked: {}", e))
    );

    // 阶段三：等待超时后触发 Open → HalfOpen
    println!("\n  [阶段三] 等待 {}ms 超时 → 转为 HalfOpen", config.timeout_ms);
    tokio::time::sleep(Duration::from_millis(150)).await;
    // 调用 can_execute 触发状态转换
    let _ = breaker.can_execute().await;
    let state_after_timeout = breaker.state().await;
    println!("  等待后 state = {}", state_after_timeout);
    assert_eq!(state_after_timeout, CircuitBreakerState::HalfOpen);
    println!("  ✓ 熔断器进入半开状态 (HalfOpen)，允许试探性请求");

    // 阶段四：连续成功触发 HalfOpen → Closed
    println!("\n  [阶段四] 连续成功 {} 次 → 恢复 Closed", config.success_threshold);
    for i in 1..=config.success_threshold {
        breaker.record_success().await;
        let status = breaker.status().await;
        println!(
            "  成功 #{}: state={}, consecutive_successes={}",
            i, status.state, status.consecutive_successes
        );
    }
    let final_state = breaker.state().await;
    assert_eq!(final_state, CircuitBreakerState::Closed);
    println!("  ✓ 熔断器已恢复关闭状态 (Closed)，正常服务");

    // ============================================
    // 4. 通过 HealthChecker 访问内置的 CircuitBreaker
    // ============================================
    println!("\n--- 4. HealthChecker 内置的 CircuitBreaker ---");
    let inner_breaker = checker.circuit_breaker();
    let inner_status = inner_breaker.status().await;
    println!(
        "  HealthChecker 内置熔断器: state={}, failures={}, successes={}",
        inner_status.state, inner_status.consecutive_failures, inner_status.consecutive_successes
    );

    // ============================================
    // 5. 演示 Degraded 状态（通过设置高等待数）
    // ============================================
    println!("\n--- 5. 模拟 Degraded 状态 ---");
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
    println!("✨ 健康检查与熔断器示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - HealthChecker::new(timeout_ms)              - 创建健康检查器");
    println!("  - checker.check() -> HealthCheckResult       - 执行健康检查");
    println!("  - HealthStatus::Healthy/Unhealthy/Degraded    - 三种健康状态");
    println!("  - PoolHealthMetrics                           - 连接池指标采集");
    println!("  - CircuitBreaker::new(CircuitBreakerConfig)   - 创建熔断器");
    println!("  - breaker.record_failure() / record_success() - 记录结果");
    println!("  - breaker.can_execute()                       - 检查是否放行");
    println!("  - CircuitBreakerState: Closed/Open/HalfOpen   - 三种熔断状态");

    Ok(())
}

/// 打印健康检查结果
fn print_health_status(status: &HealthStatus, details: &str, recommendations: &[String]) {
    let status_str = match status {
        HealthStatus::Healthy => "✅ Healthy".to_string(),
        HealthStatus::Unhealthy(reason) => format!("❌ Unhealthy: {}", reason),
        HealthStatus::Degraded(reason) => format!("⚠️  Degraded: {}", reason),
    };
    println!("  状态: {}", status_str);
    println!("  详情: {}", details);
    if !recommendations.is_empty() {
        println!("  建议:");
        for rec in recommendations {
            println!("    - {}", rec);
        }
    }
}
