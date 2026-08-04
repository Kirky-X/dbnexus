// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 性能指标收集模块
//!
//! 提供全面的性能指标收集功能，包括：
//! - **延迟指标**: P50、P90、P95、P99 延迟百分位
//! - **吞吐量指标**: 查询/秒、事务/秒
//! - **延迟分布**: 直方图统计
//! - **连接指标**: 连接获取延迟、连接池使用率
//! - **事务指标**: 事务持续时间、事务成功率

use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;

// ============================================================================
// MetricsCollector Trait Interface
// ============================================================================

/// 指标收集器错误类型
#[derive(Debug, Error)]
pub enum MetricsError {
    /// 导出失败
    #[error("Export failed: {0}")]
    ExportError(String),

    /// 收集器未初始化
    #[error("Collector not initialized")]
    NotInitialized,

    /// 未知错误
    #[error("Unknown metrics error: {0}")]
    Unknown(String),
}

impl crate::i18n::error_ext::LocalizedMsg for MetricsError {
    fn message_key(&self) -> &'static str {
        match self {
            Self::ExportError(_) => "metrics-export-error",
            Self::NotInitialized => "metrics-not-initialized",
            Self::Unknown(_) => "metrics-unknown",
        }
    }

    fn message_args(&self) -> Vec<(&str, String)> {
        match self {
            Self::ExportError(reason) => vec![("reason", reason.clone())],
            Self::NotInitialized => vec![],
            Self::Unknown(reason) => vec![("reason", reason.clone())],
        }
    }
}

/// 指标收集器 trait 接口
///
/// 定义性能指标收集的通用接口，便于测试和替换实现。
/// 所有实现必须支持 `Send + Sync` 以便在多线程环境中使用。
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
/// use dbnexus::metrics::MetricsCollectorTrait;
///
/// // 使用 trait 对象进行动态分发
/// let collector: Arc<dyn MetricsCollectorTrait> = Arc::new(MetricsCollector::new());
///
/// // 或者在测试中使用 mock 实现
/// struct MockMetrics;
/// impl MetricsCollectorTrait for MockMetrics {
///     fn record_query(&self, duration: Duration) {}
///     fn record_connection(&self, duration: Duration) {}
/// }
/// ```
pub trait MetricsCollectorTrait: Send + Sync {
    /// 记录查询延迟
    ///
    /// # Arguments
    ///
    /// * `duration` - 查询执行耗时
    fn record_query(&self, duration: Duration);

    /// 记录连接获取延迟
    ///
    /// # Arguments
    ///
    /// * `duration` - 连接获取耗时
    fn record_connection(&self, duration: Duration);

    /// 记录事务执行时间
    ///
    /// # Arguments
    ///
    /// * `duration` - 事务执行耗时
    /// * `success` - 事务是否成功提交
    fn record_transaction(&self, duration: Duration, success: bool);

    /// 记录连接池使用情况
    ///
    /// # Arguments
    ///
    /// * `total` - 总连接数
    /// * `active` - 活跃连接数
    /// * `idle` - 空闲连接数
    fn record_pool_usage(&self, total: u32, active: u32, idle: u32);

    /// 获取查询统计
    fn query_stats(&self) -> QueryStats;

    /// 获取连接获取统计
    fn connection_stats(&self) -> ConnectionAcquireStats;

    /// 获取连接池指标
    fn pool_metrics(&self) -> PoolMetrics;

    /// 获取事务统计
    fn transaction_stats(&self) -> TransactionStats;

    /// 导出 Prometheus 格式指标
    ///
    /// # Returns
    ///
    /// 返回 Prometheus 格式的指标字符串
    fn export_prometheus(&self) -> String;

    /// 清空所有统计
    fn clear(&self);
}

/// 最大延迟样本数（滑动窗口大小）
const MAX_LATENCY_SAMPLES: usize = 10000;

/// 延迟百分位数据
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LatencyPercentiles {
    /// P50 延迟（纳秒）
    pub p50_ns: u64,
    /// P75 延迟（纳秒）
    pub p75_ns: u64,
    /// P90 延迟（纳秒）
    pub p90_ns: u64,
    /// P95 延迟（纳秒）
    pub p95_ns: u64,
    /// P99 延迟（纳秒）
    pub p99_ns: u64,
    /// P99.9 延迟（纳秒）
    pub p999_ns: u64,
    /// 最小延迟（纳秒）
    pub min_ns: u64,
    /// 最大延迟（纳秒）
    pub max_ns: u64,
    /// 样本数量
    pub sample_count: u64,
}

impl LatencyPercentiles {
    /// 获取 P50 延迟
    pub fn p50(&self) -> Duration {
        Duration::from_nanos(self.p50_ns)
    }

    /// 获取 P75 延迟
    pub fn p75(&self) -> Duration {
        Duration::from_nanos(self.p75_ns)
    }

    /// 获取 P90 延迟
    pub fn p90(&self) -> Duration {
        Duration::from_nanos(self.p90_ns)
    }

    /// 获取 P95 延迟
    pub fn p95(&self) -> Duration {
        Duration::from_nanos(self.p95_ns)
    }

    /// 获取 P99 延迟
    pub fn p99(&self) -> Duration {
        Duration::from_nanos(self.p99_ns)
    }

    /// 获取 P99.9 延迟
    pub fn p999(&self) -> Duration {
        Duration::from_nanos(self.p999_ns)
    }

    /// 获取最小延迟
    pub fn min(&self) -> Duration {
        Duration::from_nanos(self.min_ns)
    }

    /// 获取最大延迟
    pub fn max(&self) -> Duration {
        Duration::from_nanos(self.max_ns)
    }
}

/// 延迟直方图桶
#[derive(Debug)]
pub struct LatencyHistogram {
    /// 桶边界（毫秒）
    buckets: Vec<u64>,
    /// 每个桶的计数
    counts: Vec<AtomicU64>,
    /// 总样本数
    total: AtomicU64,
}

impl LatencyHistogram {
    /// 创建新的延迟直方图
    ///
    /// # Arguments
    ///
    /// * `bucket_boundaries` - 桶边界定义（毫秒），如 [1, 5, 10, 50, 100, 500, 1000]
    pub fn new(bucket_boundaries: Vec<u64>) -> Self {
        let counts: Vec<_> = (0..bucket_boundaries.len() + 1).map(|_| AtomicU64::new(0)).collect();

        Self {
            buckets: bucket_boundaries,
            counts,
            total: AtomicU64::new(0),
        }
    }

