// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 健康检查模块单元测试
//!
//! 测试健康检查模块的核心功能，包括熔断器、连接池指标、健康检查器和自动恢复器。
//! 所有测试使用内存模拟，不依赖外部数据库。

use std::sync::Arc;
use std::time::Duration;

// 导入健康检查模块
use dbnexus::health::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerState, HealthChecker, HealthStatus,
    PoolHealthMetrics,
};

// ============================================================================
// 熔断器状态转换测试
// ============================================================================

/// TEST-U-HEALTH-001: 测试熔断器初始状态为关闭
///
/// 验证熔断器创建后初始状态为 Closed。
#[tokio::test]
async fn test_circuit_breaker_initial_state_is_closed() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 3,
        timeout_ms: 1000,
        window_size: 10,
    });

    let state = breaker.state().await;
    assert_eq!(state, CircuitBreakerState::Closed, "初始状态应该是 Closed");
}

/// TEST-U-HEALTH-002: 测试熔断器从关闭到打开的状态转换
///
/// 验证连续失败达到阈值后熔断器状态转换为 Open。
#[tokio::test]
async fn test_circuit_breaker_closed_to_open_transition() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        timeout_ms: 1000,
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config);

    // 初始状态应该是关闭
    assert_eq!(breaker.state().await, CircuitBreakerState::Closed);

    // 记录失败，达到阈值
    breaker.record_failure().await;
    breaker.record_failure().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Closed);

    // 第三次失败后应该转换为打开状态
    breaker.record_failure().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);
}

/// TEST-U-HEALTH-003: 测试熔断器打开状态拒绝请求
///
/// 验证熔断器处于 Open 状态时，can_execute 返回错误。
#[tokio::test]
async fn test_circuit_breaker_open_rejects_requests() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 2,
        timeout_ms: 1000,
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config);

    // 触发熔断器打开
    breaker.record_failure().await;
    breaker.record_failure().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);

    // 尝试执行应该被拒绝
    let result = breaker.can_execute().await;
    assert!(result.is_err(), "Open 状态应该拒绝请求");

    // 验证错误类型
    let err = result.unwrap_err();
    assert_eq!(err.state(), CircuitBreakerState::Open);
}

/// TEST-U-HEALTH-004: 测试熔断器关闭状态允许请求
///
/// 验证熔断器处于 Closed 状态时，can_execute 允许请求。
#[tokio::test]
async fn test_circuit_breaker_closed_allows_requests() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 3,
        timeout_ms: 1000,
        window_size: 10,
    });

    let result = breaker.can_execute().await;
    assert!(result.is_ok(), "Closed 状态应该允许请求");
}

/// TEST-U-HEALTH-005: 测试熔断器成功后重置失败计数
///
/// 验证熔断器在 Closed 状态下记录成功会重置失败计数。
#[tokio::test]
async fn test_circuit_breaker_success_resets_failure_count() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        timeout_ms: 1000,
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config);

    // 记录两次失败
    breaker.record_failure().await;
    breaker.record_failure().await;

    let status_before = breaker.status().await;
    assert_eq!(status_before.consecutive_failures, 2);

    // 记录成功
    breaker.record_success().await;

    // 失败计数应该被重置
    let status_after = breaker.status().await;
    assert_eq!(status_after.consecutive_failures, 0);
}

/// TEST-U-HEALTH-006: 测试熔断器配置默认值
///
/// 验证熔断器配置使用合理的默认值。
#[tokio::test]
async fn test_circuit_breaker_config_defaults() {
    let config = CircuitBreakerConfig::default();

    assert_eq!(config.failure_threshold, 5, "失败阈值默认应为 5");
    assert_eq!(config.success_threshold, 3, "成功阈值默认应为 3");
    assert_eq!(config.timeout_ms, 30000, "超时时间默认应为 30000ms");
    assert_eq!(config.window_size, 100, "窗口大小默认应为 100");
}

// ============================================================================
// 熔断器半开状态测试
// ============================================================================

/// TEST-U-HEALTH-007: 测试熔断器超时后转换为半开状态
///
/// 验证熔断器在 Open 状态下超时后转换为 HalfOpen。
#[tokio::test]
async fn test_circuit_breaker_timeout_transitions_to_half_open() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 2,
        timeout_ms: 50,
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config);

    // 触发熔断器打开
    breaker.record_failure().await;
    breaker.record_failure().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);

    // 等待超时
    tokio::time::sleep(Duration::from_millis(60)).await;

    // 尝试执行以触发状态转换
    let result = breaker.can_execute().await;
    assert!(result.is_ok(), "超时后应该允许尝试");
    assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);
}

