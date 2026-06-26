// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

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

use dbnexus::access::permission::RateLimiter;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================");
    println!("⏱  DBNexus 速率限制器示例");
    println!("========================================\n");

    // ============================================
    // 1. 基本使用：new() 创建并消耗令牌
    // ============================================
    println!("--- 场景 1：基本使用 ---");
    let limiter = RateLimiter::new(5, Duration::from_secs(60), 10000, 5);
    println!("  ✓ RateLimiter 创建: max_requests=5, window=60s, burst=5");

    // 前 5 个请求应该被允许
    println!("\n  发送 5 个请求（应全部允许）:");
    for i in 1..=5 {
        let allowed = limiter.check("user_001").await;
        println!("    请求 #{}: allowed={}, remaining={}", i, allowed, limiter.remaining("user_001"));
    }

    // 第 6 个请求应该被拒绝
    println!("\n  发送第 6 个请求（应被拒绝）:");
    let allowed = limiter.check("user_001").await;
    println!("    请求 #6: allowed={}, remaining={}", allowed, limiter.remaining("user_001"));
    println!();

    // ============================================
    // 2. 不同键独立计数
    // ============================================
    println!("--- 场景 2：不同键独立计数 ---");
    let limiter2 = RateLimiter::with_defaults(3, Duration::from_secs(60), 10000);
    println!("  ✓ RateLimiter 创建: max_requests=3 (with_defaults)");

    // user_A 消耗 2 个令牌
    println!("\n  user_A 消耗 2 个令牌:");
    for _ in 0..2 {
        let _ = limiter2.check("user_A").await;
    }
    println!("    user_A remaining = {}", limiter2.remaining("user_A"));

    // user_B 消耗 1 个令牌
    println!("\n  user_B 消耗 1 个令牌:");
    let _ = limiter2.check("user_B").await;
    println!("    user_B remaining = {}", limiter2.remaining("user_B"));

    // 验证 user_A 仍有 1 个令牌
    println!("\n  验证 user_A 仍有 1 个令牌（独立计数）:");
    let allowed_a = limiter2.check("user_A").await;
    println!("    user_A 请求: allowed={}, remaining={}", allowed_a, limiter2.remaining("user_A"));
    let denied_a = limiter2.check("user_A").await;
    println!("    user_A 再次请求: allowed={} (应被拒绝)", denied_a);
    println!("    user_B remaining = {} (不受影响)", limiter2.remaining("user_B"));
    println!();

    // ============================================
    // 3. reset() 重置指定键
    // ============================================
    println!("--- 场景 3：reset() 重置指定键 ---");
    let limiter3 = RateLimiter::new(2, Duration::from_secs(60), 10000, 2);
    println!("  ✓ RateLimiter 创建: max_requests=2");

    // 消耗所有令牌
    let _ = limiter3.check("user_reset").await;
    let _ = limiter3.check("user_reset").await;
    let denied = limiter3.check("user_reset").await;
    println!("\n  消耗 2 个令牌后第 3 次请求: allowed={} (应被拒绝)", denied);
    println!("    remaining = {}", limiter3.remaining("user_reset"));

    // 重置
    println!("\n  执行 limiter.reset(\"user_reset\")");
    limiter3.reset("user_reset");
    println!("    remaining = {} (重置后)", limiter3.remaining("user_reset"));

    // 验证可再次请求
    let allowed = limiter3.check("user_reset").await;
    println!("\n  重置后再次请求: allowed={} (应被允许)", allowed);
    println!();

    // ============================================
    // 4. 突发流量（burst_capacity > max_requests）
    // ============================================
    println!("--- 场景 4：突发流量处理 ---");
    // refill_rate = 1/sec (max_requests=1 / window=1s), burst=10
    let limiter4 = RateLimiter::new(1, Duration::from_secs(1), 10000, 10);
    println!("  ✓ RateLimiter 创建: max_requests=1/s, burst_capacity=10");

    // 突发 10 个请求
    println!("\n  突发 10 个请求:");
    let mut allowed_count = 0;
    for _ in 1..=10 {
        if limiter4.check("burst_user").await {
            allowed_count += 1;
        }
    }
    println!("    允许 {} / 10 次请求（突发容量）", allowed_count);

    // 第 11 个应被拒绝
    let denied = limiter4.check("burst_user").await;
    println!("    第 11 次请求: allowed={} (突发容量耗尽)", denied);
    println!("    remaining = {}", limiter4.remaining("burst_user"));
    println!();

    // ============================================
    // 5. 令牌桶填充（等待后令牌恢复）
    // ============================================
    println!("--- 场景 5：令牌桶填充 ---");
    let limiter5 = RateLimiter::new(10, Duration::from_secs(1), 10000, 10);
    println!("  ✓ RateLimiter 创建: max_requests=10/s, burst=10");

    // 消耗所有令牌
    println!("\n  消耗所有 10 个令牌:");
    for _ in 0..10 {
        let _ = limiter5.check("refill_user").await;
    }
    println!("    remaining = {} (已耗尽)", limiter5.remaining("refill_user"));

    // 等待 1.1 秒让令牌填充
    println!("\n  等待 1.1 秒让令牌填充...");
    tokio::time::sleep(Duration::from_millis(1100)).await;

    let allowed = limiter5.check("refill_user").await;
    println!("    等待后请求: allowed={} (令牌已填充)", allowed);
    println!("    remaining = {}", limiter5.remaining("refill_user"));
    println!();

    // ============================================
    // 6. cleanup() 清理孤立条目
    // ============================================
    println!("--- 场景 6：cleanup() 清理孤立条目 ---");
    let limiter6 = RateLimiter::new(10, Duration::from_millis(100), 10000, 10);
    println!("  ✓ RateLimiter 创建: window=100ms (短窗口用于演示)");

    // 创建多个条目
    println!("\n  创建 5 个条目:");
    for i in 0..5 {
        let key = format!("cleanup_user_{}", i);
        let _ = limiter6.check(&key).await;
    }
    println!("    len = {} (条目数)", limiter6.len());
    println!("    is_empty = {}", limiter6.is_empty());

    // 立即清理不应删除任何条目
    let removed = limiter6.cleanup();
    println!("\n  立即清理: removed = {} (未过期)", removed);
    println!("    len = {}", limiter6.len());

    // 等待 1.5 秒让条目过期（10 倍窗口 = 1 秒）
    println!("\n  等待 1.5 秒让条目过期（过期阈值 = 10 * 100ms = 1s）...");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let removed = limiter6.cleanup();
    println!("    清理: removed = {} (已过期)", removed);
    println!("    len = {} (清理后)", limiter6.len());
    println!("    is_empty = {}", limiter6.is_empty());
    println!();

    // ============================================
    // 7. 模拟高频率权限检查场景
    // ============================================
    println!("--- 场景 7：模拟高频率权限检查 ---");
    let limiter7 = RateLimiter::new(20, Duration::from_secs(60), 10000, 20);
    println!("  ✓ RateLimiter 创建: max_requests=20/min (模拟 API 限流)");

    let mut stats = Stats {
        allowed: 0,
        denied: 0,
    };

    println!("\n  模拟 30 次权限检查:");
    for i in 1..=30 {
        let allowed = limiter7.check("api_client").await;
        if allowed {
            stats.allowed += 1;
        } else {
            stats.denied += 1;
        }
        if i <= 22 || i == 30 {
            let status = if allowed { "✓" } else { "✗" };
            println!("    #{}: {} (remaining={})", i, status, limiter7.remaining("api_client"));
        } else if i == 23 {
            println!("    ... (省略中间输出) ...");
        }
    }
    println!("\n  统计: {} 通过, {} 拒绝", stats.allowed, stats.denied);
    println!();

    // ============================================
    // 8. Default trait 演示
    // ============================================
    println!("--- 场景 8：Default trait ---");
    let default_limiter: RateLimiter = RateLimiter::default();
    println!("  ✓ Default 配置: 100 req/min, 10000 buckets, burst=100");
    let allowed = default_limiter.check("default_user").await;
    println!("    首次请求: allowed={}, remaining={}", allowed, default_limiter.remaining("default_user"));
    println!();

    // ============================================
    // 9. update_config() 动态调整配置
    // ============================================
    println!("--- 场景 9：update_config() 动态调整 ---");
    let mut limiter9 = RateLimiter::new(10, Duration::from_secs(60), 10000, 10);
    println!("  ✓ 初始配置: max_requests=10, window=60s");

    // 动态调整为更严格的限制
    limiter9.update_config(5, Duration::from_secs(60));
    println!("  → 更新配置: max_requests=5, window=60s");
    println!("  ⚠ 注意：update_config 只影响新创建的桶，不影响已存在的桶");
    println!();

    // ============================================
    // 总结
    // ============================================
    println!("========================================");
    println!("✨ 速率限制器示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - RateLimiter::new(max, window, max_buckets, burst) - 创建限流器");
    println!("  - RateLimiter::with_defaults(max, window, max_buckets)  - 使用默认突发容量");
    println!("  - RateLimiter::default()              - 默认配置（100/min）");
    println!("  - limiter.check(key).await            - 异步检查是否允许（返回 bool）");
    println!("  - limiter.remaining(key)              - 查询剩余令牌数");
    println!("  - limiter.reset(key)                  - 重置指定键的桶");
    println!("  - limiter.cleanup()                   - 清理过期条目");
    println!("  - limiter.len() / is_empty()          - 监控条目数");
    println!("  - limiter.update_config(max, window)  - 动态调整配置");
    println!("\n💡 核心特性:");
    println!("  - 令牌桶算法：O(1) 时间复杂度，无锁 CAS 操作");
    println!("  - 多键独立：每个 key 拥有独立的令牌桶");
    println!("  - 突发容量：burst_capacity 可大于 max_requests");
    println!("  - 令牌填充：按时间速率平滑填充");
    println!("  - LRU 驱逐：达到 max_buckets 时自动驱逐");

    Ok(())
}

/// 简单的统计结构
struct Stats {
    allowed: u32,
    denied: u32,
}
