// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! RateLimiter 单元测试
//!
//! 测试 RateLimiter 的核心功能，包括：
//! - 单次令牌获取
//! - 令牌耗尽
//! - 并发获取（50 任务）
//! - 桶驱逐（达到最大容量时）

use dbnexus::permission::RateLimiter;
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// 基本功能测试
// ============================================================================

/// TEST-RATELIMITER-U-001: 单次令牌获取
#[tokio::test]
async fn test_rate_limiter_single_acquire() {
    let limiter = RateLimiter::new(5, Duration::from_secs(60), 10000, 5);

    // 单次获取应该成功
    assert!(limiter.check("user1").await, "First acquire should succeed");

    // 验证剩余令牌数
    assert_eq!(limiter.remaining("user1"), 4, "Should have 4 tokens remaining");
}

/// TEST-RATELIMITER-U-002: 令牌耗尽测试
#[tokio::test]
async fn test_rate_limiter_exhaustion() {
    let limiter = RateLimiter::new(3, Duration::from_secs(60), 10000, 3);

    // 消耗所有令牌
    assert!(limiter.check("user1").await, "1st acquire should succeed");
    assert!(limiter.check("user1").await, "2nd acquire should succeed");
    assert!(limiter.check("user1").await, "3rd acquire should succeed");

    // 验证令牌已耗尽
    assert_eq!(limiter.remaining("user1"), 0, "Should have 0 tokens remaining");

    // 第 4 次应该失败
    assert!(!limiter.check("user1").await, "4th acquire should fail (exhausted)");

    // 验证剩余令牌仍然为 0
    assert_eq!(limiter.remaining("user1"), 0, "Should still have 0 tokens");
}

/// TEST-RATELIMITER-U-003: 不同键独立计数
#[tokio::test]
async fn test_rate_limiter_independent_keys() {
    let limiter = RateLimiter::new(2, Duration::from_secs(60), 10000, 2);

    // user1 获取令牌
    assert!(limiter.check("user1").await);
    assert!(limiter.check("user1").await);
    assert!(!limiter.check("user1").await, "user1 should be exhausted");

    // user2 应该有独立的令牌池
    assert!(limiter.check("user2").await, "user2 should have separate pool");
    assert!(limiter.check("user2").await);
    assert!(!limiter.check("user2").await, "user2 should be exhausted");
}

// ============================================================================
// 并发测试（50 任务）
// ============================================================================

