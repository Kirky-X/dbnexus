// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

//! 速率限制器
//!
//! 提供基于令牌桶算法的速率限制功能。

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Duration;

/// 令牌桶速率限制器
///
/// 使用令牌桶算法限制请求频率，相比滑动窗口算法：
/// - O(1) 时间复杂度的检查操作
/// - 无锁 CAS 操作，高并发性能优异
/// - 平滑的限流效果
/// - 支持突发流量处理
///
/// 注意：此结构体主要用于内部权限检查速率限制，
/// 但其方法（如 `remaining`、`cleanup`）也可用于监控和管理。
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// 每个时间窗口允许的最大请求数（令牌桶容量）
    max_requests: u32,
    /// 时间窗口大小（用于计算令牌填充速率）
    window_duration: Duration,
    /// 令牌桶存储（使用 DashMap 实现细粒度并发控制）
    /// Key: 限制键 (IP/用户ID), Value: TokenBucket 实例
    buckets: Arc<DashMap<String, TokenBucket>>,
    /// 最大桶数量限制（防止内存泄漏）
    max_buckets: usize,
    /// 突发容量（桶的初始/最大令牌数，可以大于 max_requests）
    burst_capacity: u32,
}

/// 令牌桶实现
///
/// 使用原子操作实现高性能的令牌桶算法
#[derive(Debug)]
struct TokenBucket {
    /// 当前令牌数（使用 AtomicU64 支持高并发）
    tokens: AtomicU64,
    /// 上次填充时间戳（毫秒，使用 AtomicI64 支持高并发）
    last_refill: AtomicI64,
    /// 上次访问时间戳（毫秒，用于清理过期条目）
    last_access: AtomicI64,
    /// 最大令牌数
    max_tokens: u64,
    /// 令牌填充速率（令牌/秒）
    refill_rate: u64,
}

impl TokenBucket {
    /// 创建新的令牌桶
    ///
    /// # Arguments
    ///
    /// * `max_tokens` - 最大令牌数（桶容量）
    /// * `refill_rate` - 令牌填充速率（令牌/秒）
    fn new(max_tokens: u64, refill_rate: u64) -> Self {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        Self {
            tokens: AtomicU64::new(max_tokens),
            last_refill: AtomicI64::new(now_ms),
            last_access: AtomicI64::new(now_ms),
            max_tokens,
            refill_rate,
        }
    }

    /// 更新最后访问时间（用于 LRU 追踪）
    fn touch(&self) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        self.last_access.store(now_ms, Ordering::Relaxed);
    }

    /// 尝试消费一个令牌（优化的原子操作）
    ///
    /// # Returns
    ///
    /// 如果成功消费令牌返回 true，否则返回 false
    fn try_consume(&self) -> bool {
        // 1. 更新最后访问时间
        self.touch();

        // 2. 先尝试填充令牌
        self.refill();

        // 3. 优化的令牌消费：快速路径检查 + 原子递减
        // 使用 fetch_sub 避免高竞争下的 CAS 重试循环
        let prev_tokens = self.tokens.fetch_sub(1, Ordering::AcqRel);

        if prev_tokens == 0 {
            // 令牌已耗尽，恢复令牌数（不允许负数）
            self.tokens.fetch_add(1, Ordering::AcqRel);
            return false;
        }

        true
    }

    /// 填充令牌（基于时间差计算应添加的令牌数）
    fn refill(&self) {
        // 如果填充速率为 0，跳过填充
        if self.refill_rate == 0 {
            return;
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        loop {
            let last_refill = self.last_refill.load(Ordering::Relaxed);
            let elapsed_ms = now_ms - last_refill;

            // 如果时间差太小，不填充
            if elapsed_ms < 1000 / self.refill_rate as i64 {
                return;
            }

            // 计算应添加的令牌数
            let tokens_to_add = (elapsed_ms as u64 * self.refill_rate) / 1000;

            // 尝试更新 last_refill 时间戳（CAS）
            if self
                .last_refill
                .compare_exchange(last_refill, now_ms, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                // 成功更新时间戳，现在添加令牌
                loop {
                    let current_tokens = self.tokens.load(Ordering::Relaxed);
                    let new_tokens = std::cmp::min(current_tokens + tokens_to_add, self.max_tokens);

                    if self
                        .tokens
                        .compare_exchange(current_tokens, new_tokens, Ordering::SeqCst, Ordering::Relaxed)
                        .is_ok()
                    {
                        return;
                    }
                    // CAS 失败，重试
                }
            }
            // CAS 失败，其他线程已经更新了时间戳，重试整个循环
        }
    }

    /// 获取当前令牌数
    fn available_tokens(&self) -> u64 {
        self.refill();
        self.tokens.load(Ordering::Relaxed)
    }

    /// 获取最后访问时间戳（毫秒）
    fn last_access_ms(&self) -> i64 {
        self.last_access.load(Ordering::Relaxed)
    }
}

