// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 健康检查示例
//!
//! 展示如何使用 dbnexus 的健康检查功能：
//! - 连接池健康状态检查
//! - 熔断器模式
//! - 性能指标收集
//! - 自动恢复机制
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example health --features "sqlite,health-check"
//! ```

use dbnexus::health::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState,
    HealthCheckResult, HealthStatus, PoolHealthMetrics,
};
use dbnexus::{DbConfig, DbPool};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏥 DBNexus 健康检查示例\n");
    println!("========================================");

    // 1. 初始化数据库连接池
    println!("\n1️⃣ 初始化数据库连接池");
    println!("------------------------------------------");
    let config = DbConfig {
        url: "sqlite:file::memory:?cache=shared".to_string(),
        admin_role: "admin".to_string(),
        max_connections: 10,
        min_connections: 2,
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await?;
    println!("✓ 连接池创建成功");

    // 2. 连接池健康指标
    println!("\n2️⃣ 连接池健康指标");
    println!("------------------------------------------");
    let metrics = PoolHealthMetrics::new();

    // 模拟连接活动
    metrics.record_connection_created();
    metrics.record_connection_created();
    metrics.record_connection_created();
    println!("✓ 模拟创建了 3 个连接");

    let snapshot = metrics.snapshot();
    println!("  - 总连接数: {}", snapshot.total);
    println!("  - 活跃连接数: {}", snapshot.active);
    println!("  - 空闲连接数: {}", snapshot.idle);
    println!("  - 创建成功: {}", snapshot.created);

    // 3. 健康状态检查
    println!("\n3️⃣ 健康状态检查");
    println!("------------------------------------------");
    let is_healthy = metrics.is_healthy();
    println!("  连接池健康状态: {}", if is_healthy { "✅ 健康" } else { "❌ 不健康" });

    // 4. 熔断器配置
    println!("\n4️⃣ 熔断器配置");
    println!("------------------------------------------");
    let cb_config = CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 3,
        timeout_ms: 30000,
        window_size: 100,
    };
    println!("  - 失败阈值: {}", cb_config.failure_threshold);
    println!("  - 成功阈值: {}", cb_config.success_threshold);
    println!("  - 超时时间: {}ms", cb_config.timeout_ms);
    println!("  - 滑动窗口: {}", cb_config.window_size);

    // 5. 创建熔断器
    println!("\n5️⃣ 创建熔断器");
    println!("------------------------------------------");
    let circuit_breaker = CircuitBreaker::new(cb_config);
    println!("✓ 熔断器创建成功");

    // 6. 熔断器状态检查
    println!("\n6️⃣ 熔断器状态检查");
    println!("------------------------------------------");
    let state = circuit_breaker.state().await;
    println!("  当前状态: {}", state);

    // 7. 模拟成功请求
    println!("\n7️⃣ 模拟成功请求");
    println!("------------------------------------------");
    for i in 1..=3 {
        circuit_breaker.record_success().await;
        let state = circuit_breaker.state().await;
        println!("  请求 {} 成功, 状态: {}", i, state);
    }

    // 8. 模拟失败请求
    println!("\n8️⃣ 模拟失败请求（触发熔断）");
    println!("------------------------------------------");
    for i in 1..=6 {
        circuit_breaker.record_failure().await;
        let state = circuit_breaker.state().await;
        println!("  请求 {} 失败, 状态: {}", i, state);
        if state == CircuitBreakerState::Open {
            println!("  ⚠️ 熔断器已打开，停止接受请求");
            break;
        }
    }

    // 9. 检查是否允许请求
    println!("\n9️⃣ 检查是否允许请求");
    println!("------------------------------------------");
    match circuit_breaker.can_execute().await {
        Ok(()) => println!("  ✅ 允许请求"),
        Err(e) => println!("  ❌ 拒绝请求: {}", e),
    }

    // 10. 健康检查结果示例
    println!("\n🔟 健康检查结果示例");
    println!("------------------------------------------");
    let result = HealthCheckResult {
        status: HealthStatus::Healthy,
        latency: Duration::from_millis(5),
        details: "All connections are healthy".to_string(),
        recommendations: vec![],
    };
    println!("  状态: {:?}", result.status);
    println!("  延迟: {:?}", result.latency);
    println!("  详情: {}", result.details);

    // 11. 降级状态示例
    println!("\n1️⃣1️⃣ 降级状态示例");
    println!("------------------------------------------");
    let degraded_result = HealthCheckResult {
        status: HealthStatus::Degraded("High latency detected".to_string()),
        latency: Duration::from_millis(500),
        details: "Connection pool is under high load".to_string(),
        recommendations: vec![
            "Consider increasing max_connections".to_string(),
            "Check for slow queries".to_string(),
        ],
    };
    println!("  状态: {:?}", degraded_result.status);
    println!("  建议:");
    for rec in &degraded_result.recommendations {
        println!("    - {}", rec);
    }

    // 12. 连接池状态
    println!("\n1️⃣2️⃣ 连接池状态");
    println!("------------------------------------------");
    let pool_status = pool.status();
    println!("  总连接数: {}", pool_status.total);
    println!("  活跃连接数: {}", pool_status.active);
    println!("  空闲连接数: {}", pool_status.idle);
    println!("  等待请求数: {}", pool_status.wait_count);

    println!("\n=== 所有健康检查示例完成 ===");
    Ok(())
}
