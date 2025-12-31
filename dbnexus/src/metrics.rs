// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 性能指标收集模块
//!
//! 提供全面的性能指标收集功能，包括：
//! - **延迟指标**: P50、P90、P95、P99 延迟百分位
//! - **吞吐量指标**: 查询/秒、事务/秒
//! - **延迟分布**: 直方图统计
//! - **连接指标**: 连接获取延迟、连接池使用率
//! - **事务指标**: 事务持续时间、事务成功率

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// 延迟百分位数据
#[derive(Debug, Clone, Default)]
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
}

/// 直方图桶统计
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct HistogramStats {
    /// 总样本数
    pub total_samples: u64,
    /// 桶统计
    pub buckets: Vec<HistogramBucket>,
}

/// 吞吐量统计
#[derive(Debug, Clone)]
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

/// 查询统计信息（增强版）
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
}

/// 事务统计
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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

/// 延迟样本存储（使用锁保护）
#[derive(Debug)]
struct LatencyStorage {
    /// 存储的延迟样本
    samples: Vec<u64>,
    /// 最小延迟
    min: u64,
    /// 最大延迟
    max: u64,
}

impl LatencyStorage {
    fn new() -> Self {
        Self {
            samples: Vec::with_capacity(10000),
            min: u64::MAX,
            max: 0,
        }
    }

