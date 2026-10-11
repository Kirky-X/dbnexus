// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 速率限制器示例
//!
//! 展示如何使用 RateLimiter 基于令牌桶算法进行速率限制：
//! - 创建 RateLimiter 实例（new / with_defaults / default）
//! - 异步 check() 检查请求是否允许
//! - 多键独立计数
//! - 令牌耗尽场景演示
//! - remaining() 查询剩余配额
//! - reset() 重置指定键
//! - cleanup() 清理过期条目
//! - 突发流量处理
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example rate_limiter --features "permission"
//! ```

use dbnexus::access::RateLimiter;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("⏱  DBNexus rate limiter example");
    println!("========================================\n");

    // ============================================
    // 1. 基本使用：new() 创建并消耗令牌
    // ============================================
    println!("--- Scenario 1: basic usage ---");
    let limiter = RateLimiter::new(5, Duration::from_secs(60), 10000, 5);
    println!("  ✓ RateLimiter created: max_requests=5, window=60s, burst=5");

    // 前 5 个请求应该被允许
    println!("\n  send 5 requests (all should be allowed):");
    for i in 1..=5 {
        let allowed = limiter.check("user_001").await;
        println!(
            "    request #{}: allowed={}, remaining={}",
            i,
            allowed,
            limiter.remaining("user_001")
        );
    }

    // 第 6 个请求应该被拒绝
    println!("\n  send the 6th request (should be rejected):");
    let allowed = limiter.check("user_001").await;
    println!(
        "    request #6: allowed={}, remaining={}",
        allowed,
        limiter.remaining("user_001")
    );
    println!();

    // ============================================
    // 2. 不同键独立计数
    // ============================================
    println!("--- Scenario 2: independent counting per key ---");
    let limiter2 = RateLimiter::with_defaults(3, Duration::from_secs(60), 10000);
    println!("  ✓ RateLimiter created: max_requests=3 (with_defaults)");

    // user_A 消耗 2 个令牌
    println!("\n  user_A consumes 2 tokens:");
    for _ in 0..2 {
        let _ = limiter2.check("user_A").await;
    }
    println!("    user_A remaining = {}", limiter2.remaining("user_A"));

    // user_B 消耗 1 个令牌
    println!("\n  user_B consumes 1 token:");
    let _ = limiter2.check("user_B").await;
    println!("    user_B remaining = {}", limiter2.remaining("user_B"));

    // 验证 user_A 仍有 1 个令牌
    println!("\n  verify user_A still has 1 token (independent counting):");
    let allowed_a = limiter2.check("user_A").await;
    println!(
        "    user_A requests: allowed={}, remaining={}",
        allowed_a,
        limiter2.remaining("user_A")
    );
    let denied_a = limiter2.check("user_A").await;
    println!(
        "    user_A requests again: allowed={} (should be rejected)",
        denied_a
    );
    println!(
        "    user_B remaining = {} (unaffected)",
        limiter2.remaining("user_B")
    );
    println!();

    // ============================================
    // 3. reset() 重置指定键
    // ============================================
    println!("--- Scenario 3: reset() resets a given key ---");
    let limiter3 = RateLimiter::new(2, Duration::from_secs(60), 10000, 2);
    println!("  ✓ RateLimiter created: max_requests=2");

    // 消耗所有令牌
    let _ = limiter3.check("user_reset").await;
    let _ = limiter3.check("user_reset").await;
    let denied = limiter3.check("user_reset").await;
    println!(
        "\n  3rd request after consuming 2 tokens: allowed={} (should be rejected)",
        denied
    );
    println!("    remaining = {}", limiter3.remaining("user_reset"));

    // 重置
    println!("\n  executing limiter.reset(\"user_reset\")");
    limiter3.reset("user_reset");
    println!(
        "    remaining = {} (after reset)",
        limiter3.remaining("user_reset")
    );

    // 验证可再次请求
    let allowed = limiter3.check("user_reset").await;
    println!(
        "\n  request after reset: allowed={} (should be allowed)",
        allowed
    );
    println!();

    // ============================================
    // 4. 突发流量（burst_capacity > max_requests）
    // ============================================
    println!("--- Scenario 4: burst traffic handling ---");
    // refill_rate = 1/sec (max_requests=1 / window=1s), burst=10
    let limiter4 = RateLimiter::new(1, Duration::from_secs(1), 10000, 10);
    println!("  ✓ RateLimiter created: max_requests=1/s, burst_capacity=10");

    // 突发 10 个请求
    println!("\n  burst of 10 requests:");
    let mut allowed_count = 0;
    for _ in 1..=10 {
        if limiter4.check("burst_user").await {
            allowed_count += 1;
        }
    }
    println!(
        "    {} of 10 requests allowed (burst capacity)",
        allowed_count
    );

    // 第 11 个应被拒绝
    let denied = limiter4.check("burst_user").await;
    println!(
        "    request #11: allowed={} (burst capacity exhausted)",
        denied
    );
    println!("    remaining = {}", limiter4.remaining("burst_user"));
    println!();

    // ============================================
    // 5. 令牌桶填充（等待后令牌恢复）
    // ============================================
    println!("--- Scenario 5: token bucket refill ---");
    let limiter5 = RateLimiter::new(10, Duration::from_secs(1), 10000, 10);
    println!("  ✓ RateLimiter created: max_requests=10/s, burst=10");

    // 消耗所有令牌
    println!("\n  consume all 10 tokens:");
    for _ in 0..10 {
        let _ = limiter5.check("refill_user").await;
    }
    println!(
        "    remaining = {} (exhausted)",
        limiter5.remaining("refill_user")
    );

    // 等待 1.1 秒让令牌填充
    println!("\n  wait 1.1 seconds for tokens to refill...");
    tokio::time::sleep(Duration::from_millis(1100)).await;

    let allowed = limiter5.check("refill_user").await;
    println!(
        "    request after waiting: allowed={} (tokens refilled)",
        allowed
    );
    println!("    remaining = {}", limiter5.remaining("refill_user"));
    println!();

    // ============================================
    // 6. cleanup() 清理孤立条目
    // ============================================
    println!("--- Scenario 6: cleanup() removes stale entries ---");
    let limiter6 = RateLimiter::new(10, Duration::from_millis(100), 10000, 10);
    println!("  ✓ RateLimiter created: window=100ms (short window for the demo)");

    // 创建多个条目
    println!("\n  create 5 entries:");
    for i in 0..5 {
        let key = format!("cleanup_user_{}", i);
        let _ = limiter6.check(&key).await;
    }
    println!("    len = {} (entry count)", limiter6.len());
    println!("    is_empty = {}", limiter6.is_empty());

    // 立即清理不应删除任何条目
    let removed = limiter6.cleanup();
    println!("\n  immediate cleanup: removed = {} (not expired)", removed);
    println!("    len = {}", limiter6.len());

    // 等待 1.5 秒让条目过期（10 倍窗口 = 1 秒）
    println!("\n  wait 1.5 seconds for entries to expire (expiry threshold = 10 * 100ms = 1s)...");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let removed = limiter6.cleanup();
    println!("    cleanup: removed = {} (expired)", removed);
    println!("    len = {} (after cleanup)", limiter6.len());
    println!("    is_empty = {}", limiter6.is_empty());
    println!();

    // ============================================
    // 7. 模拟高频率权限检查场景
    // ============================================
    println!("--- Scenario 7: simulated high-frequency permission checks ---");
    let limiter7 = RateLimiter::new(20, Duration::from_secs(60), 10000, 20);
    println!("  ✓ RateLimiter created: max_requests=20/min (simulated API rate limit)");

    let mut stats = Stats {
        allowed: 0,
        denied: 0,
    };

    println!("\n  simulate 30 permission checks:");
    for i in 1..=30 {
        let allowed = limiter7.check("api_client").await;
        if allowed {
            stats.allowed += 1;
        } else {
            stats.denied += 1;
        }
        if i <= 22 || i == 30 {
            let status = if allowed { "✓" } else { "✗" };
            println!(
                "    #{}: {} (remaining={})",
                i,
                status,
                limiter7.remaining("api_client")
            );
        } else if i == 23 {
            println!("    ... (middle output omitted) ...");
        }
    }
    println!(
        "\n  statistics: {} passed, {} rejected",
        stats.allowed, stats.denied
    );
    println!();

    // ============================================
    // 8. Default trait 演示
    // ============================================
    println!("--- Scenario 8: the Default trait ---");
    let default_limiter: RateLimiter = RateLimiter::default();
    println!("  ✓ Default configuration: 100 req/min, 10000 buckets, burst=100");
    let allowed = default_limiter.check("default_user").await;
    println!(
        "    first request: allowed={}, remaining={}",
        allowed,
        default_limiter.remaining("default_user")
    );
    println!();

    // ============================================
    // 9. update_config() 动态调整配置
    // ============================================
    println!("--- Scenario 9: update_config() dynamic adjustment ---");
    let mut limiter9 = RateLimiter::new(10, Duration::from_secs(60), 10000, 10);
    println!("  ✓ initial configuration: max_requests=10, window=60s");

    // 动态调整为更严格的限制
    limiter9.update_config(5, Duration::from_secs(60));
    println!("  → updated configuration: max_requests=5, window=60s");
    println!("  ⚠ note: update_config only affects newly created buckets, not existing ones");
    println!();

    // ============================================
    // 总结
    // ============================================
    println!("========================================");
    println!("✨ Rate limiter example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - RateLimiter::new(max, window, max_buckets, burst) - create a limiter");
    println!(
        "  - RateLimiter::with_defaults(max, window, max_buckets)  - use the default burst capacity"
    );
    println!("  - RateLimiter::default()              - default configuration (100/min)");
    println!(
        "  - limiter.check(key).await            - asynchronously check if allowed (returns bool)"
    );
    println!("  - limiter.remaining(key)              - query the number of remaining tokens");
    println!("  - limiter.reset(key)                  - reset the bucket for a given key");
    println!("  - limiter.cleanup()                   - clean up expired entries");
    println!("  - limiter.len() / is_empty()          - monitor the entry count");
    println!("  - limiter.update_config(max, window)  - adjust the configuration dynamically");
    println!("\n💡 Core features:");
    println!("  - token bucket algorithm: O(1) time complexity, lock-free CAS operations");
    println!("  - per-key independence: each key has its own token bucket");
    println!("  - burst capacity: burst_capacity can exceed max_requests");
    println!("  - token refill: smooth refill at the time-based rate");
    println!("  - LRU eviction: automatic eviction when max_buckets is reached");

    Ok(())
}

/// 简单的统计结构
struct Stats {
    allowed: u32,
    denied: u32,
}