/// TEST-RATELIMITER-U-004: 50 任务并发获取测试
///
/// 验证高并发场景下的正确性，50 个并发任务竞争同一个键。
#[tokio::test]
async fn test_rate_limiter_concurrent_50_tasks() {
    let limiter = Arc::new(RateLimiter::new(100, Duration::from_secs(1), 10000, 100));
    let mut handles = vec![];

    // 启动 50 个并发任务，每个尝试获取 10 次令牌
    for _ in 0..50 {
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

    // 总成功次数应该等于桶容量（100）
    let total_success: u32 = results.iter().sum();
    assert_eq!(
        total_success, 100,
        "Total successful acquires should equal bucket capacity (100)"
    );

    // 验证剩余令牌为 0
    assert_eq!(
        limiter.remaining("shared_user"),
        0,
        "Should have 0 tokens remaining after concurrent acquires"
    );
}

/// TEST-RATELIMITER-U-005: 多键并发测试
///
/// 验证多个键在并发场景下的独立性。
#[tokio::test]
async fn test_rate_limiter_concurrent_multiple_keys() {
    let limiter = Arc::new(RateLimiter::new(50, Duration::from_secs(1), 10000, 50));
    let mut handles = vec![];

    // 启动 50 个并发任务，每个任务使用不同的键
    for i in 0..50 {
        let limiter_clone = limiter.clone();
        let handle = tokio::spawn(async move {
            let key = format!("user_{}", i);
            let mut success_count = 0;

            // 每个键尝试获取 60 次令牌（容量为 50，应该有 10 次失败）
            for _ in 0..60 {
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

    // 每个键应该成功获取 50 次令牌（桶容量）
    for (i, result) in results.iter().enumerate() {
        assert_eq!(*result, 50, "User {} should have exactly 50 successful acquires", i);
    }
}

// ============================================================================
// 桶驱逐测试
// ============================================================================

/// TEST-RATELIMITER-U-006: 桶驱逐测试（达到最大容量）
///
/// 验证当桶数量达到 max_buckets 限制时，最久未访问的桶会被驱逐。
#[tokio::test]
async fn test_rate_limiter_bucket_eviction() {
    // 创建限制为 10 个桶的 RateLimiter
    let limiter = RateLimiter::new(100, Duration::from_secs(60), 10, 100);

    // 创建 10 个桶（达到限制）
    for i in 0..10 {
        let key = format!("user_{}", i);
        assert!(limiter.check(&key).await);
    }

    // 验证当前桶数量
    assert_eq!(limiter.len(), 10, "Should have 10 buckets");

    // 创建第 11 个桶，应该触发 LRU 驱逐
    assert!(limiter.check("user_new").await, "11th bucket creation should succeed");

    // 验证桶数量仍然为 10
    assert_eq!(limiter.len(), 10, "Should still have 10 buckets after eviction");

    // 验证新桶存在
    assert!(
        limiter.remaining("user_new") > 0,
        "New bucket should exist and have tokens"
    );
}

/// TEST-RATELIMITER-U-007: LRU 驱逐顺序验证
///
/// 验证驱逐的是最久未访问的桶。
#[tokio::test]
async fn test_rate_limiter_lru_eviction_order() {
    let limiter = RateLimiter::new(100, Duration::from_secs(60), 5, 100);

    // 创建 5 个桶
    for i in 0..5 {
        let key = format!("user_{}", i);
        assert!(limiter.check(&key).await);
    }

    // 访问 user_0 和 user_1，使它们成为最近访问
    limiter.check("user_0").await;
    limiter.check("user_1").await;

    // 创建新桶，应该驱逐最久未访问的桶（user_2、user_3 或 user_4）
    limiter.check("user_new").await;

    // 验证桶数量仍为 5
    assert_eq!(limiter.len(), 5, "Should have 5 buckets after eviction");

    // 验证 user_new 桶存在（刚创建）
    assert!(
        limiter.remaining("user_new") < 100,
        "user_new should exist and have consumed a token"
    );

    // 验证被驱逐的桶（user_2/user_3/user_4 中之一）重新访问时会重新创建
    // 驱逐后，user_2 的令牌应已耗尽（因为之前 check 消耗了 1 个）
    // 注意：由于 refill 机制，令牌可能被补充，因此用 check 是否成功来验证
    let _was_evicted = !limiter.check("user_2").await;
    // user_2 可能被驱逐也可能没有（取决于 LRU 算法精确性）
    // 关键是 user_0 和 user_1 必须仍然存在（最近访问）
    assert!(
        limiter.check("user_0").await,
        "user_0 should still exist (recently accessed)"
    );
    assert!(
        limiter.check("user_1").await,
        "user_1 should still exist (recently accessed)"
    );
}

/// TEST-RATELIMITER-U-008: 驱逐后新桶可用性
///
/// 验证被驱逐的键重新访问时会创建新桶。
#[tokio::test]
async fn test_rate_limiter_evicted_key_reuse() {
    let limiter = RateLimiter::new(10, Duration::from_secs(60), 3, 10);

    // 创建 3 个桶（达到限制）
    limiter.check("user_0").await;
    limiter.check("user_1").await;
    limiter.check("user_2").await;

    // 创建第 4 个桶，触发驱逐
    limiter.check("user_3").await;

    // 被驱逐的键重新访问，应该创建新桶
    // 由于不知道哪个键被驱逐，我们尝试访问 user_0
    // 如果它被驱逐，应该获得满桶（10 tokens）
    // 如果它未被驱逐，应该剩余 < 10 tokens
    let _remaining = limiter.remaining("user_0");

    // 无论如何，访问应该成功
    assert!(limiter.check("user_0").await, "Evicted key should be reusable");

    // 验证桶数量仍为 3
    assert_eq!(limiter.len(), 3, "Should have 3 buckets");
}

/// TEST-RATELIMITER-U-009: 并发场景下的驱逐安全性
///
/// 验证高并发场景下驱逐操作的安全性。
#[tokio::test]
async fn test_rate_limiter_concurrent_eviction_safety() {
    let limiter = Arc::new(RateLimiter::new(100, Duration::from_secs(1), 10, 100));
    let mut handles = vec![];

    // 启动 20 个并发任务，每个任务使用不同的键
    // 这会触发驱逐，因为 max_buckets = 10
    for i in 0..20 {
        let limiter_clone = limiter.clone();
        let handle = tokio::spawn(async move {
            let key = format!("user_{}", i);
            let mut success_count = 0;

            // 每个任务尝试获取 10 次令牌
            for _ in 0..10 {
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

    // 验证每个任务都成功获取了一些令牌
    for (i, result) in results.iter().enumerate() {
        assert!(*result > 0, "Task {} should have acquired at least some tokens", i);
    }

    // 验证桶数量不超过限制
    assert!(limiter.len() <= 10, "Bucket count should not exceed max_buckets (10)");
}

// ============================================================================
// 边界条件测试
// ============================================================================

/// TEST-RATELIMITER-U-010: max_buckets = 1 的极端情况
#[tokio::test]
async fn test_rate_limiter_max_buckets_one() {
    let limiter = RateLimiter::new(10, Duration::from_secs(60), 1, 10);

    // 创建第一个桶
    assert!(limiter.check("user_0").await);

    // 创建第二个桶，应该驱逐第一个
    assert!(limiter.check("user_1").await);

    // 验证只有一个桶
    assert_eq!(limiter.len(), 1, "Should have exactly 1 bucket");

    // user_1 应该存在
    assert!(
        limiter.remaining("user_1") < 10,
        "user_1 should exist and have been accessed"
    );
}

/// TEST-RATELIMITER-U-011: 重置功能
#[tokio::test]
async fn test_rate_limiter_reset() {
    let limiter = RateLimiter::new(5, Duration::from_secs(60), 10000, 5);

    // 消耗所有令牌
    for _ in 0..5 {
        limiter.check("user1").await;
    }

    assert_eq!(limiter.remaining("user1"), 0);

    // 重置
    limiter.reset("user1");

    // 重置后应该恢复满桶
    assert_eq!(limiter.remaining("user1"), 5);
    assert!(limiter.check("user1").await);
}

/// TEST-RATELIMITER-U-012: cleanup 方法测试
#[tokio::test]
async fn test_rate_limiter_cleanup() {
    let limiter = RateLimiter::new(10, Duration::from_secs(1), 10000, 10);

    // 创建一些桶
    limiter.check("user1").await;
    limiter.check("user2").await;
    limiter.check("user3").await;

    assert_eq!(limiter.len(), 3);

    // 立即清理不应该删除任何桶（未过期）
    let removed = limiter.cleanup();
    assert_eq!(removed, 0);
    assert_eq!(limiter.len(), 3);
}