    fn record(&mut self, latency_ns: u64) {
        self.samples.push(latency_ns);
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

        let mut sorted = self.samples.clone();
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
    pub pool_total: Arc<AtomicU64>,
    /// 连接池活跃连接数
    pub pool_active: Arc<AtomicU64>,
    /// 连接池空闲连接数
    pub pool_idle: Arc<AtomicU64>,

    /// 连接错误计数
    pub connection_errors: Arc<AtomicU64>,
    /// 查询错误计数
    pub query_errors: Arc<AtomicU64>,

    /// 连接获取指标
    connection_acquire: Arc<RwLock<ConnectionAcquireMetricsInner>>,
    /// 事务指标
    transaction: Arc<RwLock<TransactionMetricsInner>>,

    /// 慢查询记录（最近 N 条）
    slow_queries: Arc<RwLock<Vec<SlowQueryRecord>>>,
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
}

impl ConnectionAcquireMetricsInner {
    fn new() -> Self {
        Self {
            total_attempts: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            timeout_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
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
        }
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
            connection_acquire: Arc::new(RwLock::new(ConnectionAcquireMetricsInner::new())),
            transaction: Arc::new(RwLock::new(TransactionMetricsInner::new())),
            slow_queries: Arc::new(RwLock::new(Vec::new())),
            slow_query_config: Arc::new(RwLock::new(SlowQueryConfig {
                threshold_ms: 1000,
                enabled: true,
            })),
            max_slow_queries: 100,
            start_time: Instant::now(),
        }
    }

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
            slow.push(SlowQueryRecord {
                query_type: query_type.to_string(),
                duration_ms,
                timestamp: time::OffsetDateTime::now_utc(),
            });
            while slow.len() > self.max_slow_queries {
                slow.remove(0);
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

        for (_, m) in map.iter() {
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
        self.slow_queries.read().clone()
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
        self.connection_acquire.write().record_success();
    }

    /// 记录连接获取超时
    pub fn record_connection_acquire_timeout(&self) {
        self.connection_acquire.write().record_timeout();
    }

    /// 记录连接获取失败
    pub fn record_connection_acquire_failure(&self) {
        self.connection_acquire.write().record_failure();
    }

    /// 获取连接获取统计
    pub fn connection_acquire_stats(&self) -> ConnectionAcquireStats {
        self.connection_acquire.read().stats()
    }

    /// 记录事务提交
    pub fn record_transaction_commit(&self) {
        self.transaction.write().record_commit();
    }

    /// 记录事务回滚
    pub fn record_transaction_rollback(&self) {
        self.transaction.write().record_rollback();
    }

    /// 记录事务失败
    pub fn record_transaction_failure(&self) {
        self.transaction.write().record_failure();
    }

    /// 获取事务统计
    pub fn transaction_stats(&self) -> TransactionStats {
        self.transaction.read().stats()
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

        let mut acquire = self.connection_acquire.write();
        *acquire = ConnectionAcquireMetricsInner::new();

        let mut txn = self.transaction.write();
        *txn = TransactionMetricsInner::new();
    }

    /// 导出为 Prometheus 格式
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        let now = time::OffsetDateTime::now_utc();

        let uptime_seconds = self.uptime().as_secs_f64();
        output.push_str("# TYPE dbnexus_uptime gauge\n");
        output.push_str(&format!("dbnexus_uptime_seconds {:.3}\n", uptime_seconds));

        // 连接池指标
        output.push_str("# TYPE dbnexus_pool_connections gauge\n");
        output.push_str(&format!(
            "dbnexus_pool_connections_total {}\n",
            self.pool_total.load(Ordering::SeqCst)
        ));
        output.push_str(&format!(
            "dbnexus_pool_connections_active {}\n",
            self.pool_active.load(Ordering::SeqCst)
        ));
        output.push_str(&format!(
            "dbnexus_pool_connections_idle {}\n",
            self.pool_idle.load(Ordering::SeqCst)
        ));
        output.push_str(&format!(
            "dbnexus_pool_connections_utilization {:.4}\n",
            self.pool_status().utilization_rate()
        ));

        // 错误指标
        output.push_str("# TYPE dbnexus_errors counter\n");
        output.push_str(&format!(
            "dbnexus_connection_errors_total {}\n",
            self.connection_errors.load(Ordering::SeqCst)
        ));
        output.push_str(&format!(
            "dbnexus_query_errors_total {}\n",
            self.query_errors.load(Ordering::SeqCst)
        ));

        // 连接获取指标
        let acquire_stats = self.connection_acquire_stats();
        output.push_str("# TYPE dbnexus_connection_acquire counter\n");
        output.push_str(&format!(
            "dbnexus_connection_acquire_total {}\n",
            acquire_stats.total_attempts
        ));
        output.push_str(&format!(
            "dbnexus_connection_acquire_timeout_total {}\n",
            acquire_stats.timeout_count
        ));
        output.push_str(&format!(
            "dbnexus_connection_acquire_failure_total {}\n",
            acquire_stats.failure_count
        ));

        // 事务指标
        let txn_stats = self.transaction_stats();
        output.push_str("# TYPE dbnexus_transactions counter\n");
        output.push_str(&format!(
            "dbnexus_transactions_total {}\n",
            txn_stats.total_transactions
        ));
        output.push_str(&format!(
            "dbnexus_transactions_commit_total {}\n",
            txn_stats.commit_count
        ));
        output.push_str(&format!(
            "dbnexus_transactions_rollback_total {}\n",
            txn_stats.rollback_count
        ));
        output.push_str(&format!(
            "dbnexus_transactions_failure_total {}\n",
            txn_stats.failure_count
        ));
        output.push_str(&format!(
            "dbnexus_transactions_success_rate {:.2}\n",
            txn_stats.success_rate
        ));

        // 查询指标
        let stats = self.all_query_stats();
        for (query_type, stat) in stats {
            let type_label = query_type.to_lowercase();

            output.push_str(&format!(
                "# TYPE dbnexus_queries_total counter\ndbnexus_queries_total{{type=\"{}\"}} {}\n",
                type_label, stat.count
            ));

            output.push_str("# TYPE dbnexus_query_throughput gauge\n");
            output.push_str(&format!(
                "dbnexus_query_throughput_qps{{type=\"{}\"}} {:.2}\n",
                type_label, stat.throughput.avg_qps
            ));

            // 延迟百分位
            output.push_str("# TYPE dbnexus_query_latency_seconds gauge\n");
            output.push_str(&format!(
                "dbnexus_query_latency_p50_seconds{{type=\"{}\"}} {:.6}\n",
                type_label,
                stat.latency_percentiles.p50().as_secs_f64()
            ));
            output.push_str(&format!(
                "dbnexus_query_latency_p90_seconds{{type=\"{}\"}} {:.6}\n",
                type_label,
                stat.latency_percentiles.p90().as_secs_f64()
            ));
            output.push_str(&format!(
                "dbnexus_query_latency_p95_seconds{{type=\"{}\"}} {:.6}\n",
                type_label,
                stat.latency_percentiles.p95().as_secs_f64()
            ));
            output.push_str(&format!(
                "dbnexus_query_latency_p99_seconds{{type=\"{}\"}} {:.6}\n",
                type_label,
                stat.latency_percentiles.p99().as_secs_f64()
            ));
        }

        // 总吞吐量
        let total = self.total_throughput();
        output.push_str("# TYPE dbnexus_total_throughput gauge\n");
        output.push_str(&format!("dbnexus_total_qps {:.2}\n", total.avg_qps));
        output.push_str(&format!("dbnexus_total_operations {}\n", total.total_operations));
        output.push_str(&format!("dbnexus_error_rate {:.4}\n", total.error_rate));

        output.push_str("# TYPE dbnexus_metrics_timestamp gauge\n");
        output.push_str(&format!("dbnexus_metrics_timestamp {}\n", now.unix_timestamp()));

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-040: 延迟百分位计算测试
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
}
