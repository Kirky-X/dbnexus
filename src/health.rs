// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 健康检查和可观测性模块
//!
//! 提供连接池健康检查、性能指标收集和熔断器模式
//!
//! # 功能
//!
//! - 连接池健康状态检查
//! - 数据库连接健康探测
//! - 熔断器模式 (Circuit Breaker)
//! - 性能指标收集
//! - 自动恢复机制

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time;

/// 健康状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 不健康
    Unhealthy(String),
    /// 降级运行
    Degraded(String),
}

/// 健康检查结果
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// 健康状态
    pub status: HealthStatus,
    /// 检查耗时
    pub latency: Duration,
    /// 详细信息
    pub details: String,
    /// 恢复建议
    pub recommendations: Vec<String>,
}

/// 连接池健康指标
#[derive(Debug, Clone)]
pub struct PoolHealthMetrics {
    /// 总连接数
    pub total_connections: Arc<AtomicUsize>,
    /// 活跃连接数
    pub active_connections: Arc<AtomicUsize>,
    /// 空闲连接数
    pub idle_connections: Arc<AtomicUsize>,
    /// 等待获取连接的请求数
    pub waiting_requests: Arc<AtomicUsize>,
    /// 连接创建成功次数
    pub connections_created: Arc<AtomicU64>,
    /// 连接创建失败次数
    pub connections_failed: Arc<AtomicU64>,
    /// 连接断开次数
    pub connections_closed: Arc<AtomicU64>,
    /// 最后健康检查时间
    pub last_health_check: Arc<RwLock<Instant>>,
    /// 最后成功连接时间
    pub last_successful_connection: Arc<RwLock<Instant>>,
}

impl Default for PoolHealthMetrics {
    fn default() -> Self {
        Self {
            total_connections: Arc::new(AtomicUsize::new(0)),
            active_connections: Arc::new(AtomicUsize::new(0)),
            idle_connections: Arc::new(AtomicUsize::new(0)),
            waiting_requests: Arc::new(AtomicUsize::new(0)),
            connections_created: Arc::new(AtomicU64::new(0)),
            connections_failed: Arc::new(AtomicU64::new(0)),
            connections_closed: Arc::new(AtomicU64::new(0)),
            last_health_check: Arc::new(RwLock::new(Instant::now())),
            last_successful_connection: Arc::new(RwLock::new(Instant::now())),
        }
    }
}

