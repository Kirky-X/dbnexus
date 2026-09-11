// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Observability 模块
//!
//! 提供健康检查、性能指标等可观测性功能

// 单文件模块
#[cfg(feature = "health-check")]
pub mod health;
#[cfg(feature = "metrics")]
pub mod metrics;
// T412：OTel 导出桥（慢查询/池指标 → OTLP/HTTP + stdout fallback）
#[cfg(feature = "otel")]
pub mod otel;

// Re-exports
#[cfg(feature = "health-check")]
pub use health::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerState, HealthChecker,
    HealthStatus, PoolHealthMetrics,
};
#[cfg(all(feature = "metrics", any(test, feature = "test-utils")))]
pub use metrics::MockMetrics;
#[cfg(feature = "metrics")]
pub use metrics::{
    ConnectionAcquireStats, HistogramBucket, HistogramStats, LatencyHistogram, LatencyPercentiles,
    MetricsCollector, MetricsCollectorTrait, MetricsError, PoolMetrics, QueryStats,
    SlowQueryConfig, SlowQueryRecord, ThroughputStats, TransactionStats,
};
#[cfg(feature = "otel")]
pub use otel::{HttpTransport, OtelConfig, OtelExporter, OtelMetricEvent, OtlpTransport};