    /// 记录一次延迟
    pub fn record(&self, duration: Duration) {
        let latency_ms = duration.as_millis() as u64;
        let mut bucket_idx = 0;

        for (idx, boundary) in self.buckets.iter().enumerate() {
            if latency_ms <= *boundary {
                bucket_idx = idx;
                break;
            }
            bucket_idx = idx + 1;
        }

        self.counts[bucket_idx].fetch_add(1, Ordering::SeqCst);
        self.total.fetch_add(1, Ordering::SeqCst);
    }

    /// 获取直方图统计
    pub fn stats(&self) -> HistogramStats {
        let total = self.total.load(Ordering::SeqCst);

        let mut cumulative = 0u64;
        let mut bucket_stats = Vec::new();

        for (idx, boundary) in self.buckets.iter().enumerate() {
            let count = self.counts[idx].load(Ordering::SeqCst);
            cumulative += count;
            bucket_stats.push(HistogramBucket {
                boundary_ms: *boundary,
                count,
                cumulative_count: cumulative,
                percentile: if total > 0 {
                    (cumulative as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
            });
        }

        // 溢出桶
        let overflow_count = self.counts[self.buckets.len()].load(Ordering::SeqCst);
        cumulative += overflow_count;
        bucket_stats.push(HistogramBucket {
            boundary_ms: u64::MAX,
            count: overflow_count,
            cumulative_count: cumulative,
            percentile: if total > 0 {
                (cumulative as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        });

        HistogramStats {
            total_samples: total,
            buckets: bucket_stats,
        }
    }

    /// 原子重置所有桶计数（v0.3.0 性能优化：支持无锁 reset）
    pub fn reset(&self) {
        for c in &self.counts {
            c.store(0, Ordering::SeqCst);
        }
        self.total.store(0, Ordering::SeqCst);
    }
}

/// 直方图桶统计
#[derive(Debug, Clone, PartialEq)]
pub struct HistogramBucket {
    /// 桶边界（毫秒）
    pub boundary_ms: u64,
    /// 桶内样本数
    pub count: u64,
    /// 累计样本数
    pub cumulative_count: u64,
    /// 累计百分比
    pub percentile: f64,
}

/// 直方图统计
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistogramStats {
    /// 总样本数
    pub total_samples: u64,
    /// 桶统计
    pub buckets: Vec<HistogramBucket>,
}

/// 吞吐量统计
#[derive(Debug, Clone, PartialEq)]
pub struct ThroughputStats {
    /// 总操作数
    pub total_operations: u64,
    /// 成功操作数
    pub success_count: u64,
    /// 失败操作数
    pub failure_count: u64,
    /// 错误率
    pub error_rate: f64,
    /// 平均 QPS
    pub avg_qps: f64,
    /// 窗口 QPS
    pub window_qps: f64,
}

impl Default for ThroughputStats {
    fn default() -> Self {
        Self {
            total_operations: 0,
            success_count: 0,
            failure_count: 0,
            error_rate: 0.0,
            avg_qps: 0.0,
            window_qps: 0.0,
        }
    }
}

/// 查询统计信息（增强版）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QueryStats {
    /// 查询次数
    pub count: u64,
    /// 错误次数
    pub error_count: u64,
    /// 延迟百分位
    pub latency_percentiles: LatencyPercentiles,
    /// 直方图统计
    pub histogram: HistogramStats,
    /// 吞吐量统计
    pub throughput: ThroughputStats,
}

impl QueryStats {
    /// 获取错误率
    pub fn error_rate(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.error_count as f64 / self.count as f64
        }
    }
}

/// 慢查询配置
#[derive(Debug, Clone)]
pub struct SlowQueryConfig {
    /// 慢查询阈值（毫秒）
    pub threshold_ms: u64,
    /// 是否记录慢查询
    pub enabled: bool,
}

/// 慢查询记录
#[derive(Debug, Clone)]
pub struct SlowQueryRecord {
    /// 查询类型
    pub query_type: String,
    /// 查询耗时
    pub duration_ms: u64,
    /// 记录时间
    pub timestamp: time::OffsetDateTime,
}

/// 连接获取统计
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConnectionAcquireStats {
    /// 总尝试次数
    pub total_attempts: u64,
    /// 成功次数
    pub success_count: u64,
    /// 超时次数
    pub timeout_count: u64,
    /// 失败次数
    pub failure_count: u64,
    /// 超时率
    pub timeout_rate: f64,
    /// 慢获取次数（>3s）
    pub slow_acquires: u64,
    /// 警告级超时次数（3s-5s）
    pub timeout_warn: u64,
    /// 错误级超时次数（5s-10s）
    pub timeout_error: u64,
    /// 严重级超时次数（>=10s）
    pub timeout_critical: u64,
    /// 连接获取延迟直方图
    pub acquire_histogram: HistogramStats,
}

/// 事务统计
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransactionStats {
    /// 总事务数
    pub total_transactions: u64,
    /// 提交次数
    pub commit_count: u64,
    /// 回滚次数
    pub rollback_count: u64,
    /// 失败次数
    pub failure_count: u64,
    /// 成功率
    pub success_rate: f64,
}

/// 连接池指标
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PoolMetrics {
    /// 总连接数
    pub total: u64,
    /// 活跃连接数
    pub active: u64,
    /// 空闲连接数
    pub idle: u64,
}

impl PoolMetrics {
    /// 获取连接使用率
    pub fn utilization_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.active as f64 / self.total as f64
        }
    }
}

/// 延迟样本存储（使用滑动窗口限制内存使用）
#[derive(Debug)]
struct LatencyStorage {
    /// 存储的延迟样本（滑动窗口，使用 VecDeque 实现）
    samples: VecDeque<u64>,
    /// 最小延迟
    min: u64,
    /// 最大延迟
    max: u64,
}