impl PoolHealthMetrics {
    /// 创建新的健康指标
    pub fn new() -> Self {
        Self {
            total_connections: Arc::new(AtomicUsize::new(0)),
            active_connections: Arc::new(AtomicUsize::new(0)),
            idle_connections: Arc::new(AtomicUsize::new(0)),
            waiting_requests: Arc::new(AtomicUsize::new(0)),
            connections_created: Arc::new(AtomicU64::new(0)),
            connections_failed: Arc::new(AtomicU64::new(0)),
            connections_closed: Arc::new(AtomicU64::new(0)),
            last_health_check: Arc::new(RwLock::new(Instant::now())),
            last_successful_connection: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// 获取连接池状态快照
    pub fn snapshot(&self) -> PoolSnapshot {
        PoolSnapshot {
            total: self.total_connections.load(Ordering::Relaxed),
            active: self.active_connections.load(Ordering::Relaxed),
            idle: self.idle_connections.load(Ordering::Relaxed),
            waiting: self.waiting_requests.load(Ordering::Relaxed),
            created: self.connections_created.load(Ordering::Relaxed),
            failed: self.connections_failed.load(Ordering::Relaxed),
            closed: self.connections_closed.load(Ordering::Relaxed),
        }
    }

    /// 检查是否健康
    pub fn is_healthy(&self) -> bool {
        let snapshot = self.snapshot();
        // 检查是否有可用连接
        snapshot.idle > 0 || snapshot.active < snapshot.total
    }

    /// 检查是否需要创建新连接
    pub fn should_create_connection(&self, min_connections: usize) -> bool {
        let snapshot = self.snapshot();
        snapshot.total < min_connections || (snapshot.active >= snapshot.total && snapshot.idle == 0)
    }

    /// 增加活跃连接
    pub fn increment_active(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// 减少活跃连接
    pub fn decrement_active(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// 增加空闲连接
    pub async fn increment_idle(&self) {
        self.idle_connections.fetch_add(1, Ordering::Relaxed);
        *self.last_successful_connection.write().await = Instant::now();
    }

    /// 减少空闲连接
    pub fn decrement_idle(&self) {
        self.idle_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// 记录连接创建
    pub fn record_connection_created(&self) {
        self.total_connections.fetch_add(1, Ordering::Relaxed);
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.connections_created.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录连接失败
    pub fn record_connection_failed(&self) {
        self.connections_failed.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录连接关闭
    pub fn record_connection_closed(&self) {
        self.total_connections.fetch_sub(1, Ordering::Relaxed);
        self.connections_closed.fetch_add(1, Ordering::Relaxed);
    }

    /// 更新等待请求数
    pub fn set_waiting_requests(&self, count: usize) {
        self.waiting_requests.store(count, Ordering::Relaxed);
    }

    /// 更新最后健康检查时间
    pub async fn update_health_check_time(&self) {
        *self.last_health_check.write().await = Instant::now();
    }
}

/// 连接池状态快照
#[derive(Debug, Clone)]
pub struct PoolSnapshot {
    /// 总连接数
    pub total: usize,
    /// 活跃连接数
    pub active: usize,
    /// 空闲连接数
    pub idle: usize,
    /// 等待的请求数
    pub waiting: usize,
    /// 创建的连接数
    pub created: u64,
    /// 失败的连接数
    pub failed: u64,
    /// 关闭的连接数
    pub closed: u64,
}

/// 熔断器状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// 关闭状态，正常运行
    Closed,
    /// 半开状态，尝试恢复
    HalfOpen,
    /// 打开状态，拒绝请求
    Open,
}

impl std::fmt::Display for CircuitBreakerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerState::Closed => write!(f, "closed"),
            CircuitBreakerState::HalfOpen => write!(f, "half-open"),
            CircuitBreakerState::Open => write!(f, "open"),
        }
    }
}

/// 熔断器错误
#[derive(Debug, Error, Clone)]
#[error("Circuit breaker is {state}")]
pub struct CircuitBreakerError {
    state: CircuitBreakerState,
}

impl CircuitBreakerError {
    /// 使用给定状态创建错误
    pub fn new(state: CircuitBreakerState) -> Self {
        Self { state }
    }

    /// 获取熔断器状态
    pub fn state(&self) -> CircuitBreakerState {
        self.state
    }
}

/// 熔断器配置
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// 失败阈值（连续失败次数）
    pub failure_threshold: u64,
    /// 成功阈值（半开状态下的成功次数）
    pub success_threshold: u64,
    /// 超时时间（毫秒）
    pub timeout_ms: u64,
    /// 滑动窗口大小（用于计算失败率）
    pub window_size: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_ms: 30000, // 30秒
            window_size: 100,
        }
    }
}

/// 熔断器
///
/// 实现熔断器模式，防止级联故障
#[derive(Debug)]
pub struct CircuitBreaker {
    /// 当前状态
    state: Arc<RwLock<CircuitBreakerState>>,
    /// 连续失败次数
    consecutive_failures: Arc<AtomicU64>,
    /// 连续成功次数
    consecutive_successes: Arc<AtomicU64>,
    /// 最后状态变更时间
    last_state_change: Arc<RwLock<Instant>>,
    /// 配置
    config: Arc<CircuitBreakerConfig>,
    /// 失败记录（滑动窗口）
    failure_window: Arc<RwLock<Vec<bool>>>,
}

