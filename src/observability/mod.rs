// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Observability 模块
//!
//! 提供健康检查、性能指标等可观测性功能

// 单文件模块
pub mod health;
pub mod metrics;

// Re-exports
#[cfg(feature = "health-check")]
pub use health::{CircuitBreaker, HealthChecker, HealthStatus, PoolHealthMetrics};
#[cfg(feature = "metrics")]
pub use metrics::{LatencyHistogram, LatencyPercentiles, MetricsCollector, MetricsCollectorTrait};