impl LatencyStorage {
    fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(MAX_LATENCY_SAMPLES),
            min: u64::MAX,
            max: 0,
        }
    }

    fn record(&mut self, latency_ns: u64) {
        // 使用滑动窗口：如果达到最大容量，移除最旧的样本
        if self.samples.len() >= MAX_LATENCY_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(latency_ns);

        if latency_ns < self.min {
            self.min = latency_ns;
        }
        if latency_ns > self.max {
            self.max = latency_ns;
        }
    }

    fn percentiles(&self) -> LatencyPercentiles {
        if self.samples.is_empty() {
            return LatencyPercentiles::default();
        }

        let mut sorted: Vec<_> = self.samples.iter().cloned().collect();
        sorted.sort();

        let len = sorted.len();
        let p50_idx = (len as f64 * 0.50) as usize;
        let p75_idx = (len as f64 * 0.75) as usize;
        let p90_idx = (len as f64 * 0.90) as usize;
        let p95_idx = (len as f64 * 0.95) as usize;
        let p99_idx = (len as f64 * 0.99) as usize;
        let p999_idx = (len as f64 * 0.999) as usize;

        LatencyPercentiles {
            p50_ns: sorted[p50_idx],
            p75_ns: sorted[p75_idx],
            p90_ns: sorted[p90_idx],
            p95_ns: sorted[p95_idx],
            p99_ns: sorted[p99_idx],
            p999_ns: sorted[p999_idx],
            min_ns: self.min,
            max_ns: self.max,
            sample_count: self.samples.len() as u64,
        }
    }

    fn clear(&mut self) {
        self.samples.clear();
        self.min = u64::MAX;
        self.max = 0;
    }
}

/// Metrics 收集器（增强版）
///
/// 提供全面的性能指标收集功能
#[derive(Clone)]
pub struct MetricsCollector {
    /// 按查询类型分类的指标
    query_metrics: Arc<RwLock<HashMap<String, Arc<QueryMetricsInner>>>>,

    /// 连接池总连接数
    pool_total: Arc<AtomicU64>,
    /// 连接池活跃连接数
    pool_active: Arc<AtomicU64>,
    /// 连接池空闲连接数
    pool_idle: Arc<AtomicU64>,

    /// 连接错误计数
    connection_errors: Arc<AtomicU64>,
    /// 查询错误计数
    query_errors: Arc<AtomicU64>,

    /// 连接获取指标（v0.3.0：移除 RwLock，内部全为 AtomicU64，无锁访问）
    connection_acquire: Arc<ConnectionAcquireMetricsInner>,
    /// 事务指标（v0.3.0：移除 RwLock，内部全为 AtomicU64，无锁访问）
    transaction: Arc<TransactionMetricsInner>,

    /// 慢查询记录（最近 N 条）
    slow_queries: Arc<RwLock<VecDeque<SlowQueryRecord>>>,
    /// 慢查询配置
    slow_query_config: Arc<RwLock<SlowQueryConfig>>,
    /// 慢查询最大记录数
    max_slow_queries: usize,

    /// 启动时间
    start_time: Instant,
}

struct QueryMetricsInner {
    /// 延迟存储
    latency: RwLock<LatencyStorage>,
    /// 直方图
    histogram: LatencyHistogram,
    /// 吞吐量跟踪器
    throughput: ThroughputTrackerInner,
    /// 错误计数
    error_count: AtomicU64,
}

struct ThroughputTrackerInner {
    success_count: AtomicU64,
    failure_count: AtomicU64,
    bytes_total: AtomicU64,
    last_record_time: AtomicU64,
}

impl ThroughputTrackerInner {
    fn new() -> Self {
        Self {
            success_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            bytes_total: AtomicU64::new(0),
            last_record_time: AtomicU64::new(0),
        }
    }

    fn record_success(&self, bytes: Option<u64>) {
        let now = Instant::now().elapsed().as_secs();
        self.success_count.fetch_add(1, Ordering::SeqCst);
        self.last_record_time.store(now, Ordering::SeqCst);
        if let Some(b) = bytes {
            self.bytes_total.fetch_add(b, Ordering::SeqCst);
        }
    }

    fn record_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::SeqCst);
    }

    fn throughput(&self, elapsed_secs: u64) -> ThroughputStats {
        let success = self.success_count.load(Ordering::SeqCst);
        let failure = self.failure_count.load(Ordering::SeqCst);
        let total = success + failure;
        let avg_qps = if elapsed_secs > 0 {
            total as f64 / elapsed_secs as f64
        } else {
            total as f64
        };

        ThroughputStats {
            total_operations: total,
            success_count: success,
            failure_count: failure,
            error_rate: if total > 0 { failure as f64 / total as f64 } else { 0.0 },
            avg_qps,
            window_qps: 0.0,
        }
    }

    fn total_operations(&self) -> u64 {
        self.success_count.load(Ordering::SeqCst) + self.failure_count.load(Ordering::SeqCst)
    }
}

struct ConnectionAcquireMetricsInner {
    total_attempts: AtomicU64,
    success_count: AtomicU64,
    timeout_count: AtomicU64,
    failure_count: AtomicU64,
    /// 慢获取计数（>3s 警告阈值）
    slow_acquires: AtomicU64,
    /// 分级超时计数 (warn/error/critical)
    timeout_warn: AtomicU64,
    timeout_error: AtomicU64,
    timeout_critical: AtomicU64,
    /// 连接获取延迟直方图（100ms, 500ms, 1s, 3s, 5s, 10s buckets）
    acquire_duration: LatencyHistogram,
}

impl ConnectionAcquireMetricsInner {
    fn new() -> Self {
        Self {
            total_attempts: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            timeout_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            slow_acquires: AtomicU64::new(0),
            timeout_warn: AtomicU64::new(0),
            timeout_error: AtomicU64::new(0),
            timeout_critical: AtomicU64::new(0),
            acquire_duration: LatencyHistogram::new(vec![100, 500, 1000, 3000, 5000, 10000]),
        }
    }

    fn record_success(&self) {
        self.total_attempts.fetch_add(1, Ordering::SeqCst);
        self.success_count.fetch_add(1, Ordering::SeqCst);
    }

    fn record_timeout(&self) {
        self.total_attempts.fetch_add(1, Ordering::SeqCst);
        self.timeout_count.fetch_add(1, Ordering::SeqCst);
    }