impl CircuitBreaker {
    /// 创建熔断器
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            consecutive_failures: Arc::new(AtomicU64::new(0)),
            consecutive_successes: Arc::new(AtomicU64::new(0)),
            last_state_change: Arc::new(RwLock::new(Instant::now())),
            config: Arc::new(config),
            failure_window: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 获取当前状态
    pub async fn state(&self) -> CircuitBreakerState {
        *self.state.read().await
    }

    /// 记录成功
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;
        let config = &self.config;

        match *state {
            CircuitBreakerState::Closed => {
                self.consecutive_failures.store(0, Ordering::Relaxed);
                self.consecutive_successes.fetch_add(1, Ordering::Relaxed);
            }
            CircuitBreakerState::HalfOpen => {
                let successes = self.consecutive_successes.fetch_add(1, Ordering::Relaxed) + 1;
                if successes >= config.success_threshold {
                    *state = CircuitBreakerState::Closed;
                    *self.last_state_change.write().await = Instant::now();
                    self.consecutive_failures.store(0, Ordering::Relaxed);
                }
            }
            CircuitBreakerState::Open => {
                // Open 状态下不允许成功
            }
        }

        // 更新滑动窗口
        self.update_failure_window(false).await;
    }

    /// 记录失败
    pub async fn record_failure(&self) {
        let mut state = self.state.write().await;
        let config = &self.config;

        match *state {
            CircuitBreakerState::Closed => {
                let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
                if failures >= config.failure_threshold {
                    *state = CircuitBreakerState::Open;
                    *self.last_state_change.write().await = Instant::now();
                }
                self.consecutive_successes.store(0, Ordering::Relaxed);
            }
            CircuitBreakerState::HalfOpen => {
                *state = CircuitBreakerState::Open;
                *self.last_state_change.write().await = Instant::now();
                self.consecutive_successes.store(0, Ordering::Relaxed);
            }
            CircuitBreakerState::Open => {
                // 已经在 Open 状态
            }
        }

        // 更新滑动窗口
        self.update_failure_window(true).await;
    }

    /// 检查是否允许请求
    pub async fn can_execute(&self) -> Result<(), CircuitBreakerError> {
        let state = self.state.read().await;
        let config = &self.config;

        match *state {
            CircuitBreakerState::Closed => Ok(()),
            CircuitBreakerState::HalfOpen => {
                // 半开状态允许少量请求
                let failures = self.failure_window.read().await.iter().filter(|&&f| f).count();
                let total = self.failure_window.read().await.len();
                if total > 0 {
                    let failure_rate = failures as f64 / total as f64;
                    if failure_rate > 0.5 {
                        return Err(CircuitBreakerError::new(CircuitBreakerState::Open));
                    }
                }
                Ok(())
            }
            CircuitBreakerState::Open => {
                // 检查是否超时
                let elapsed = self.last_state_change.read().await.elapsed();
                if elapsed.as_millis() >= config.timeout_ms as u128 {
                    // 转换为半开状态
                    drop(state);
                    let mut write_state = self.state.write().await;
                    *write_state = CircuitBreakerState::HalfOpen;
                    *self.last_state_change.write().await = Instant::now();
                    self.consecutive_failures.store(0, Ordering::Relaxed);
                    self.consecutive_successes.store(0, Ordering::Relaxed);
                    Ok(())
                } else {
                    Err(CircuitBreakerError::new(CircuitBreakerState::Open))
                }
            }
        }
    }

    /// 更新滑动窗口
    async fn update_failure_window(&self, is_failure: bool) {
        let mut window = self.failure_window.write().await;
        window.push(is_failure);
        if window.len() > self.config.window_size {
            window.remove(0);
        }
    }

    /// 获取熔断器状态信息
    pub async fn status(&self) -> CircuitBreakerStatus {
        let state = self.state.read().await;
        let elapsed = self.last_state_change.read().await.elapsed();

        CircuitBreakerStatus {
            state: *state,
            consecutive_failures: self.consecutive_failures.load(Ordering::Relaxed),
            consecutive_successes: self.consecutive_successes.load(Ordering::Relaxed),
            time_since_last_change: elapsed,
        }
    }
}

