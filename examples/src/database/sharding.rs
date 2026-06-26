// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

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

use chrono::{TimeZone, Utc};
use dbnexus::{ShardConfig, ShardRouter};

fn main() {
    println!("========================================");
    println!("🔀 DBNexus 数据分片示例");
    println!("========================================\n");

    // ============================================
    // 1. 配置分片
    // ============================================
    // 连接模板中 {shard} 会被替换为分片名称（prefix_shardId）
    let config = ShardConfig::new(
        "hash",  // 策略：hash 均匀分布
        4,       // 4 个分片
        "shard", // 分片名前缀
        "sqlite:./data/{shard}.db", // 连接模板
    );

    println!("📋 分片配置:");
    println!("  - 策略           : {}", config.strategy);
    println!("  - 总分片数       : {}", config.total_shards);
    println!("  - 名称前缀       : {}", config.prefix);
    println!("  - 连接模板       : {}", config.connection_template);

    // 展示生成的连接字符串
    println!("\n生成的分片连接字符串:");
    for (shard_id, conn_str) in config.generate_all_connections() {
        println!("  - 分片 {}: {}", shard_id, conn_str);
    }

    // ============================================
    // 2. 创建分片路由器（同步版本，不创建连接池）
    // ============================================
    let router = ShardRouter::with_config_sync(&config);
    println!("\n✓ 路由器创建成功");
    println!("  - 策略名称: {}", router.strategy_name());
    println!("  - 总分片数: {}", router.total_shards());
    println!("  - 已注册分片: {}", router.all_shards().len());

    // ============================================
    // 3. 演示不同策略的路由结果
    // ============================================
    println!("\n─── 策略对比：不同策略对同一时间戳的路由结果 ───\n");

    let test_time = Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
    let strategies = ["yearly", "monthly", "daily", "hash"];

    for strategy_name in &strategies {
        let cfg = ShardConfig::new(strategy_name, 4, "shard", "sqlite:./data/{shard}.db");
        let r = ShardRouter::with_config_sync(&cfg);
        let shard_id = r.calculate_shard(test_time, "");
        let shard_info = r.route(test_time);

        println!(
            "  {:<10} → 分片 {} ({})",
            strategy_name,
            shard_id,
            shard_info.map(|s| s.name.as_str()).unwrap_or("未注册")
        );
    }

    // ============================================
    // 4. 哈希分片：时间戳路由
    // ============================================
    println!("\n─── 哈希分片：不同时间戳的路由结果 ───\n");

    let timestamps = vec![
        ("2024-01-01 00:00", Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
        ("2024-06-15 12:00", Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap()),
        ("2024-12-31 23:59", Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 0).unwrap()),
        ("2025-03-20 08:30", Utc.with_ymd_and_hms(2025, 3, 20, 8, 30, 0).unwrap()),
    ];

    for (label, ts) in &timestamps {
        let shard_id = router.calculate_shard(*ts, "");
        let shard_info = router.route(*ts);
        println!(
            "  {} → 分片 {} ({})",
            label,
            shard_id,
            shard_info.map(|s| s.name.as_str()).unwrap_or("未注册")
        );
    }

    // ============================================
    // 5. 哈希分片：带关键字的路由（更均匀分布）
    // ============================================
    println!("\n─── 哈希分片：带关键字的路由（用户ID分布） ───\n");

    let now = Utc::now();
    let mut distribution: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();

    for i in 0..20 {
        let key = format!("user_{}", i);
        let shard_id = router.calculate_shard(now, &key);
        let shard_info = router.route_with_key(now, &key);

        *distribution.entry(shard_id).or_insert(0) += 1;
        println!(
            "  {} → 分片 {} ({})",
            key,
            shard_id,
            shard_info.map(|s| s.name.as_str()).unwrap_or("未注册")
        );
    }

    println!("\n📊 分片分布统计:");
    for shard_id in 0..config.total_shards {
        let count = distribution.get(&shard_id).copied().unwrap_or(0);
        let bar = "█".repeat(count);
        println!("  分片 {}: {:>2} 条 {}", shard_id, count, bar);
    }

    // ============================================
    // 6. 分片策略特性说明
    // ============================================
    println!("\n─── 分片策略特性 ───\n");
    println!("┌──────────┬────────────────────────────────────────────┐");
    println!("│ 策略     │ 特性                                       │");
    println!("├──────────┼────────────────────────────────────────────┤");
    println!("│ yearly   │ 按年分片，适合按年归档的历史数据            │");
    println!("│ monthly  │ 按月分片，适合月度报表/日志                 │");
    println!("│ daily    │ 按日分片，适合高频日志数据                  │");
    println!("│ hash     │ 哈希分片，数据均匀分布，适合高并发写入      │");
    println!("└──────────┴────────────────────────────────────────────┘");

    println!("\n========================================");
    println!("✨ 数据分片示例完成！");
    println!("========================================");
    println!("\n📚 关键概念:");
    println!("  - ShardConfig::new(strategy, total_shards, prefix, connection_template)");
    println!("  - ShardRouter::with_config_sync(&config) 创建路由器（不连接数据库）");
    println!("  - router.route(timestamp) 根据时间戳路由到分片");
    println!("  - router.route_with_key(timestamp, key) 根据时间+关键字路由（更均匀）");
    println!("  - router.calculate_shard(timestamp, key) 仅计算分片ID不查表");
    println!("  - 连接模板中 {{shard}} 占位符会被替换为 prefix_shardId");
}