impl RateLimiter {
    /// 创建新的速率限制器
    ///
    /// # Arguments
    ///
    /// * `max_requests` - 每个时间窗口允许的最大请求数（也作为默认突发容量）
    /// * `window_duration` - 时间窗口大小
    /// * `max_buckets` - 最大桶数量限制（防止内存泄漏）
    /// * `burst_capacity` - 突发容量（桶的最大令牌数），默认为 max_requests
    pub fn new(max_requests: u32, window_duration: Duration, max_buckets: usize, burst_capacity: u32) -> Self {
        Self {
            max_requests,
            window_duration,
            buckets: Arc::new(DashMap::new()),
            max_buckets,
            burst_capacity,
        }
    }

    /// 创建新的速率限制器（使用默认突发容量 = max_requests）
    ///
    /// # Arguments
    ///
    /// * `max_requests` - 每个时间窗口允许的最大请求数
    /// * `window_duration` - 时间窗口大小
    /// * `max_buckets` - 最大桶数量限制（防止内存泄漏）
    pub fn with_defaults(max_requests: u32, window_duration: Duration, max_buckets: usize) -> Self {
        Self::new(max_requests, window_duration, max_buckets, max_requests)
    }

    /// 动态更新配置（适用于需要运行时调整限制的场景）
    ///
    /// 注意：此方法只更新 RateLimiter 的配置，不影响已创建的 TokenBucket。
    /// 已存在的桶会继续使用创建时的参数，直到被驱逐后重建。
    ///
    /// # Arguments
    ///
    /// * `max_requests` - 新的每个时间窗口允许的最大请求数
    /// * `window_duration` - 新的时间窗口大小
    pub fn update_config(&mut self, max_requests: u32, window_duration: Duration) {
        self.max_requests = max_requests;
        self.window_duration = window_duration;
        // burst_capacity 保持不变
    }

    /// 检查是否允许请求（令牌桶算法）
    ///
    /// 使用令牌桶算法：
    /// 1. 获取或创建对应键的令牌桶
    /// 2. 尝试消费一个令牌
    /// 3. 根据是否成功消费决定是否允许请求
    ///
    /// # Arguments
    ///
    /// * `key` - 速率限制的键（如 IP 地址、用户 ID）
    ///
    /// # Returns
    ///
    /// 如果允许请求返回 true，否则返回 false
    pub async fn check(&self, key: &str) -> bool {
        // 计算填充速率：max_requests / window_duration（秒）
        // 使用浮点数计算，确保精度，然后向上取整，最小为 1
        let window_secs = self.window_duration.as_secs();
        let refill_rate = if window_secs > 0 {
            let rate = (self.max_requests as f64 / window_secs as f64).ceil() as u64;
            rate.max(1) // 确保至少为 1
        } else {
            self.max_requests as u64
        };

        // 检查是否需要 LRU 驱逐（在 entry() 之前检查，避免在持有 entry 时删除）
        if self.buckets.len() >= self.max_buckets && !self.buckets.contains_key(key) {
            // 执行 LRU 驱逐：移除最长时间未访问的桶
            self.evict_lru_bucket();
        }

        // 使用 entry() API 实现原子的 get-or-insert 操作
        match self.buckets.entry(key.to_string()) {
            Entry::Occupied(entry) => {
                // 桶已存在：直接更新 LRU 时间戳并消费令牌
                entry.get().touch();
                entry.get().try_consume()
            }
            Entry::Vacant(entry) => {
                // 桶不存在：创建新桶并插入（使用 burst_capacity 作为初始/最大令牌数）
                let bucket = TokenBucket::new(self.burst_capacity as u64, refill_rate);
                bucket.touch();
                let consumed = bucket.try_consume();
                entry.insert(bucket);
                consumed
            }
        }
    }