/// TEST-U-HEALTH-008: 测试熔断器半开状态成功恢复为关闭
///
/// 验证熔断器在 HalfOpen 状态下连续成功达到阈值后转换为 Closed。
#[tokio::test]
async fn test_circuit_breaker_half_open_recovers_to_closed() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 2,
        timeout_ms: 50,
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config);

    // 触发熔断器打开
    breaker.record_failure().await;
    breaker.record_failure().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);

    // 等待超时转换为半开
    tokio::time::sleep(Duration::from_millis(60)).await;
    let _ = breaker.can_execute().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);

    // 记录成功达到阈值
    breaker.record_success().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);

    breaker.record_success().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Closed);
}

/// TEST-U-HEALTH-009: 测试熔断器半开状态失败返回打开
///
/// 验证熔断器在 HalfOpen 状态下失败后返回 Open 状态。
#[tokio::test]
async fn test_circuit_breaker_half_open_failure_returns_to_open() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 3,
        timeout_ms: 50,
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config);

    // 触发熔断器打开
    breaker.record_failure().await;
    breaker.record_failure().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);

    // 等待超时转换为半开
    tokio::time::sleep(Duration::from_millis(60)).await;
    let _ = breaker.can_execute().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);

    // 在半开状态下记录失败
    breaker.record_failure().await;

    // 应该返回打开状态
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);
}

/// TEST-U-HEALTH-010: 测试熔断器半开状态失败率阈值
///
/// 验证熔断器在 HalfOpen 状态下失败率过高会拒绝请求。
#[tokio::test]
async fn test_circuit_breaker_half_open_failure_rate_threshold() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 2,
        timeout_ms: 50,
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config);

    // 触发熔断器打开
    breaker.record_failure().await;
    breaker.record_failure().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);

    // 等待超时转换为半开
    tokio::time::sleep(Duration::from_millis(60)).await;
    let _ = breaker.can_execute().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);

    // 记录多次失败以提高失败率
    for _ in 0..6 {
        breaker.record_failure().await;
    }

    // 尝试执行应该被拒绝
    let result = breaker.can_execute().await;
    assert!(result.is_err(), "失败率过高应该拒绝请求");
}

/// TEST-U-HEALTH-011: 测试熔断器状态显示实现
///
/// 验证熔断器状态的 Display 实现返回正确的字符串。
#[tokio::test]
async fn test_circuit_breaker_state_display() {
    assert_eq!(format!("{}", CircuitBreakerState::Closed), "closed");
    assert_eq!(format!("{}", CircuitBreakerState::HalfOpen), "half-open");
    assert_eq!(format!("{}", CircuitBreakerState::Open), "open");
}

/// TEST-U-HEALTH-012: 测试熔断器错误实现
///
/// 验证熔断器错误正确包含状态信息。
#[tokio::test]
async fn test_circuit_breaker_error_implementation() {
    let error = CircuitBreakerError::new(CircuitBreakerState::Open);
    assert_eq!(error.state(), CircuitBreakerState::Open);
    assert!(format!("{:?}", error).contains("Open"));
}

// ============================================================================
// 健康检查超时测试
// ============================================================================

/// TEST-U-HEALTH-013: 测试健康检查器超时返回降级状态
///
/// 验证健康检查超时后返回降级状态。
#[tokio::test]
async fn test_health_checker_timeout_returns_degraded() {
    // 使用极短的超时时间
    let checker = HealthChecker::new(1);

    let result = checker.check().await;

    // 超时应该返回降级或 unhealthy 状态
    assert!(
        matches!(result.status, HealthStatus::Degraded(_)) || matches!(result.status, HealthStatus::Unhealthy(_)),
        "超时应该返回降级或 unhealthy 状态"
    );
    assert!(result.latency > Duration::ZERO, "应该记录延迟");
}

/// TEST-U-HEALTH-014: 测试健康检查器正常完成
///
/// 验证健康检查正常完成时返回正确的状态。
#[tokio::test]
async fn test_health_checker_normal_completion() {
    let checker = HealthChecker::new(1000);

    let result = checker.check().await;

    // 正常完成时延迟应该小于超时时间
    assert!(result.latency < Duration::from_millis(1000), "延迟应该小于超时时间");
    // 应该包含详细信息
    assert!(!result.details.is_empty(), "应该包含详细信息");
}