    fn record_failure(&self) {
        self.total_attempts.fetch_add(1, Ordering::SeqCst);
        self.failure_count.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录连接获取延迟
    fn record_acquire_duration(&self, duration: Duration) {
        self.total_attempts.fetch_add(1, Ordering::SeqCst);
        self.success_count.fetch_add(1, Ordering::SeqCst);
        self.acquire_duration.record(duration);
        // 慢获取阈值: 3000ms
        if duration.as_millis() as u64 >= 3000 {
            self.slow_acquires.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// 记录分级超时（warn/error/critical）
    fn record_timeout_level(&self, elapsed_ms: u64) {
        self.total_attempts.fetch_add(1, Ordering::SeqCst);
        self.timeout_count.fetch_add(1, Ordering::SeqCst);
        if elapsed_ms >= 10000 {
            self.timeout_critical.fetch_add(1, Ordering::SeqCst);
        } else if elapsed_ms >= 5000 {
            self.timeout_error.fetch_add(1, Ordering::SeqCst);
        } else {
            self.timeout_warn.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn stats(&self) -> ConnectionAcquireStats {
        let total = self.total_attempts.load(Ordering::SeqCst);
        ConnectionAcquireStats {
            total_attempts: total,
            success_count: self.success_count.load(Ordering::SeqCst),
            timeout_count: self.timeout_count.load(Ordering::SeqCst),
            failure_count: self.failure_count.load(Ordering::SeqCst),
            timeout_rate: if total > 0 {
                self.timeout_count.load(Ordering::SeqCst) as f64 / total as f64
            } else {
                0.0
            },
            slow_acquires: self.slow_acquires.load(Ordering::SeqCst),
            timeout_warn: self.timeout_warn.load(Ordering::SeqCst),
            timeout_error: self.timeout_error.load(Ordering::SeqCst),
            timeout_critical: self.timeout_critical.load(Ordering::SeqCst),
            acquire_histogram: self.acquire_duration.stats(),
        }
    }

    /// 重置所有计数器（v0.3.0 性能优化：移除 RwLock 后用于替代 `*inner = Inner::new()`）
    fn reset(&self) {
        self.total_attempts.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        self.timeout_count.store(0, Ordering::SeqCst);
        self.failure_count.store(0, Ordering::SeqCst);
        self.slow_acquires.store(0, Ordering::SeqCst);
        self.timeout_warn.store(0, Ordering::SeqCst);
        self.timeout_error.store(0, Ordering::SeqCst);
        self.timeout_critical.store(0, Ordering::SeqCst);
        self.acquire_duration.reset();
    }
}

struct TransactionMetricsInner {
    total_transactions: AtomicU64,
    commit_count: AtomicU64,
    rollback_count: AtomicU64,
    failure_count: AtomicU64,
}

impl TransactionMetricsInner {
    fn new() -> Self {
        Self {
            total_transactions: AtomicU64::new(0),
            commit_count: AtomicU64::new(0),
            rollback_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
        }
    }

    fn record_commit(&self) {
        self.total_transactions.fetch_add(1, Ordering::SeqCst);
        self.commit_count.fetch_add(1, Ordering::SeqCst);
    }

    fn record_rollback(&self) {
        self.total_transactions.fetch_add(1, Ordering::SeqCst);
        self.rollback_count.fetch_add(1, Ordering::SeqCst);
    }

    fn record_failure(&self) {
        self.total_transactions.fetch_add(1, Ordering::SeqCst);
        self.failure_count.fetch_add(1, Ordering::SeqCst);
    }

    fn stats(&self) -> TransactionStats {
        let total = self.total_transactions.load(Ordering::SeqCst);
        TransactionStats {
            total_transactions: total,
            commit_count: self.commit_count.load(Ordering::SeqCst),
            rollback_count: self.rollback_count.load(Ordering::SeqCst),
            failure_count: self.failure_count.load(Ordering::SeqCst),
            success_rate: if total > 0 {
                (self.commit_count.load(Ordering::SeqCst) as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        }
    }

    /// 重置所有事务计数器（v0.3.0 性能优化：移除 RwLock 后用于替代 `*inner = Inner::new()`）
    fn reset(&self) {
        self.total_transactions.store(0, Ordering::SeqCst);
        self.commit_count.store(0, Ordering::SeqCst);
        self.rollback_count.store(0, Ordering::SeqCst);
        self.failure_count.store(0, Ordering::SeqCst);
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// 创建新的 Metrics 收集器
    pub fn new() -> Self {
        Self {
            query_metrics: Arc::new(RwLock::new(HashMap::new())),
            pool_total: Arc::new(AtomicU64::new(0)),
            pool_active: Arc::new(AtomicU64::new(0)),
            pool_idle: Arc::new(AtomicU64::new(0)),
            connection_errors: Arc::new(AtomicU64::new(0)),
            query_errors: Arc::new(AtomicU64::new(0)),
            connection_acquire: Arc::new(ConnectionAcquireMetricsInner::new()),
            transaction: Arc::new(TransactionMetricsInner::new()),
            slow_queries: Arc::new(RwLock::new(VecDeque::new())),
            slow_query_config: Arc::new(RwLock::new(SlowQueryConfig {
                threshold_ms: 1000,
                enabled: true,
            })),
            max_slow_queries: 100,
            start_time: Instant::now(),
        }
    }
}

impl MetricsCollector {
    /// 记录一次查询
    pub fn record_query(&self, query_type: &str, duration: Duration, success: bool, bytes: Option<u64>) {
        let latency_ns = duration.as_nanos() as u64;
        let duration_ms = duration.as_millis() as u64;

        // 获取或创建指标
        let metrics = {
            let mut map = self.query_metrics.write();
            if let Some(m) = map.get(query_type) {
                m.clone()
            } else {
                let new_metrics = Arc::new(QueryMetricsInner {
                    latency: RwLock::new(LatencyStorage::new()),
                    histogram: LatencyHistogram::new(vec![1, 5, 10, 25, 50, 100, 250, 500, 1000, 5000]),
                    throughput: ThroughputTrackerInner::new(),
                    error_count: AtomicU64::new(0),
                });
                map.insert(query_type.to_string(), new_metrics.clone());
                new_metrics
            }
        };

        // 记录延迟
        metrics.latency.write().record(latency_ns);
        metrics.histogram.record(duration);

        // 记录吞吐量
        if success {
            metrics.throughput.record_success(bytes);
        } else {
            metrics.throughput.record_failure();
            metrics.error_count.fetch_add(1, Ordering::SeqCst);
            self.query_errors.fetch_add(1, Ordering::SeqCst);
        }

        // 检查是否为慢查询
        let config = self.slow_query_config.read();
        if config.enabled && duration_ms >= config.threshold_ms {
            let mut slow = self.slow_queries.write();
            slow.push_back(SlowQueryRecord {
                query_type: query_type.to_string(),
                duration_ms,
                timestamp: time::OffsetDateTime::now_utc(),
            });
            while slow.len() > self.max_slow_queries {
                slow.pop_front();
            }
        }
    }

    /// 获取查询类型统计
    pub fn get_query_stats(&self, query_type: &str) -> Option<QueryStats> {
        let map = self.query_metrics.read();
        map.get(query_type).map(|m| {
            let elapsed = self.start_time.elapsed().as_secs();
            let throughput = m.throughput.throughput(elapsed);
            let latency = m.latency.read().percentiles();
            let histogram = m.histogram.stats();

            QueryStats {
                count: m.throughput.total_operations(),
                error_count: m.error_count.load(Ordering::SeqCst),
                latency_percentiles: latency,
                histogram,
                throughput,
            }
        })
    }

    /// 获取所有查询统计
    pub fn all_query_stats(&self) -> HashMap<String, QueryStats> {
        let map = self.query_metrics.read();
        let elapsed = self.start_time.elapsed().as_secs();
        map.iter()
            .map(|(k, v)| {
                let throughput = v.throughput.throughput(elapsed);
                let latency = v.latency.read().percentiles();
                let histogram = v.histogram.stats();

                (
                    k.clone(),
                    QueryStats {
                        count: v.throughput.total_operations(),
                        error_count: v.error_count.load(Ordering::SeqCst),
                        latency_percentiles: latency,
                        histogram,
                        throughput,
                    },
                )
            })
            .collect()
    }

    /// 获取总吞吐量统计
    pub fn total_throughput(&self) -> ThroughputStats {
        let elapsed = self.start_time.elapsed().as_secs();
        let map = self.query_metrics.read();
        let mut total = ThroughputStats {
            total_operations: 0,
            success_count: 0,
            failure_count: 0,
            error_rate: 0.0,
            avg_qps: 0.0,
            window_qps: 0.0,
        };

        for m in map.values() {
            let throughput = m.throughput.throughput(elapsed);
            total.total_operations += throughput.total_operations;
            total.success_count += throughput.success_count;
            total.failure_count += throughput.failure_count;
            total.avg_qps += throughput.avg_qps;
        }

        if total.total_operations > 0 {
            total.error_rate = total.failure_count as f64 / total.total_operations as f64;
        }

        total
    }

    /// 获取慢查询记录
    pub fn slow_queries(&self) -> Vec<SlowQueryRecord> {
        self.slow_queries.read().iter().cloned().collect()
    }

    /// 设置慢查询阈值
    pub fn set_slow_query_threshold(&self, threshold_ms: u64) {
        let mut config = self.slow_query_config.write();
        config.threshold_ms = threshold_ms;
    }

    /// 启用/禁用慢查询记录
    pub fn set_slow_query_enabled(&self, enabled: bool) {
        let mut config = self.slow_query_config.write();
        config.enabled = enabled;
    }

    /// 记录连接错误
    pub fn record_connection_error(&self) {
        self.connection_errors.fetch_add(1, Ordering::SeqCst);
    }

    /// 获取连接错误计数
    pub fn connection_error_count(&self) -> u64 {
        self.connection_errors.load(Ordering::SeqCst)
    }

    /// 更新连接池状态
    pub fn update_pool_status(&self, total: u32, active: u32, idle: u32) {
        self.pool_total.store(total as u64, Ordering::SeqCst);
        self.pool_active.store(active as u64, Ordering::SeqCst);
        self.pool_idle.store(idle as u64, Ordering::SeqCst);
    }

    /// 获取连接池状态
    pub fn pool_status(&self) -> PoolMetrics {
        PoolMetrics {
            total: self.pool_total.load(Ordering::SeqCst),
            active: self.pool_active.load(Ordering::SeqCst),
            idle: self.pool_idle.load(Ordering::SeqCst),
        }
    }

    /// 记录连接获取成功
    pub fn record_connection_acquire_success(&self) {
        self.connection_acquire.record_success();
    }

    /// 记录连接获取超时
    pub fn record_connection_acquire_timeout(&self) {
        self.connection_acquire.record_timeout();
    }

    /// 记录连接获取失败
    pub fn record_connection_acquire_failure(&self) {
        self.connection_acquire.record_failure();
    }

    /// 记录连接获取延迟（成功时调用）
    pub fn record_connection_acquire_duration(&self, duration: Duration) {
        self.connection_acquire.record_acquire_duration(duration);
    }

    /// 记录分级超时（根据耗时判断级别）
    pub fn record_connection_timeout_level(&self, elapsed_ms: u64) {
        self.connection_acquire.record_timeout_level(elapsed_ms);
    }

    /// 获取连接获取统计
    pub fn connection_acquire_stats(&self) -> ConnectionAcquireStats {
        self.connection_acquire.stats()
    }

    /// 记录事务提交
    pub fn record_transaction_commit(&self) {
        self.transaction.record_commit();
    }

    /// 记录事务回滚
    pub fn record_transaction_rollback(&self) {
        self.transaction.record_rollback();
    }

    /// 记录事务失败
    pub fn record_transaction_failure(&self) {
        self.transaction.record_failure();
    }

    /// 获取事务统计
    pub fn transaction_stats(&self) -> TransactionStats {
        self.transaction.stats()
    }

    /// 获取事务统计（内部方法，用于 trait 实现）
    pub fn transaction_stats_inner(&self) -> TransactionStats {
        self.transaction.stats()
    }

    /// 获取运行时长
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// 重置所有指标
    pub fn reset(&self) {
        self.pool_total.store(0, Ordering::SeqCst);
        self.pool_active.store(0, Ordering::SeqCst);
        self.pool_idle.store(0, Ordering::SeqCst);
        self.connection_errors.store(0, Ordering::SeqCst);
        self.query_errors.store(0, Ordering::SeqCst);

        let mut map = self.query_metrics.write();
        for metrics in map.values() {
            metrics.latency.write().clear();
            // 无法重置原子计数器，但它们会在下次统计时被覆盖
        }
        map.clear();

        let mut slow = self.slow_queries.write();
        slow.clear();

        // v0.3.0：移除 RwLock 后改用原子 reset() 替代整体替换
        self.connection_acquire.reset();
        self.transaction.reset();
    }

    /// 重置所有指标（内部方法，用于 trait 实现）
    pub fn clear_all_inner(&self) {
        self.reset();
    }

    /// 导出为 Prometheus 格式
    pub fn export_prometheus(&self) -> String {
        // 优化：预分配缓冲区，减少字符串分配
        let mut output = String::with_capacity(2048);
        let now = time::OffsetDateTime::now_utc();

        let uptime_seconds = self.uptime().as_secs_f64();
        output.push_str("# TYPE dbnexus_uptime gauge\n");
        use std::fmt::Write;
        writeln!(output, "dbnexus_uptime_seconds {:.3}", uptime_seconds).unwrap();

        // 连接池指标
        output.push_str("# TYPE dbnexus_pool_connections gauge\n");
        writeln!(
            output,
            "dbnexus_pool_connections_total {}",
            self.pool_total.load(Ordering::SeqCst)
        )
        .unwrap();
        writeln!(
            output,
            "dbnexus_pool_connections_active {}",
            self.pool_active.load(Ordering::SeqCst)
        )
        .unwrap();
        writeln!(
            output,
            "dbnexus_pool_connections_idle {}",
            self.pool_idle.load(Ordering::SeqCst)
        )
        .unwrap();
        writeln!(
            output,
            "dbnexus_pool_connections_utilization {:.4}",
            self.pool_status().utilization_rate()
        )
        .unwrap();

        // 错误指标
        output.push_str("# TYPE dbnexus_errors counter\n");
        writeln!(
            output,
            "dbnexus_connection_errors_total {}",
            self.connection_errors.load(Ordering::SeqCst)
        )
        .unwrap();
        writeln!(
            output,
            "dbnexus_query_errors_total {}",
            self.query_errors.load(Ordering::SeqCst)
        )
        .unwrap();

        // 连接获取指标
        let acquire_stats = self.connection_acquire_stats();
        output.push_str("# TYPE dbnexus_connection_acquire counter\n");
        writeln!(
            output,
            "dbnexus_connection_acquire_total {}",
            acquire_stats.total_attempts
        )
        .unwrap();
        writeln!(
            output,
            "dbnexus_connection_acquire_timeout_total {}",
            acquire_stats.timeout_count
        )
        .unwrap();
        writeln!(
            output,
            "dbnexus_connection_acquire_failure_total {}",
            acquire_stats.failure_count
        )
        .unwrap();
        writeln!(
            output,
            "dbnexus_connection_acquire_slow_total {}",
            acquire_stats.slow_acquires
        )
        .unwrap();

        // 分级超时指标（带级别标签）
        output.push_str("# TYPE dbnexus_connection_timeout_total counter\n");
        writeln!(
            output,
            "dbnexus_connection_timeout_total{{level=\"warn\"}} {}",
            acquire_stats.timeout_warn
        )
        .unwrap();
        writeln!(
            output,
            "dbnexus_connection_timeout_total{{level=\"error\"}} {}",
            acquire_stats.timeout_error
        )
        .unwrap();
        writeln!(
            output,
            "dbnexus_connection_timeout_total{{level=\"critical\"}} {}",
            acquire_stats.timeout_critical
        )
        .unwrap();

        // 连接获取延迟直方图
        output.push_str("# TYPE dbnexus_pool_acquire_duration_seconds histogram\n");
        for bucket in &acquire_stats.acquire_histogram.buckets {
            writeln!(
                output,
                "dbnexus_pool_acquire_duration_seconds_bucket{{le=\"{}}} {}",
                bucket.boundary_ms as f64 / 1000.0,
                bucket.cumulative_count
            )
            .unwrap();
        }
        writeln!(
            output,
            "dbnexus_pool_acquire_duration_seconds_bucket{{le=\"+Inf\"}} {}",
            acquire_stats.acquire_histogram.total_samples
        )
        .unwrap();
        writeln!(output, "dbnexus_pool_acquire_duration_seconds_sum {}", 0.0).unwrap();
        writeln!(
            output,
            "dbnexus_pool_acquire_duration_seconds_count {}",
            acquire_stats.acquire_histogram.total_samples
        )
        .unwrap();

        // 事务指标
        let txn_stats = self.transaction_stats();
        output.push_str("# TYPE dbnexus_transactions counter\n");
        writeln!(output, "dbnexus_transactions_total {}", txn_stats.total_transactions).unwrap();
        writeln!(output, "dbnexus_transactions_commit_total {}", txn_stats.commit_count).unwrap();
        writeln!(
            output,
            "dbnexus_transactions_rollback_total {}",
            txn_stats.rollback_count
        )
        .unwrap();
        writeln!(output, "dbnexus_transactions_failure_total {}", txn_stats.failure_count).unwrap();
        writeln!(
            output,
            "dbnexus_transactions_success_rate {:.2}",
            txn_stats.success_rate
        )
        .unwrap();

        // 查询指标
        let stats = self.all_query_stats();
        for (query_type, stat) in stats {
            let type_label = query_type.to_lowercase();

            // 使用 writeln! 替代 push_str + format!
            writeln!(
                output,
                "# TYPE dbnexus_queries_total counter\ndbnexus_queries_total{{type=\"{}\"}} {}",
                type_label, stat.count
            )
            .unwrap();

            output.push_str("# TYPE dbnexus_query_throughput gauge\n");
            writeln!(
                output,
                "dbnexus_query_throughput_qps{{type=\"{}\"}} {:.2}",
                type_label, stat.throughput.avg_qps
            )
            .unwrap();

            // 延迟百分位
            output.push_str("# TYPE dbnexus_query_latency_seconds gauge\n");
            let p50 = stat.latency_percentiles.p50().as_secs_f64();
            let p90 = stat.latency_percentiles.p90().as_secs_f64();
            let p95 = stat.latency_percentiles.p95().as_secs_f64();
            let p99 = stat.latency_percentiles.p99().as_secs_f64();

            writeln!(
                output,
                "dbnexus_query_latency_p50_seconds{{type=\"{}\"}} {:.6}",
                type_label, p50
            )
            .unwrap();
            writeln!(
                output,
                "dbnexus_query_latency_p90_seconds{{type=\"{}\"}} {:.6}",
                type_label, p90
            )
            .unwrap();
            writeln!(
                output,
                "dbnexus_query_latency_p95_seconds{{type=\"{}\"}} {:.6}",
                type_label, p95
            )
            .unwrap();
            writeln!(
                output,
                "dbnexus_query_latency_p99_seconds{{type=\"{}\"}} {:.6}",
                type_label, p99
            )
            .unwrap();
        }

        // 总吞吐量
        let total = self.total_throughput();
        output.push_str("# TYPE dbnexus_total_throughput gauge\n");
        writeln!(output, "dbnexus_total_qps {:.2}", total.avg_qps).unwrap();
        writeln!(output, "dbnexus_total_operations {}", total.total_operations).unwrap();
        writeln!(output, "dbnexus_error_rate {:.4}", total.error_rate).unwrap();

        output.push_str("# TYPE dbnexus_metrics_timestamp gauge\n");
        output.push_str(&format!("dbnexus_metrics_timestamp {}\n", now.unix_timestamp()));

        output
    }

    /// 导出为 Prometheus 格式（内部方法，用于 trait 实现）
    pub fn export_prometheus_inner(&self) -> String {
        self.export_prometheus()
    }
}

// ============================================================================
// MetricsCollector Trait Implementation
// ============================================================================

impl MetricsCollectorTrait for MetricsCollector {
    fn record_query(&self, duration: Duration) {
        self.record_query("default", duration, true, None);
    }

    fn record_connection(&self, duration: Duration) {
        let start = Instant::now();
        if duration.as_millis() < 100 {
            self.record_connection_acquire_success();
        } else if duration.as_millis() < 1000 {
            self.record_connection_acquire_timeout();
        } else {
            self.record_connection_acquire_failure();
        }
        let _ = start;
    }

    fn record_transaction(&self, duration: Duration, success: bool) {
        let _ = duration;
        if success {
            self.record_transaction_commit();
        } else {
            self.record_transaction_failure();
        }
    }

    fn record_pool_usage(&self, total: u32, active: u32, idle: u32) {
        self.update_pool_status(total, active, idle);
    }

    fn query_stats(&self) -> QueryStats {
        self.get_query_stats("default").unwrap_or_default()
    }

    fn connection_stats(&self) -> ConnectionAcquireStats {
        self.connection_acquire_stats()
    }

    fn pool_metrics(&self) -> PoolMetrics {
        self.pool_status()
    }

    fn transaction_stats(&self) -> TransactionStats {
        self.transaction_stats_inner()
    }

    fn export_prometheus(&self) -> String {
        self.export_prometheus_inner()
    }

    fn clear(&self) {
        self.clear_all_inner();
    }
}

// ============================================================================
// MockMetrics - 用于测试的 Mock 实现
// ============================================================================

/// Mock 指标收集器
///
/// 用于测试的指标收集器实现，所有操作都是无操作（no-op）。
/// 这个实现可用于单元测试和集成测试，避免依赖真实的指标收集逻辑。
///
/// # Example
///
/// ```rust,ignore
/// // 注意：MockMetrics 需启用 `test-utils` feature（或在 test build 中）才可用
/// use std::sync::Arc;
/// use dbnexus::{MetricsCollectorTrait, MockMetrics};
///
/// // 在测试中使用 MockMetrics
/// let mock: Arc<dyn MetricsCollectorTrait> = Arc::new(MockMetrics::new());
/// mock.record_query(std::time::Duration::from_millis(10));
/// mock.record_connection(std::time::Duration::from_millis(5));
/// ```
// MockMetrics 仅在测试或启用 `test-utils` feature 时编译（BREAKING: 从默认公共 API 移除）
#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone, Default)]
pub struct MockMetrics {
    _private: (),
}

#[cfg(any(test, feature = "test-utils"))]
impl MockMetrics {
    /// 创建新的 MockMetrics
    pub fn new() -> Self {
        Self { _private: () }
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl MetricsCollectorTrait for MockMetrics {
    fn record_query(&self, _duration: Duration) {
        // No-op: Mock 实现不记录任何指标
    }

    fn record_connection(&self, _duration: Duration) {
        // No-op: Mock 实现不记录任何指标
    }

    fn record_transaction(&self, _duration: Duration, _success: bool) {
        // No-op: Mock 实现不记录任何指标
    }

    fn record_pool_usage(&self, _total: u32, _active: u32, _idle: u32) {
        // No-op: Mock 实现不记录任何指标
    }

    fn query_stats(&self) -> QueryStats {
        QueryStats::default()
    }

    fn connection_stats(&self) -> ConnectionAcquireStats {
        ConnectionAcquireStats::default()
    }

    fn pool_metrics(&self) -> PoolMetrics {
        PoolMetrics::default()
    }

    fn transaction_stats(&self) -> TransactionStats {
        TransactionStats::default()
    }

    fn export_prometheus(&self) -> String {
        String::new()
    }

    fn clear(&self) {
        // No-op: Mock 实现没有状态需要清除
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-047: MockMetrics 基本功能测试
    #[test]
    fn test_mock_metrics_basic() {
        let mock = MockMetrics::new();

        // 测试所有方法都可以调用而不panic
        mock.record_query(Duration::from_millis(10));
        mock.record_connection(Duration::from_millis(5));
        mock.record_transaction(Duration::from_millis(100), true);
        mock.record_pool_usage(10, 5, 5);

        // 测试返回默认值的统计
        assert_eq!(mock.query_stats(), QueryStats::default());
        assert_eq!(mock.connection_stats(), ConnectionAcquireStats::default());
        assert_eq!(mock.pool_metrics(), PoolMetrics::default());
        assert_eq!(mock.transaction_stats(), TransactionStats::default());

        // 测试 Prometheus 导出返回空字符串
        assert_eq!(mock.export_prometheus(), String::new());

        // 测试 clear 不 panic
        mock.clear();
    }

    /// TEST-U-048: MockMetrics Clone 测试
    #[test]
    fn test_mock_metrics_clone() {
        let mock1 = MockMetrics::new();
        let mock2 = mock1.clone();

        // 两个实例应该可以独立使用
        mock1.record_query(Duration::from_millis(10));
        mock2.record_query(Duration::from_millis(20));

        // 都应该正常工作
        assert_eq!(mock1.export_prometheus(), String::new());
        assert_eq!(mock2.export_prometheus(), String::new());
    }

    /// TEST-U-049: MockMetrics 作为 trait 对象使用
    #[test]
    fn test_mock_metrics_trait_object() {
        use std::sync::Arc;

        // 验证 MockMetrics 可以作为 trait 对象使用
        let mock: Arc<dyn MetricsCollectorTrait> = Arc::new(MockMetrics::new());

        mock.record_query(Duration::from_millis(10));
        mock.record_connection(Duration::from_millis(5));
        mock.record_transaction(Duration::from_millis(100), false);
        mock.record_pool_usage(10, 5, 5);

        // 验证返回的统计是默认值
        let stats = mock.query_stats();
        assert_eq!(stats.count, 0);

        let conn_stats = mock.connection_stats();
        assert_eq!(conn_stats.total_attempts, 0);

        mock.clear();
    }

    /// TEST-U-050: MockMetrics 与 MetricsCollector Trait 兼容性测试
    #[test]
    fn test_mock_metrics_trait_compatibility() {
        // 验证 MockMetrics 实现了所有 MetricsCollectorTrait 的方法
        fn _assert_impl_trait(_mock: &dyn MetricsCollectorTrait) {}

        let mock = MockMetrics::new();
        _assert_impl_trait(&mock);
    }

    // ========== Latency Percentiles Tests ==========
    #[test]
    fn test_latency_percentiles() {
        let collector = MetricsCollector::new();

        // 记录不同延迟
        for i in 1..=100 {
            collector.record_query("SELECT", Duration::from_millis(i), true, Some(100));
        }

        let stats = collector.get_query_stats("SELECT").unwrap();
        assert_eq!(stats.count, 100);

        // 验证 P50 大约为 50ms
        assert!(stats.latency_percentiles.p50_ns >= 49_000_000 && stats.latency_percentiles.p50_ns <= 51_000_000);
        // 验证 P99 大约为 99ms
        assert!(stats.latency_percentiles.p99_ns >= 98_000_000 && stats.latency_percentiles.p99_ns <= 100_000_000);
    }

    /// TEST-U-041: 延迟直方图测试
    #[test]
    fn test_latency_histogram() {
        let collector = MetricsCollector::new();

        // 记录不同延迟
        collector.record_query("SELECT", Duration::from_millis(5), true, None);
        collector.record_query("SELECT", Duration::from_millis(15), true, None);
        collector.record_query("SELECT", Duration::from_millis(75), true, None);
        collector.record_query("SELECT", Duration::from_millis(200), true, None);

        let stats = collector.get_query_stats("SELECT").unwrap();
        assert_eq!(stats.histogram.total_samples, 4);
    }

    /// TEST-U-042: 吞吐量测试
    #[test]
    fn test_throughput() {
        let collector = MetricsCollector::new();

        collector.record_query("SELECT", Duration::from_millis(10), true, Some(1024));
        collector.record_query("SELECT", Duration::from_millis(20), true, Some(2048));
        collector.record_query("INSERT", Duration::from_millis(50), false, None);

        let total = collector.total_throughput();
        assert_eq!(total.total_operations, 3);
        assert_eq!(total.success_count, 2);
        assert_eq!(total.failure_count, 1);
        assert!((total.error_rate - 0.333).abs() < 0.01);
    }

    /// TEST-U-043: 连接获取指标测试
    #[test]
    fn test_connection_acquire_metrics() {
        let collector = MetricsCollector::new();

        for _ in 0..50 {
            collector.record_connection_acquire_success();
        }
        for _ in 0..5 {
            collector.record_connection_acquire_timeout();
        }
        for _ in 0..3 {
            collector.record_connection_acquire_failure();
        }

        let stats = collector.connection_acquire_stats();
        assert_eq!(stats.success_count, 50);
        assert_eq!(stats.timeout_count, 5);
        assert_eq!(stats.failure_count, 3);
        assert_eq!(stats.total_attempts, 58);
    }

    /// TEST-U-044: 事务指标测试
    #[test]
    fn test_transaction_metrics() {
        let collector = MetricsCollector::new();

        for _ in 0..100 {
            collector.record_transaction_commit();
        }
        for _ in 0..20 {
            collector.record_transaction_rollback();
        }
        for _ in 0..5 {
            collector.record_transaction_failure();
        }

        let stats = collector.transaction_stats();
        assert_eq!(stats.commit_count, 100);
        assert_eq!(stats.rollback_count, 20);
        assert_eq!(stats.failure_count, 5);
        assert_eq!(stats.total_transactions, 125);
    }

    /// TEST-U-045: Prometheus 导出测试
    #[test]
    fn test_prometheus_export() {
        let collector = MetricsCollector::new();

        collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
        collector.record_query("INSERT", Duration::from_millis(50), false, None);

        let prometheus = collector.export_prometheus();

        assert!(prometheus.contains("dbnexus_uptime_seconds"));
        assert!(prometheus.contains("dbnexus_pool_connections_total"));
        assert!(prometheus.contains("dbnexus_queries_total"));
        assert!(prometheus.contains("dbnexus_total_qps"));
    }

    /// TEST-U-046: 慢查询记录测试
    #[test]
    fn test_slow_query_recording() {
        let collector = MetricsCollector::new();
        collector.set_slow_query_threshold(50);

        collector.record_query("SELECT", Duration::from_millis(100), true, None);

        let slow = collector.slow_queries();
        assert_eq!(slow.len(), 1);
        assert_eq!(slow[0].query_type, "SELECT");
        assert_eq!(slow[0].duration_ms, 100);
    }

    /// TEST-U-051: 无锁 reset 验证（v0.3.0 性能优化）
    ///
    /// 验证移除 RwLock 后 reset() 仍能正确清零所有计数器。
    #[test]
    fn test_reset_clears_all_metrics() {
        let collector = MetricsCollector::new();

        // 填充各类指标
        collector.record_query("SELECT", Duration::from_millis(10), true, Some(100));
        collector.record_query("INSERT", Duration::from_millis(50), false, None);
        collector.record_connection_acquire_success();
        collector.record_connection_acquire_timeout();
        collector.record_connection_acquire_duration(Duration::from_millis(5));
        collector.record_transaction_commit();
        collector.record_transaction_rollback();
        collector.record_connection_error();
        collector.update_pool_status(10, 5, 5);

        // 验证填充
        assert!(collector.total_throughput().total_operations > 0);
        assert!(collector.connection_acquire_stats().total_attempts > 0);
        assert!(collector.transaction_stats().total_transactions > 0);
        assert_eq!(collector.connection_error_count(), 1);
        assert_eq!(collector.pool_status().total, 10);

        // 执行 reset
        collector.reset();

        // 验证所有指标归零
        assert_eq!(collector.total_throughput().total_operations, 0);
        assert_eq!(collector.connection_acquire_stats().total_attempts, 0);
        assert_eq!(collector.transaction_stats().total_transactions, 0);
        assert_eq!(collector.connection_error_count(), 0);
        assert_eq!(collector.pool_status().total, 0);
        assert_eq!(collector.pool_status().active, 0);
        assert_eq!(collector.pool_status().idle, 0);
        assert!(collector.slow_queries().is_empty());
    }

    /// TEST-U-052: 无锁并发访问验证（v0.3.0 性能优化）
    ///
    /// 验证移除 RwLock 后多线程并发记录指标不会 panic 或数据竞争。
    #[test]
    fn test_concurrent_metrics_access() {
        use std::sync::Arc;
        use std::thread;

        let collector = Arc::new(MetricsCollector::new());
        let mut handles = Vec::new();

        // 4 个线程并发写 connection_acquire 和 transaction
        for _ in 0..4 {
            let c = collector.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    c.record_connection_acquire_success();
                    c.record_transaction_commit();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // 验证总计数（无锁并发应准确累加）
        let acquire_stats = collector.connection_acquire_stats();
        let txn_stats = collector.transaction_stats();
        assert_eq!(acquire_stats.success_count, 4000);
        assert_eq!(acquire_stats.total_attempts, 4000);
        assert_eq!(txn_stats.commit_count, 4000);
        assert_eq!(txn_stats.total_transactions, 4000);
    }
}