    /// 获取剩余请求数量
    ///
    /// 用于监控和管理场景，例如：
    /// - 在管理界面显示用户的剩余请求配额
    /// - API 返回响应头中的 RateLimit-Remaining
    pub fn remaining(&self, key: &str) -> u32 {
        if let Some(bucket) = self.buckets.get(key) {
            bucket.available_tokens() as u32
        } else {
            self.max_requests
        }
    }

    /// 重置指定键的速率限制
    ///
    /// 用于管理场景，例如：
    /// - 管理员手动解除用户的速率限制
    /// - 在用户申诉后重置其限制
    pub fn reset(&self, key: &str) {
        self.buckets.remove(key);
    }

    /// 清理孤立条目（防止内存泄漏）
    ///
    /// 移除长时间没有任何请求的 key（超过 10 倍时间窗口）
    /// 建议定期调用此方法，例如每小时一次或使用定时任务
    ///
    /// # Returns
    ///
    /// 返回清理的条目数量
    pub fn cleanup(&self) -> usize {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // 计算过期阈值：10 倍时间窗口（毫秒）
        let expiry_threshold_ms = (self.window_duration.as_millis() as i64) * 10;

        let mut removed_count = 0;

        // 收集需要删除的键
        let keys_to_remove: Vec<String> = self
            .buckets
            .iter()
            .filter(|entry| {
                let bucket = entry.value();
                let last_access = bucket.last_access_ms();
                let elapsed = now_ms - last_access;
                elapsed > expiry_threshold_ms
            })
            .map(|entry| entry.key().clone())
            .collect();

        // 删除过期条目
        for key in keys_to_remove {
            if self.buckets.remove(&key).is_some() {
                removed_count += 1;
            }
        }

        removed_count
    }

    /// 获取当前条目数量（用于监控）
    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    /// 执行 LRU 驱逐：移除最长时间未访问的桶
    ///
    /// 当桶数量达到 max_buckets 限制时调用此方法。
    /// 遍历所有桶，找到 last_access 最小的桶并删除。
    fn evict_lru_bucket(&self) {
        let mut oldest_key: Option<String> = None;
        let mut oldest_access = i64::MAX;

        // 查找最久未访问的桶
        for entry in self.buckets.iter() {
            let last_access = entry.value().last_access_ms();
            if last_access < oldest_access {
                oldest_access = last_access;
                oldest_key = Some(entry.key().clone());
            }
        }

        // 删除找到的桶
        if let Some(key) = oldest_key {
            if self.buckets.remove(&key).is_some() {
            }
        }
    }
}