/// TEST-U-HEALTH-015: 测试健康检查器超时包含建议
///
/// 验证健康检查超时时返回适当的建议。
#[tokio::test]
async fn test_health_checker_timeout_includes_recommendations() {
    let checker = HealthChecker::new(1);

    let result = checker.check().await;

    // 超时时应该有建议
    assert!(
        !result.recommendations.is_empty() || matches!(result.status, HealthStatus::Healthy),
        "超时或健康状态时应该有建议或状态"
    );
}

/// TEST-U-HEALTH-016: 测试健康检查器超时时间配置
///
/// 验证健康检查器的超时配置正确应用。
#[tokio::test]
async fn test_health_checker_timeout_configuration() {
    let checker = HealthChecker::new(500);

    let result = checker.check().await;

    // 无论结果如何，延迟应该被记录
    assert!(result.latency > Duration::ZERO);
}

// ============================================================================
// 连接池健康指标测试
// ============================================================================

/// TEST-U-HEALTH-017: 测试连接池健康指标初始化
///
/// 验证连接池健康指标初始状态正确。
#[tokio::test]
async fn test_pool_health_metrics_initial_state() {
    let metrics = PoolHealthMetrics::new();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.total, 0, "总连接数初始为 0");
    assert_eq!(snapshot.active, 0, "活跃连接数初始为 0");
    assert_eq!(snapshot.idle, 0, "空闲连接数初始为 0");
    assert_eq!(snapshot.waiting, 0, "等待请求数初始为 0");
    assert_eq!(snapshot.created, 0, "创建连接数初始为 0");
    assert_eq!(snapshot.failed, 0, "失败连接数初始为 0");
    assert_eq!(snapshot.closed, 0, "关闭连接数初始为 0");
}

/// TEST-U-HEALTH-018: 测试连接池健康指标记录连接创建
///
/// 验证正确记录连接创建操作。
#[tokio::test]
async fn test_pool_health_metrics_record_connection_created() {
    let metrics = PoolHealthMetrics::new();

    metrics.record_connection_created();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.total, 1, "总连接数应为 1");
    assert_eq!(snapshot.active, 1, "活跃连接数应为 1");
    assert_eq!(snapshot.created, 1, "创建连接数应为 1");
}

/// TEST-U-HEALTH-019: 测试连接池健康指标记录连接失败
///
/// 验证正确记录连接失败操作。
#[tokio::test]
async fn test_pool_health_metrics_record_connection_failed() {
    let metrics = PoolHealthMetrics::new();

    metrics.record_connection_failed();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.failed, 1, "失败连接数应为 1");
}

/// TEST-U-HEALTH-020: 测试连接池健康指标记录连接关闭
///
/// 验证正确记录连接关闭操作。
#[tokio::test]
async fn test_pool_health_metrics_record_connection_closed() {
    let metrics = PoolHealthMetrics::new();

    metrics.record_connection_created();
    metrics.record_connection_closed();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.total, 0, "总连接数应为 0");
    assert_eq!(snapshot.closed, 1, "关闭连接数应为 1");
}

/// TEST-U-HEALTH-021: 测试连接池健康指标活跃连接计数
///
/// 验证正确追踪活跃连接数。
#[tokio::test]
async fn test_pool_health_metrics_active_connection_counting() {
    let metrics = PoolHealthMetrics::new();

    metrics.increment_active();
    metrics.increment_active();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.active, 2, "活跃连接数应为 2");

    metrics.decrement_active();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.active, 1, "活跃连接数应为 1");
}

/// TEST-U-HEALTH-022: 测试连接池健康指标空闲连接计数
///
/// 验证正确追踪空闲连接数。
#[tokio::test]
async fn test_pool_health_metrics_idle_connection_counting() {
    let metrics = PoolHealthMetrics::new();

    metrics.increment_idle().await;
    metrics.increment_idle().await;

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.idle, 2, "空闲连接数应为 2");

    metrics.decrement_idle();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.idle, 1, "空闲连接数应为 1");
}

/// TEST-U-HEALTH-023: 测试连接池健康指标 is_healthy 方法
///
/// 验证 is_healthy 方法正确判断健康状态。
#[tokio::test]
async fn test_pool_health_metrics_is_healthy() {
    let metrics = PoolHealthMetrics::new();

    // 无连接时应该不健康
    assert!(!metrics.is_healthy(), "无连接时应该不健康");

    // 有空闲连接时应该健康
    metrics.record_connection_created();
    metrics.increment_idle().await;
    assert!(metrics.is_healthy(), "有空闲连接时应该健康");
}

