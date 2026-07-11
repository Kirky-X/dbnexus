// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 健康检查模块集成测试
//!
//! 测试连接池健康检查、熔断器模式、性能指标收集和自动恢复机制

#[cfg(feature = "health-check")]
mod health_tests {
    use dbnexus::DbPool;
    use dbnexus::{
        CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState, HealthChecker, HealthStatus, PoolHealthMetrics,
    };
    use std::sync::Arc;
    use std::time::Duration;

    fn get_database_url() -> Option<String> {
        if let Ok(url) = std::env::var("DATABASE_URL") {
            return Some(url);
        }

        if cfg!(feature = "sqlite") {
            return Some("sqlite::memory:".to_string());
        }

        None
    }

    // ============================================================================
    // 熔断器集成测试
    // ============================================================================

    #[tokio::test]
    async fn test_circuit_breaker_state_transitions() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout_ms: 50,
            window_size: 10,
        };
        let breaker = CircuitBreaker::new(config);

        assert_eq!(breaker.state().await, CircuitBreakerState::Closed);

        breaker.record_failure().await;
        breaker.record_failure().await;
        assert_eq!(breaker.state().await, CircuitBreakerState::Closed);

        breaker.record_failure().await;
        assert_eq!(breaker.state().await, CircuitBreakerState::Open);

        let result = breaker.can_execute().await;
        assert!(result.is_err());

        tokio::time::sleep(Duration::from_millis(60)).await;

        let result = breaker.can_execute().await;
        assert!(result.is_ok());
        assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);

        breaker.record_success().await;
        assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);

        breaker.record_success().await;
        assert_eq!(breaker.state().await, CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_failure_rate_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout_ms: 10,
            window_size: 10,
        };
        let breaker = CircuitBreaker::new(config);

        breaker.record_failure().await;
        breaker.record_failure().await;
        assert_eq!(breaker.state().await, CircuitBreakerState::Open);

        tokio::time::sleep(Duration::from_millis(20)).await;
        let result = breaker.can_execute().await;
        assert!(result.is_ok());
        assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);

        breaker.record_failure().await;
        assert_eq!(breaker.state().await, CircuitBreakerState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_status() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_ms: 1000,
            window_size: 10,
        };
        let breaker = CircuitBreaker::new(config);

        let status = breaker.status().await;
        assert_eq!(status.state, CircuitBreakerState::Closed);
        assert_eq!(status.consecutive_failures, 0);
        assert_eq!(status.consecutive_successes, 0);

        breaker.record_failure().await;
        breaker.record_failure().await;

        let status = breaker.status().await;
        assert_eq!(status.consecutive_failures, 2);
        assert!(status.time_since_last_change > Duration::ZERO);
    }

    // ============================================================================
    // 健康指标集成测试
    // ============================================================================

    #[tokio::test]
    async fn test_pool_health_metrics_operations() {
        let metrics = PoolHealthMetrics::new();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total, 0);
        assert_eq!(snapshot.active, 0);
        assert_eq!(snapshot.idle, 0);

        metrics.record_connection_created();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total, 1);
        assert_eq!(snapshot.active, 1);

        metrics.increment_active();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.active, 2);

        metrics.decrement_active();
        metrics.increment_idle().await;
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.active, 1);
        assert_eq!(snapshot.idle, 1);

        metrics.record_connection_failed();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.failed, 1);

        metrics.record_connection_closed();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total, 0);

        metrics.set_waiting_requests(5);
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.waiting, 5);
    }

    #[tokio::test]
    async fn test_pool_health_metrics_is_healthy() {
        let metrics = PoolHealthMetrics::new();
        assert!(!metrics.is_healthy());

        metrics.record_connection_created();
        assert!(!metrics.is_healthy());

        metrics.increment_idle().await;
        assert!(metrics.is_healthy());
    }

    #[tokio::test]
    async fn test_pool_health_metrics_should_create_connection() {
        let metrics = PoolHealthMetrics::new();
        assert!(metrics.should_create_connection(1));

        metrics.record_connection_created();
        assert!(metrics.should_create_connection(1));

        metrics.increment_idle().await;
        assert!(!metrics.should_create_connection(1));
    }

    // ============================================================================
    // 健康检查器集成测试
    // ============================================================================

    #[tokio::test]
    async fn test_health_checker_initial_state() {
        let checker = HealthChecker::new(1000);
        let result = checker.check().await;
        assert!(matches!(result.status, HealthStatus::Unhealthy(_)));
        assert!(result.latency < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_health_checker_timeout() {
        let checker = HealthChecker::new(1);
        let result = checker.check().await;
        assert!(
            matches!(result.status, HealthStatus::Degraded(_)) || matches!(result.status, HealthStatus::Unhealthy(_))
        );
        assert!(result.latency > Duration::ZERO);
    }

    // ============================================================================
    // 数据库连接健康测试
    // ============================================================================

    #[tokio::test]
    async fn test_pool_status_health() {
        let Some(url) = get_database_url() else {
            return;
        };

        let pool = DbPool::new(&url).await.unwrap();
        let status = pool.status();
        let _ = status.total;
        let _ = status.active;
    }

    // ============================================================================
    // 综合健康场景测试
    // ============================================================================

    #[tokio::test]
    async fn test_health_metrics_circuit_breaker_integration() {
        let metrics = PoolHealthMetrics::new();
        let _breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 1,
            timeout_ms: 50,
            window_size: 10,
        });

        for _ in 0..15 {
            metrics.record_connection_created();
            metrics.record_connection_failed();
        }

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.created, 15);
        assert_eq!(snapshot.failed, 15);

        let checker = HealthChecker::new(1000);
        let result = checker.check().await;
        assert!(matches!(result.status, HealthStatus::Unhealthy(_)));
    }

    #[tokio::test]
    async fn test_health_status_display() {
        let healthy = HealthStatus::Healthy;
        let unhealthy = HealthStatus::Unhealthy("Connection refused".to_string());
        let degraded = HealthStatus::Degraded("High latency".to_string());

        assert!(!format!("{healthy:?}").is_empty());
        assert!(!format!("{unhealthy:?}").is_empty());
        assert!(!format!("{degraded:?}").is_empty());
    }

    #[tokio::test]
    async fn test_circuit_breaker_display() {
        assert_eq!(format!("{}", CircuitBreakerState::Closed), "closed");
        assert_eq!(format!("{}", CircuitBreakerState::HalfOpen), "half-open");
        assert_eq!(format!("{}", CircuitBreakerState::Open), "open");
    }

    // ============================================================================
    // 并发健康检查测试
    // ============================================================================

    #[tokio::test]
    async fn test_concurrent_health_checks() {
        let checker = Arc::new(HealthChecker::new(1000));
        let metrics = checker.metrics();
        metrics.record_connection_created();
        metrics.increment_idle().await;

        let mut handles = Vec::new();
        for _ in 0..5 {
            let checker = Arc::clone(&checker);
            handles.push(tokio::spawn(async move { checker.check().await }));
        }

        let results = futures::future::join_all(handles).await;
        assert_eq!(results.len(), 5);
        for result in results {
            let result = result.unwrap();
            assert!(result.latency < Duration::from_secs(1));
        }
    }

    #[tokio::test]
    async fn test_concurrent_circuit_breaker_operations() {
        let breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 10,
            success_threshold: 5,
            timeout_ms: 100,
            window_size: 100,
        }));

        let mut handles = Vec::new();
        for i in 0..10 {
            let breaker = Arc::clone(&breaker);
            handles.push(tokio::spawn(async move {
                if i % 2 == 0 {
                    breaker.record_failure().await;
                } else {
                    breaker.record_success().await;
                }
            }));
        }

        futures::future::join_all(handles).await;

        let state = breaker.state().await;
        assert!(matches!(state, CircuitBreakerState::Closed));
    }

    // ============================================================================
    // 熔断器配置测试
    // ============================================================================

    #[tokio::test]
    async fn test_circuit_breaker_config_defaults() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.timeout_ms, 30000);
    }

    #[tokio::test]
    async fn test_circuit_breaker_custom_config() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 1,
            timeout_ms: 100,
            window_size: 5,
        };
        let breaker = CircuitBreaker::new(config);

        breaker.record_failure().await;
        breaker.record_failure().await;
        assert_eq!(breaker.state().await, CircuitBreakerState::Open);

        tokio::time::sleep(Duration::from_millis(150)).await;

        let result = breaker.can_execute().await;
        assert!(result.is_ok());
        assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);
    }

    // ============================================================================
    // 滑动窗口测试
    // ============================================================================

    #[tokio::test]
    async fn test_failure_window_size_limit() {
        let config = CircuitBreakerConfig {
            failure_threshold: 100,
            success_threshold: 100,
            timeout_ms: 1000,
            window_size: 5,
        };
        let breaker = CircuitBreaker::new(config);

        for _ in 0..10 {
            breaker.record_failure().await;
        }

        let status = breaker.status().await;
        assert!(status.consecutive_failures >= 5);
    }

    // ============================================================================
    // 压力测试场景
    // ============================================================================

    #[tokio::test]
    async fn test_health_metrics_stress() {
        let metrics = PoolHealthMetrics::new();

        for _ in 0..100 {
            metrics.record_connection_created();
            metrics.increment_active();
            metrics.decrement_active();
            metrics.increment_idle().await;
            metrics.decrement_idle();
            metrics.record_connection_closed();
        }

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total, 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_stress() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1000,
            success_threshold: 1000,
            timeout_ms: 1000,
            window_size: 1000,
        });

        for _ in 0..1000 {
            breaker.record_success().await;
        }

        let state = breaker.state().await;
        assert!(matches!(state, CircuitBreakerState::Closed));

        let status = breaker.status().await;
        assert_eq!(status.consecutive_failures, 0);
        assert!(status.consecutive_successes > 0);
    }
}