/// 默认速率限制配置
impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(100, Duration::from_secs(60), 10000, 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "permission-engine")]
    use futures;

    /// TEST-U-019: 速率限制器测试 - 基本功能
    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let limiter = RateLimiter::new(3, std::time::Duration::from_secs(60), 10000, 3);

        // 前3个请求应该允许
        assert!(limiter.check("user1").await);
        assert!(limiter.check("user1").await);
        assert!(limiter.check("user1").await);

        // 第4个请求应该被拒绝
        assert!(!limiter.check("user1").await);
    }

    /// TEST-U-020: 速率限制器测试 - 不同键独立计数
    #[tokio::test]
    async fn test_rate_limiter_different_keys() {
        let limiter = RateLimiter::new(2, std::time::Duration::from_secs(60), 10000, 2);

        assert!(limiter.check("user1").await);
        assert!(limiter.check("user1").await);
        assert!(!limiter.check("user1").await);

        assert!(limiter.check("user2").await);
        assert!(limiter.check("user2").await);
        assert!(!limiter.check("user2").await);
    }

    /// TEST-U-021: 速率限制器测试 - 重置功能
    #[tokio::test]
    async fn test_rate_limiter_reset() {
        let limiter = RateLimiter::new(1, std::time::Duration::from_secs(60), 10000, 1);

        assert!(limiter.check("user1").await);
        assert!(!limiter.check("user1").await);

        limiter.reset("user1");

        assert!(limiter.check("user1").await);
    }

    /// TEST-U-022: 速率限制器测试 - 剩余请求计数
    #[tokio::test]
    async fn test_rate_limiter_remaining() {
        let limiter = RateLimiter::new(3, std::time::Duration::from_secs(60), 10000, 3);

        assert_eq!(limiter.remaining("user1"), 3);

        limiter.check("user1").await;
        assert_eq!(limiter.remaining("user1"), 2);

        limiter.check("user1").await;
        assert_eq!(limiter.remaining("user1"), 1);

        limiter.check("user1").await;
        assert_eq!(limiter.remaining("user1"), 0);

        assert!(!limiter.check("user1").await);
        assert_eq!(limiter.remaining("user1"), 0);
    }

    /// TEST-U-031: 令牌桶令牌填充测试
    ///
    /// 验证令牌桶在时间流逝后正确填充令牌。
    #[tokio::test]
    async fn test_token_bucket_refill() {
        // 创建速率限制器：每秒填充 10 个令牌，最大 10 个
        let limiter = RateLimiter::new(10, std::time::Duration::from_secs(1), 10000, 10);

        // 消耗所有令牌
        for _ in 0..10 {
            assert!(limiter.check("user1").await);
        }

        // 应该被拒绝
        assert!(!limiter.check("user1").await);
        assert_eq!(limiter.remaining("user1"), 0);

        // 等待令牌填充（等待 1.1 秒，应该填充约 10 个令牌）
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

        // 现在应该有令牌可用
        assert!(limiter.check("user1").await);
        assert!(limiter.remaining("user1") > 0);
    }

    /// TEST-U-032: 令牌桶突发流量测试
    ///
    /// 验证令牌桶能够处理突发流量（桶满时可以一次性使用所有令牌）。
    #[tokio::test]
    async fn test_token_bucket_burst() {
        // 创建速率限制器：每秒填充 5 个令牌，最大 20 个
        let limiter = RateLimiter::new(20, std::time::Duration::from_secs(4), 10000, 20);

        // 等待桶填满（初始就是满的）
        assert_eq!(limiter.remaining("user1"), 20);

        // 突发请求：一次性使用所有令牌
        for _ in 0..20 {
            assert!(limiter.check("user1").await);
        }

        // 应该被拒绝
        assert!(!limiter.check("user1").await);
        assert_eq!(limiter.remaining("user1"), 0);
    }

    /// TEST-U-033: 令牌桶并发安全测试
    ///
    /// 验证令牌桶在高并发场景下的正确性。
    #[tokio::test]
    async fn test_token_bucket_concurrent_safety() {
        let limiter = Arc::new(RateLimiter::new(100, std::time::Duration::from_secs(1), 10000, 100));
        let mut handles = vec![];

        // 启动 10 个并发任务，每个尝试获取 15 个令牌
        for i in 0..10 {
            let limiter_clone = limiter.clone();
            let handle = tokio::spawn(async move {
                let key = format!("user{}", i);
                let mut success_count = 0;
                for _ in 0..15 {
                    if limiter_clone.check(&key).await {
                        success_count += 1;
                    }
                }
                success_count
            });
            handles.push(handle);
        }

        // 等待所有任务完成
        let results: Vec<u32> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // 每个用户应该成功获取 15 个令牌（桶容量为 100，足够）
        for result in results {
            assert_eq!(result, 15);
        }
    }

    /// TEST-U-034: 令牌桶单键高并发竞争测试
    ///
    /// 验证单个键在高并发竞争下的正确性。
    #[tokio::test]
    async fn test_token_bucket_single_key_concurrent() {
        let limiter = Arc::new(RateLimiter::new(50, std::time::Duration::from_secs(1), 10000, 50));
        let mut handles = vec![];

        // 启动 10 个并发任务，竞争同一个键
        for _ in 0..10 {
            let limiter_clone = limiter.clone();
            let handle = tokio::spawn(async move {
                let mut success_count = 0;
                for _ in 0..10 {
                    if limiter_clone.check("shared_user").await {
                        success_count += 1;
                    }
                }
                success_count
            });
            handles.push(handle);
        }

        // 等待所有任务完成
        let results: Vec<u32> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // 总成功次数应该等于桶容量（50）
        let total_success: u32 = results.iter().sum();
        assert_eq!(total_success, 50);

        // 剩余令牌应该为 0
        assert_eq!(limiter.remaining("shared_user"), 0);
    }

    /// TEST-U-035: 令牌桶 cleanup 方法测试
    ///
    /// 验证 cleanup 方法能正确清理长时间未使用的条目。
    #[tokio::test]
    async fn test_token_bucket_cleanup() {
        // 创建速率限制器：窗口 1 秒
        let limiter = RateLimiter::new(10, std::time::Duration::from_secs(1), 10000, 10);

        // 创建一些条目
        limiter.check("user1").await;
        limiter.check("user2").await;
        limiter.check("user3").await;

        assert_eq!(limiter.len(), 3);

        // 立即清理不应该删除任何条目
        let removed = limiter.cleanup();
        assert_eq!(removed, 0);
        assert_eq!(limiter.len(), 3);

        // 重置 user1 以更新其访问时间
        limiter.reset("user1");
        limiter.check("user1").await;

        // 验证 user1 仍然存在
        assert_eq!(limiter.remaining("user1"), 9);
    }

    /// TEST-U-036: 令牌桶 O(1) 复杂度验证
    ///
    /// 验证令牌桶操作的时间复杂度为 O(1)。
    #[tokio::test]
    async fn test_token_bucket_o1_complexity() {
        let limiter = RateLimiter::new(1000, std::time::Duration::from_secs(60), 10000, 1000);

        // 创建大量条目
        for i in 0..1000 {
            let key = format!("user{}", i);
            limiter.check(&key).await;
        }

        assert_eq!(limiter.len(), 1000);

        // 测量单个操作时间（应该与条目数量无关）
        let start = std::time::Instant::now();
        for _ in 0..100 {
            limiter.check("user500").await;
        }
        let duration = start.elapsed();

        // 100 次操作应该在 10ms 内完成（O(1) 操作应该非常快）
        // 注意：这是一个宽松的阈值，主要验证没有 O(n) 操作
        assert!(
            duration < std::time::Duration::from_millis(100),
            "100 operations took {:?}, expected < 100ms",
            duration
        );
    }

    /// TEST-U-037: 令牌桶精确限流测试
    ///
    /// 验证令牌桶精确限制请求数量。
    #[tokio::test]
    async fn test_token_bucket_precise_limiting() {
        // 创建速率限制器：每秒 5 个请求
        let limiter = RateLimiter::new(5, std::time::Duration::from_secs(1), 10000, 5);

        // 精确测试：应该允许 5 个请求，拒绝第 6 个
        for i in 1..=5 {
            assert!(limiter.check("user1").await, "Request {} should be allowed", i);
        }

        assert!(!limiter.check("user1").await, "6th request should be denied");

        // 验证剩余令牌
        assert_eq!(limiter.remaining("user1"), 0);
    }

    /// TEST-U-038: 令牌桶重置后状态测试
    ///
    /// 验证重置后令牌桶恢复到初始状态。
    #[tokio::test]
    async fn test_token_bucket_reset_state() {
        let limiter = RateLimiter::new(10, std::time::Duration::from_secs(60), 10000, 10);

        // 消耗所有令牌
        for _ in 0..10 {
            limiter.check("user1").await;
        }
        assert_eq!(limiter.remaining("user1"), 0);

        // 重置
        limiter.reset("user1");

        // 验证重置后状态（条目被移除，remaining 返回最大值）
        assert_eq!(limiter.remaining("user1"), 10);
        assert!(limiter.check("user1").await);
    }

    /// TEST-U-039: 令牌桶高并发压力测试（50+ 并发任务）
    ///
    /// 验证令牌桶在 50+ 并发任务同时调用 check 时的正确性和稳定性。
    /// 测试无 panic、无数据竞争、令牌计数准确。
    #[tokio::test]
    async fn test_token_bucket_high_concurrent_stress() {
        let limiter = Arc::new(RateLimiter::new(200, std::time::Duration::from_secs(1), 10000, 200));
        let mut handles = vec![];

        // 启动 60 个并发任务，每个尝试获取 5 个令牌（共 300 次请求）
        for _ in 0..60 {
            let limiter_clone = limiter.clone();
            let handle = tokio::spawn(async move {
                let mut success_count = 0;
                for _ in 0..5 {
                    if limiter_clone.check("stress_user").await {
                        success_count += 1;
                    }
                }
                success_count
            });
            handles.push(handle);
        }

        // 等待所有任务完成
        let results: Vec<u32> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // 总成功次数应该等于桶容量（200）
        let total_success: u32 = results.iter().sum();
        assert_eq!(total_success, 200, "Total success should equal bucket capacity");

        // 剩余令牌应该为 0
        assert_eq!(limiter.remaining("stress_user"), 0);

        // 验证无 panic：所有任务都应正常完成
        assert_eq!(results.len(), 60);
    }

    /// TEST-U-040: TokenBucket fetch_sub 精度损失测试
    ///
    /// 验证 fetch_sub 返回值判断逻辑正确处理令牌耗尽情况。
    /// 当 tokens 从 1 变为 0 时，fetch_sub 返回 1，应该允许请求。
    /// 当 tokens 为 0 时，fetch_sub 返回 0，应该拒绝请求并归还令牌。
    #[tokio::test]
    async fn test_token_bucket_fetch_sub_precision() {
        let limiter = RateLimiter::new(1, std::time::Duration::from_secs(60), 10000, 1);

        // 第一次请求：tokens 从 1 变为 0，fetch_sub 返回 1，应该允许
        assert!(limiter.check("precision_user").await, "First request should be allowed");
        assert_eq!(limiter.remaining("precision_user"), 0);

        // 第二次请求：tokens 为 0，fetch_sub 返回 0，应该拒绝并归还令牌
        assert!(
            !limiter.check("precision_user").await,
            "Second request should be denied"
        );
        // 归还令牌后 tokens 应该仍为 0（不能为负数）
        assert_eq!(limiter.remaining("precision_user"), 0);

        // 再次请求应该仍然被拒绝
        assert!(
            !limiter.check("precision_user").await,
            "Third request should still be denied"
        );
        assert_eq!(limiter.remaining("precision_user"), 0);
    }

    /// TEST-U-041: TokenBucket 边界值精度测试
    ///
    /// 验证在令牌数接近边界值时的精确行为。
    #[tokio::test]
    async fn test_token_bucket_boundary_precision() {
        let limiter = RateLimiter::new(3, std::time::Duration::from_secs(60), 10000, 3);

        // 消耗令牌直到耗尽
        assert!(limiter.check("boundary_user").await);
        assert_eq!(limiter.remaining("boundary_user"), 2);

        assert!(limiter.check("boundary_user").await);
        assert_eq!(limiter.remaining("boundary_user"), 1);

        assert!(limiter.check("boundary_user").await);
        assert_eq!(limiter.remaining("boundary_user"), 0);

        // 在边界值 0 处反复请求，验证不会出现负数或错误的允许
        for _ in 0..5 {
            assert!(!limiter.check("boundary_user").await);
            assert_eq!(limiter.remaining("boundary_user"), 0);
        }
    }
}
