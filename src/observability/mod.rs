// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Observability 模块
//!
//! 提供健康检查、性能指标等可观测性功能

// 单文件模块
pub mod health;
#[cfg(feature = "metrics")]
pub mod metrics;
#[cfg(feature = "tracing")]
pub mod tracing;

// Re-exports
#[cfg(feature = "health-check")]
pub use health::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerState, HealthChecker, HealthStatus,
    PoolHealthMetrics,
};
#[cfg(all(feature = "metrics", any(test, feature = "test-utils")))]
pub use metrics::MockMetrics;
#[cfg(feature = "metrics")]
pub use metrics::{
    ConnectionAcquireStats, HistogramBucket, HistogramStats, LatencyHistogram, LatencyPercentiles, MetricsCollector,
    MetricsCollectorTrait, MetricsError, PoolMetrics, QueryStats, SlowQueryConfig, SlowQueryRecord, ThroughputStats,
    TransactionStats,
};
#[cfg(feature = "tracing")]
pub use tracing::{TracingError, TracingGuard};
