// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据分片示例
//!
//! 演示如何使用 [`ShardRouter`] 进行数据分片路由：
//! - 配置 `ShardConfig`（策略、分片数、前缀、连接模板）
//! - 创建 `ShardRouter` 并注册分片
//! - 展示 `ShardingStrategy`（yearly/monthly/daily/hash）
//! - 根据时间戳和关键字路由到不同分片
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example sharding --features "sqlite,sharding"
//! ```

use dbnexus::chrono::{TimeZone, Utc};
use dbnexus::{ShardConfig, ShardRouter};

fn main() {
    println!("========================================");
    println!("🔀 DBNexus data sharding example");
    println!("========================================\n");

    // 配置分片
    // 连接模板中 {shard} 会被替换为分片名称（prefix_shardId）
    let config = ShardConfig::new(
        "hash",                     // 策略：hash 均匀分布
        4,                          // 4 个分片
        "shard",                    // 分片名前缀
        "sqlite:./data/{shard}.db", // 连接模板
    );

    println!("📋 Shard configuration:");
    println!("  - strategy           : {}", config.strategy);
    println!("  - total shards       : {}", config.total_shards);
    println!("  - name prefix        : {}", config.prefix);
    println!("  - connection template : {}", config.connection_template);

    println!("\nGenerated shard connection strings:");
    for (shard_id, conn_str) in config.generate_all_connections() {
        println!("  - shard {}: {}", shard_id, conn_str);
    }

    // 创建分片路由器（同步版本，不创建连接池）
    let router = ShardRouter::with_config_sync(&config);
    println!("\n✓ Router created");
    println!("  - strategy name: {}", router.strategy_name());
    println!("  - total shards: {}", router.total_shards());
    println!("  - registered shards: {}", router.all_shards().len());

    // 演示不同策略的路由结果
    println!("\n─── Strategy comparison: routing results for the same timestamp ───\n");

    let test_time = Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
    let strategies = ["yearly", "monthly", "daily", "hash"];

    for strategy_name in &strategies {
        let cfg = ShardConfig::new(strategy_name, 4, "shard", "sqlite:./data/{shard}.db");
        let r = ShardRouter::with_config_sync(&cfg);
        let shard_id = r.calculate_shard(test_time, "");
        let shard_info = r.route(test_time);

        println!(
            "  {:<10} → shard {} ({})",
            strategy_name,
            shard_id,
            shard_info
                .map(|s| s.name.as_str())
                .unwrap_or("unregistered")
        );
    }

    // 哈希分片：时间戳路由
    println!("\n─── Hash sharding: routing results for different timestamps ───\n");

    let timestamps = vec![
        (
            "2024-01-01 00:00",
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
        ),
        (
            "2024-06-15 12:00",
            Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap(),
        ),
        (
            "2024-12-31 23:59",
            Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 0).unwrap(),
        ),
        (
            "2025-03-20 08:30",
            Utc.with_ymd_and_hms(2025, 3, 20, 8, 30, 0).unwrap(),
        ),
    ];

    for (label, ts) in &timestamps {
        let shard_id = router.calculate_shard(*ts, "");
        let shard_info = router.route(*ts);
        println!(
            "  {} → shard {} ({})",
            label,
            shard_id,
            shard_info
                .map(|s| s.name.as_str())
                .unwrap_or("unregistered")
        );
    }

    // 哈希分片：带关键字的路由（更均匀分布）
    println!("\n─── Hash sharding: routing with a key (user ID distribution) ───\n");

    let now = Utc::now();
    let mut distribution: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();

    for i in 0..20 {
        let key = format!("user_{}", i);
        let shard_id = router.calculate_shard(now, &key);
        let shard_info = router.route_with_key(now, &key);

        *distribution.entry(shard_id).or_insert(0) += 1;
        println!(
            "  {} → shard {} ({})",
            key,
            shard_id,
            shard_info
                .map(|s| s.name.as_str())
                .unwrap_or("unregistered")
        );
    }

    println!("\n📊 Shard distribution statistics:");
    for shard_id in 0..config.total_shards {
        let count = distribution.get(&shard_id).copied().unwrap_or(0);
        let bar = "█".repeat(count);
        println!("  shard {}: {:>2} {} entries", shard_id, count, bar);
    }

    // 分片策略特性说明
    println!("\n─── Sharding strategy features ───\n");
    println!("┌──────────┬────────────────────────────────────────────┐");
    println!("│ strategy │ features                                        │");
    println!("├──────────┼────────────────────────────────────────────┤");
    println!("│ yearly   │ shard by year, good for yearly archived history     │");
    println!("│ monthly  │ shard by month, good for monthly reports/logs       │");
    println!("│ daily    │ shard by day, good for high-frequency log data       │");
    println!("│ hash     │ hash sharding, even distribution, good for heavy writes │");
    println!("└──────────┴────────────────────────────────────────────┘");

    println!("\n========================================");
    println!("✨ Data sharding example complete!");
    println!("========================================");
    println!("\n📚 Key concepts:");
    println!("  - ShardConfig::new(strategy, total_shards, prefix, connection_template)");
    println!(
        "  - ShardRouter::with_config_sync(&config) create a router (without connecting to the database)"
    );
    println!("  - router.route(timestamp) route to a shard by timestamp");
    println!("  - router.route_with_key(timestamp, key) route by timestamp + key (more even)");
    println!(
        "  - router.calculate_shard(timestamp, key) computes the shard ID only, no table lookup"
    );
    println!(
        "  - the {{shard}} placeholder in the connection template is replaced with prefix_shardId"
    );
}