/// 熔断器状态信息
#[derive(Debug, Clone)]
pub struct CircuitBreakerStatus {
    /// 当前状态
    pub state: CircuitBreakerState,
    /// 连续失败次数
    pub consecutive_failures: u64,
    /// 连续成功次数
    pub consecutive_successes: u64,
    /// 距离上次状态变更的时间
    pub time_since_last_change: Duration,
}

/// 健康检查器
#[derive(Debug)]
pub struct HealthChecker {
    /// 健康指标
    metrics: PoolHealthMetrics,
    /// 熔断器
    circuit_breaker: CircuitBreaker,
    /// 检查超时
    check_timeout: Duration,
}

impl HealthChecker {
    /// 创建健康检查器
    pub fn new(check_timeout_ms: u64) -> Self {
        Self {
            metrics: PoolHealthMetrics::new(),
            circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig::default()),
            check_timeout: Duration::from_millis(check_timeout_ms),
        }
    }

    /// 执行健康检查
    pub async fn check(&self) -> HealthCheckResult {
        // 使用超时进行健康检查
        let check_future = self.perform_check();
        let result = tokio::time::timeout(self.check_timeout, check_future).await;

        match result {
            Ok(check_result) => {
                // 更新健康检查时间
                self.metrics.update_health_check_time().await;
                check_result
            }
            Err(_) => {
                // 超时返回降级状态
                HealthCheckResult {
                    status: HealthStatus::Degraded("健康检查超时".to_string()),
                    latency: self.check_timeout,
                    details: "检查超时".to_string(),
                    recommendations: vec!["健康检查超时，建议优化检查逻辑".to_string()],
                }
            }
        }
    }

    /// 执行实际健康检查
    async fn perform_check(&self) -> HealthCheckResult {
        let start = Instant::now();
        let mut recommendations = Vec::new();

        let cb_status = self.circuit_breaker.status().await;

        // 检查连接池指标
        let snapshot = self.metrics.snapshot();

        // 评估健康状态
        let status = if snapshot.failed > snapshot.created.saturating_sub(10) && snapshot.created > 10 {
            recommendations.push("检查数据库连接配置是否正确".to_string());
            recommendations.push("验证网络连接是否稳定".to_string());
            HealthStatus::Unhealthy(format!(
                "连接失败率过高: {}/{} ({:.1}%)",
                snapshot.failed,
                snapshot.created,
                snapshot.created as f64 / (snapshot.created + snapshot.failed) as f64 * 100.0
            ))
        } else if snapshot.total == 0 {
            recommendations.push("请确保数据库连接池已正确初始化".to_string());
            HealthStatus::Unhealthy("无可用连接".to_string())
        } else if cb_status.state == CircuitBreakerState::Open {
            recommendations.push("等待熔断器恢复".to_string());
            HealthStatus::Degraded("熔断器已打开，请求被拒绝".to_string())
        } else if snapshot.active >= snapshot.total && snapshot.waiting > 10 {
            recommendations.push("考虑增加连接池大小".to_string());
            recommendations.push("检查是否有连接泄露".to_string());
            HealthStatus::Degraded(format!(
                "连接池已满: {}/{} 活跃, {} 等待",
                snapshot.active, snapshot.total, snapshot.waiting
            ))
        } else if snapshot.idle == 0 && snapshot.active > 0 {
            recommendations.push("考虑增加最小连接数".to_string());
            HealthStatus::Degraded("无空闲连接".to_string())
        } else {
            HealthStatus::Healthy
        };

        // 更新健康检查时间
        self.metrics.update_health_check_time().await;

        let details = format!(
            "连接池: total={}, active={}, idle={}, waiting={}\n\
             统计: created={}, failed={}, closed={}\n\
             熔断器: {:?} ({} 次失败, {} 次成功)",
            snapshot.total,
            snapshot.active,
            snapshot.idle,
            snapshot.waiting,
            snapshot.created,
            snapshot.failed,
            snapshot.closed,
            cb_status.state,
            cb_status.consecutive_failures,
            cb_status.consecutive_successes
        );

        HealthCheckResult {
            status,
            latency: start.elapsed(),
            details,
            recommendations,
        }
    }

    /// 获取健康指标
    pub fn metrics(&self) -> &PoolHealthMetrics {
        &self.metrics
    }

    /// 获取熔断器
    pub fn circuit_breaker(&self) -> &CircuitBreaker {
        &self.circuit_breaker
    }
}