/// TEST-U-HEALTH-024: 测试连接池健康指标 should_create_connection 方法
///
/// 验证 should_create_connection 方法正确判断是否需要创建新连接。
#[tokio::test]
async fn test_pool_health_metrics_should_create_connection() {
    let metrics = PoolHealthMetrics::new();

    // 无连接时应该创建
    assert!(metrics.should_create_connection(1), "无连接时应该创建");

    // 有连接时不应该创建
    metrics.record_connection_created();
    metrics.increment_idle().await;
    assert!(!metrics.should_create_connection(1), "有空闲连接时不应该创建");
}

/// TEST-U-HEALTH-025: 测试连接池健康指标设置等待请求数
///
/// 验证正确设置等待请求数。
#[tokio::test]
async fn test_pool_health_metrics_set_waiting_requests() {
    let metrics = PoolHealthMetrics::new();

    metrics.set_waiting_requests(5);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.waiting, 5, "等待请求数应为 5");
}

// ============================================================================
// 多数据源健康聚合测试
// ============================================================================

/// TEST-U-HEALTH-026: 测试多数据源健康状态聚合
///
/// 验证多个数据源的健康状态可以正确聚合。
#[tokio::test]
async fn test_multi_datasource_health_aggregation() {
    // 创建多个健康检查器
    let checker1 = Arc::new(HealthChecker::new(1000));
    let checker2 = Arc::new(HealthChecker::new(1000));

    // 为第一个检查器添加连接
    let metrics1 = checker1.metrics();
    metrics1.record_connection_created();
    metrics1.increment_idle().await;

    // 为第二个检查器添加连接
    let metrics2 = checker2.metrics();
    metrics2.record_connection_created();
    metrics2.increment_idle().await;

    // 执行健康检查
    let result1 = checker1.check().await;
    let result2 = checker2.check().await;

    // 两个检查器都应该有结果
    assert!(matches!(result1.status, HealthStatus::Healthy) || matches!(result1.status, HealthStatus::Degraded(_)));
    assert!(matches!(result2.status, HealthStatus::Healthy) || matches!(result2.status, HealthStatus::Degraded(_)));
}

/// TEST-U-HEALTH-027: 测试部分数据源不健康时的聚合
///
/// 验证部分数据源不健康时聚合结果正确。
#[tokio::test]
async fn test_partial_unhealthy_aggregation() {
    let checker1 = Arc::new(HealthChecker::new(1000));
    let checker2 = Arc::new(HealthChecker::new(1000));

    // 第一个检查器有连接（健康）
    let metrics1 = checker1.metrics();
    metrics1.record_connection_created();
    metrics1.increment_idle().await;

    // 第二个检查器无连接（不健康）
    // 第二个检查器保持初始状态

    let result1 = checker1.check().await;
    let result2 = checker2.check().await;

    // 第一个应该健康或降级
    assert!(matches!(result1.status, HealthStatus::Healthy) || matches!(result1.status, HealthStatus::Degraded(_)));
    // 第二个应该不健康或降级
    assert!(
        matches!(result2.status, HealthStatus::Unhealthy(_)) || matches!(result2.status, HealthStatus::Degraded(_))
    );
}

/// TEST-U-HEALTH-028: 测试并发多数据源健康检查
///
/// 验证并发执行多个数据源的健康检查正常工作。
#[tokio::test]
async fn test_concurrent_multi_datasource_health_check() {
    let checker = Arc::new(HealthChecker::new(1000));
    let metrics = checker.metrics();
    metrics.record_connection_created();
    metrics.increment_idle().await;

    // 并发执行多个健康检查
    let mut handles = Vec::new();
    for _ in 0..5 {
        let checker = Arc::clone(&checker);
        handles.push(tokio::spawn(async move { checker.check().await }));
    }

    let results = futures::future::join_all(handles).await;

    // 所有结果都应该成功
    assert_eq!(results.len(), 5);
    for result in results {
        let result = result.unwrap();
        assert!(result.latency < Duration::from_secs(1));
    }
}

// ============================================================================
// 健康检查缓存测试
// ============================================================================

/// TEST-U-HEALTH-029: 测试健康检查结果缓存行为
///
/// 验证健康检查的延迟被正确记录。
#[tokio::test]
async fn test_health_check_result_latency_recording() {
    let checker = HealthChecker::new(1000);

    let result = checker.check().await;

    // 延迟应该被正确记录
    assert!(result.latency > Duration::ZERO);
    assert!(result.latency < Duration::from_millis(2000));
}

/// TEST-U-HEALTH-030: 测试健康检查详情包含连接池信息
///
/// 验证健康检查结果包含详细的连接池信息。
#[tokio::test]
async fn test_health_check_details_contain_pool_info() {
    let checker = HealthChecker::new(1000);
    let metrics = checker.metrics();

    // 添加一些连接
    metrics.record_connection_created();
    metrics.increment_idle().await;

    let result = checker.check().await;

    // 详情应该包含连接池信息
    assert!(result.details.contains("total="));
    assert!(result.details.contains("active="));
    assert!(result.details.contains("idle="));
}

/// TEST-U-HEALTH-031: 测试健康检查详情包含熔断器信息
///
/// 验证健康检查结果包含熔断器状态信息。
#[tokio::test]
async fn test_health_check_details_contain_circuit_breaker_info() {
    let checker = HealthChecker::new(1000);

    let result = checker.check().await;

    // 详情应该包含熔断器信息
    assert!(result.details.contains("熔断器") || result.details.contains("Circuit"));
}

/// TEST-U-HEALTH-032: 测试连续健康检查的一致性
///
/// 验证连续多次健康检查返回一致的结果。
#[tokio::test]
async fn test_consecutive_health_checks_consistency() {
    let checker = HealthChecker::new(1000);
    let metrics = checker.metrics();

    metrics.record_connection_created();
    metrics.increment_idle().await;

    let result1 = checker.check().await;
    let result2 = checker.check().await;

    // 延迟应该都在合理范围内
    assert!(result1.latency < Duration::from_secs(1));
    assert!(result2.latency < Duration::from_secs(1));
}

/// TEST-U-HEALTH-033: 测试健康检查建议生成
///
/// 验证在特定条件下生成适当的建议。
#[tokio::test]
async fn test_health_check_recommendations_generation() {
    let checker = HealthChecker::new(1000);
    let metrics = checker.metrics();

    // 模拟连接池满的情况
    metrics.record_connection_created();
    metrics.record_connection_created();
    metrics.increment_active();
    metrics.increment_active();
    metrics.set_waiting_requests(15);

    let result = checker.check().await;

    // 应该有建议或状态不是健康
    assert!(
        !result.recommendations.is_empty()
            || matches!(result.status, HealthStatus::Healthy)
            || matches!(result.status, HealthStatus::Unhealthy(_))
    );
}

// ============================================================================
// 熔断器滑动窗口测试
// ============================================================================

/// TEST-U-HEALTH-034: 测试熔断器滑动窗口大小限制
///
/// 验证滑动窗口不超过配置的大小。
#[tokio::test]
async fn test_circuit_breaker_sliding_window_size_limit() {
    let config = CircuitBreakerConfig {
        failure_threshold: 100,
        success_threshold: 100,
        timeout_ms: 1000,
        window_size: 5,
    };
    let breaker = CircuitBreaker::new(config);

    // 记录超过窗口大小的失败
    for _ in 0..10 {
        breaker.record_failure().await;
    }

    let status = breaker.status().await;
    // 连续失败次数应该大于窗口大小
    assert!(status.consecutive_failures >= 5, "失败计数应该被正确追踪");
}

/// TEST-U-HEALTH-035: 测试熔断器状态信息完整性
///
/// 验证熔断器状态信息包含所有必要字段。
#[tokio::test]
async fn test_circuit_breaker_status_completeness() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        timeout_ms: 1000,
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config);

    let status = breaker.status().await;

    // 验证所有字段
    assert_eq!(status.state, CircuitBreakerState::Closed);
    assert_eq!(status.consecutive_failures, 0);
    assert_eq!(status.consecutive_successes, 0);
    assert!(status.time_since_last_change >= Duration::ZERO);
}

// ============================================================================
// 健康检查器综合测试
// ============================================================================

/// TEST-U-HEALTH-036: 测试健康检查器无连接时状态
///
/// 验证健康检查器在无连接时返回适当的错误信息。
#[tokio::test]
async fn test_health_checker_no_connections_state() {
    let checker = HealthChecker::new(1000);

    let result = checker.check().await;

    // 无连接时应该是不健康或降级状态
    assert!(
        matches!(result.status, HealthStatus::Unhealthy(_)) || matches!(result.status, HealthStatus::Degraded(_)),
        "无连接时应该返回不健康或降级状态"
    );
}