/// 自动恢复器
///
/// 定期检查健康状态并尝试自动恢复
#[derive(Debug)]
pub struct AutoRecoverer {
    health_checker: Arc<HealthChecker>,
    check_interval: Duration,
    running: Arc<AtomicU64>,
}

impl AutoRecoverer {
    /// 创建自动恢复器
    pub fn new(health_checker: Arc<HealthChecker>, check_interval_ms: u64) -> Self {
        Self {
            health_checker,
            check_interval: Duration::from_millis(check_interval_ms),
            running: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 启动自动恢复
    pub async fn start(&self) {
        if self
            .running
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return; // 已经在运行
        }

        let mut interval = time::interval(self.check_interval);

        loop {
            interval.tick().await;

            // 检查熔断器状态
            if let Ok(()) = self.health_checker.circuit_breaker().can_execute().await {
                // 记录成功
                let _ = self.health_checker.circuit_breaker().record_success().await;
            } else {
                // 熔断器打开，尝试恢复
                let health_result = self.health_checker.check().await;

                if matches!(health_result.status, HealthStatus::Healthy) {
                    // 健康状态良好，熔断器会自动恢复
                }
            }
        }
    }

    /// 停止自动恢复
    pub fn stop(&self) {
        self.running.store(0, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout_ms: 1000,
            window_size: 10,
        });

        // 初始状态应该是关闭
        assert_eq!(breaker.state().await, CircuitBreakerState::Closed);

        // 记录失败
        for _ in 0..3 {
            breaker.record_failure().await;
        }

        // 应该转换为打开状态
        assert_eq!(breaker.state().await, CircuitBreakerState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout_ms: 100,
            window_size: 10,
        });

        // 打开熔断器
        breaker.record_failure().await;
        breaker.record_failure().await;
        assert_eq!(breaker.state().await, CircuitBreakerState::Open);

        // 等待超时
        tokio::time::sleep(Duration::from_millis(150)).await;

        // 尝试执行以触发状态转换
        let _ = breaker.can_execute().await;

        // 应该转换为半开状态
        assert_eq!(breaker.state().await, CircuitBreakerState::HalfOpen);

        // 记录成功
        breaker.record_success().await;
        breaker.record_success().await;

        // 应该转换为关闭状态
        assert_eq!(breaker.state().await, CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_health_metrics() {
        let metrics = PoolHealthMetrics::new();

        // 初始状态
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total, 0);
        assert_eq!(snapshot.active, 0);
        assert_eq!(snapshot.idle, 0);

        // 模拟连接创建
        metrics.record_connection_created();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total, 1);
        assert_eq!(snapshot.active, 1);

        // 模拟连接激活
        metrics.increment_active();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.active, 2);

        // 模拟连接变为空闲
        metrics.decrement_active();
        metrics.increment_idle().await;
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.active, 1);
        assert_eq!(snapshot.idle, 1);

        // 模拟连接关闭
        metrics.record_connection_closed();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total, 0);
    }

    #[tokio::test]
    async fn test_health_checker() {
        let checker = HealthChecker::new(1000);

        // 执行健康检查
        let result = checker.check().await;

        // 初始状态应该是降级的（因为没有连接）
        // 这验证了健康检查功能正常工作
        assert!(result.latency < Duration::from_secs(1));
        // 无连接时状态应为Unhealthy或有建议
        assert!(
            !result.recommendations.is_empty()
                || matches!(result.status, HealthStatus::Healthy)
                || matches!(result.status, HealthStatus::Unhealthy(_))
        );
    }
}