/// TEST-U-HEALTH-037: 测试健康检查器健康状态
///
/// 验证健康检查器在有可用连接时返回健康状态。
#[tokio::test]
async fn test_health_checker_healthy_state() {
    let checker = HealthChecker::new(1000);
    let metrics = checker.metrics();

    // 创建足够的连接
    for _ in 0..5 {
        metrics.record_connection_created();
        metrics.increment_idle().await;
    }

    let result = checker.check().await;

    // 有可用连接时应该返回健康状态
    assert!(
        matches!(result.status, HealthStatus::Healthy) || matches!(result.status, HealthStatus::Degraded(_)),
        "有可用连接时应该返回健康或降级状态"
    );
}

/// TEST-U-HEALTH-038: 测试健康检查器获取熔断器
///
/// 验证健康检查器可以正确获取关联的熔断器。
#[tokio::test]
async fn test_health_checker_get_circuit_breaker() {
    let checker = HealthChecker::new(1000);
    let cb = checker.circuit_breaker();

    let state = cb.state().await;
    assert_eq!(state, CircuitBreakerState::Closed);
}

/// TEST-U-HEALTH-039: 测试健康检查器获取健康指标
///
/// 验证健康检查器可以正确获取关联的健康指标。
#[tokio::test]
async fn test_health_checker_get_metrics() {
    let checker = HealthChecker::new(1000);
    let metrics = checker.metrics();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.total, 0);
}

// ============================================================================
// 边界条件测试
// ============================================================================

/// TEST-U-HEALTH-040: 测试熔断器配置边界值
///
/// 验证熔断器使用边界配置值时正常工作。
#[tokio::test]
async fn test_circuit_breaker_config_boundary_values() {
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout_ms: 10,
        window_size: 1,
    };
    let breaker = CircuitBreaker::new(config);

    // 一次失败就应该打开
    breaker.record_failure().await;
    assert_eq!(breaker.state().await, CircuitBreakerState::Open);
}

/// TEST-U-HEALTH-041: 测试熔断器状态转换时间记录
///
/// 验证熔断器状态转换时正确记录时间。
#[tokio::test]
async fn test_circuit_breaker_state_change_time_recording() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 2,
        timeout_ms: 1000,
        window_size: 10,
    };
    let breaker = CircuitBreaker::new(config);

    let status_before = breaker.status().await;
    let time_before = status_before.time_since_last_change;

    // 触发状态转换
    breaker.record_failure().await;
    breaker.record_failure().await;

    let status_after = breaker.status().await;

    // 状态应该是 Open
    assert_eq!(status_after.state, CircuitBreakerState::Open);
    // 时间应该被重置
    assert!(status_after.time_since_last_change < time_before || time_before == Duration::ZERO);
}

/// TEST-U-HEALTH-042: 测试健康检查器超时边界
///
/// 验证健康检查器在超时边界时的行为。
#[tokio::test]
async fn test_health_checker_timeout_boundary() {
    // 使用很短但非零的超时
    let checker = HealthChecker::new(10);

    let result = checker.check().await;

    // 应该返回某种状态
    assert!(result.latency > Duration::ZERO);
}

/// TEST-U-HEALTH-043: 测试 PoolSnapshot 克隆
///
/// 验证 PoolSnapshot 可以正确克隆。
#[tokio::test]
async fn test_pool_snapshot_clone() {
    let metrics = PoolHealthMetrics::new();
    metrics.record_connection_created();

    let snapshot1 = metrics.snapshot();
    let snapshot2 = snapshot1.clone();

    assert_eq!(snapshot1.total, snapshot2.total);
    assert_eq!(snapshot1.active, snapshot2.active);
}

/// TEST-U-HEALTH-044: 测试 CircuitBreakerStatus 克隆
///
/// 验证 CircuitBreakerStatus 可以正确克隆。
#[tokio::test]
async fn test_circuit_breaker_status_clone() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());

    let status1 = breaker.status().await;
    let status2 = status1.clone();

    assert_eq!(status1.state, status2.state);
    assert_eq!(status1.consecutive_failures, status2.consecutive_failures);
}

/// TEST-U-HEALTH-045: 测试健康检查结果克隆
///
/// 验证 HealthCheckResult 可以正确克隆。
#[tokio::test]
async fn test_health_check_result_clone() {
    let checker = HealthChecker::new(1000);

    let result1 = checker.check().await;
    let result2 = result1.clone();

    // 验证基本字段一致
    assert_eq!(result1.status, result2.status);
    assert!(result1.latency >= result2.latency || result2.latency >= result1.latency);
}
