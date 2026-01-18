// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 分片管理示例
//!
//! 展示如何使用 dbnexus 的分片功能：
//! - 配置分片路由器
//! - 使用不同的分片策略（年、月、日、哈希）
//! - 管理多个数据库分片
//! - 跨分片查询
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example sharding --features sqlite,sharding
//! ```

use chrono::{DateTime, Datelike, TimeZone, Utc};
use dbnexus::sharding::{ShardConfig, ShardRouter};
use std::collections::HashMap;

/// 定义 Order Entity（用于分片演示）
#[derive(Debug, Clone)]
struct Order {
    id: i64,
    user_id: i64,
    amount: f64,
    created_at: DateTime<Utc>,
}

/// 定义 Log Entity（日志数据，适合按时间分片）
#[derive(Debug, Clone)]
struct LogEntry {
    id: i64,
    level: String,
    message: String,
    timestamp: DateTime<Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔀 DBNexus 分片管理示例\n");
    println!("========================================");

    // 1. 演示年分片策略
    println!("\n1️⃣ 年分片策略 (Yearly Strategy)");
    println!("------------------------------------------");
    let yearly_router = ShardRouter::with_strategy("yearly", 12);
    let total_shards = 12;

    println!("  策略名称: {}", yearly_router.strategy_name());
    println!("  总分片数: {}", total_shards);

    // 测试不同年份的分片
    let test_years = [2023, 2024, 2025, 2026];
    for year in test_years {
        let timestamp = Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).unwrap();
        let shard_id = yearly_router.calculate_shard(timestamp, "");
        println!("  - {} 年 -> 分片 #{}", year, shard_id);
    }

    // 2. 演示月分片策略
    println!("\n2️⃣ 月分片策略 (Monthly Strategy)");
    println!("------------------------------------------");
    let monthly_router = ShardRouter::with_strategy("monthly", 12);

    println!("  策略名称: {}", monthly_router.strategy_name());
    println!("  总分片数: {}", total_shards);

    // 测试不同月份的分片
    let test_months = [(2024, 1), (2024, 6), (2024, 12), (2025, 1), (2025, 6)];
    for (year, month) in test_months {
        let timestamp = Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0).unwrap();
        let shard_id = monthly_router.calculate_shard(timestamp, "");
        println!("  - {}年{}月 -> 分片 #{}", year, month, shard_id);
    }

    // 3. 演示日分片策略
    println!("\n3️⃣ 日分片策略 (Daily Strategy)");
    println!("------------------------------------------");
    let daily_router = ShardRouter::with_strategy("daily", 365);
    let total_shards = 365;

    println!("  策略名称: {}", daily_router.strategy_name());
    println!("  总分片数: {}", total_shards);

    // 测试不同日期的分片
    let test_dates = [(2024, 1, 1), (2024, 6, 15), (2024, 12, 31), (2025, 1, 1)];
    for (year, month, day) in test_dates {
        let timestamp = Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap();
        let shard_id = daily_router.calculate_shard(timestamp, "");
        println!("  - {}-{}-{} -> 分片 #{}", year, month, day, shard_id);
    }

    // 4. 演示哈希分片策略
    println!("\n4️⃣ 哈希分片策略 (Hash Strategy)");
    println!("------------------------------------------");
    let hash_router = ShardRouter::with_strategy("hash", 16);
    let total_shards = 16;

    println!("  策略名称: {}", hash_router.strategy_name());
    println!("  总分片数: {}", total_shards);

    // 测试不同时间戳的哈希分片
    let test_times = [
        "2024-01-01T00:00:00Z",
        "2024-01-01T12:00:00Z",
        "2024-06-15T00:00:00Z",
        "2024-12-31T23:59:59Z",
    ];
    for time_str in test_times {
        let timestamp: DateTime<Utc> = time_str.parse()?;
        let shard_id = hash_router.calculate_shard(timestamp, "");
        println!("  - {} -> 分片 #{}", time_str, shard_id);
    }

    // 5. 创建分片配置
    println!("\n5️⃣ 创建分片配置");
    println!("------------------------------------------");

    // 按月分片的订单数据配置
    let order_shard_config = ShardConfig::new("monthly", 12, "orders", "sqlite:./shards/{shard}.db");

    println!("✓ 订单分片配置创建成功");
    println!("  - 策略: {}", order_shard_config.strategy);
    println!("  - 总分片数: {}", order_shard_config.total_shards);
    println!("  - 前缀: {}", order_shard_config.prefix);
    println!("  - 连接模板: {}", order_shard_config.connection_template);

    // 6. 创建分片路由器
    println!("\n6️⃣ 创建分片路由器");
    println!("------------------------------------------");

    let router = ShardRouter::with_config_sync(&order_shard_config);
    println!("✓ 分片路由器创建成功");

    // 模拟订单数据并计算分片
    let orders = vec![
        Order {
            id: 1,
            user_id: 100,
            amount: 99.99,
            created_at: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
        },
        Order {
            id: 2,
            user_id: 101,
            amount: 149.99,
            created_at: Utc.with_ymd_and_hms(2024, 6, 20, 14, 45, 0).unwrap(),
        },
        Order {
            id: 3,
            user_id: 102,
            amount: 199.99,
            created_at: Utc.with_ymd_and_hms(2024, 12, 25, 9, 15, 0).unwrap(),
        },
        Order {
            id: 4,
            user_id: 103,
            amount: 79.99,
            created_at: Utc.with_ymd_and_hms(2025, 1, 5, 16, 20, 0).unwrap(),
        },
    ];

    println!("\n  订单分片路由:");
    for order in &orders {
        let shard_id = router.calculate_shard(order.created_at, "");
        println!(
            "    - 订单 #{} ({}年{}月) -> 分片 #{}",
            order.id,
            order.created_at.year(),
            order.created_at.month(),
            shard_id
        );
    }

    // 7. 查看所有分片
    println!("\n7️⃣ 查看所有分片");
    println!("------------------------------------------");

    let all_shards = router.all_shards();
    println!("  总分片数: {}", all_shards.len());
    for shard in all_shards {
        println!(
            "  ✓ 分片 #{}: {} ({})",
            shard.shard_id, shard.name, shard.connection_string
        );
    }

    // 8. 分片统计信息
    println!("\n8️⃣ 分片统计信息");
    println!("------------------------------------------");

    println!("  📊 分片统计:");
    println!("    - 总分片数: {}", router.total_shards());
    println!("    - 已注册分片数: {}", router.all_shards().len());
    println!("    - 策略名称: {}", router.strategy_name());

    // 9. 演示跨分片查询
    println!("\n9️⃣ 跨分片查询演示");
    println!("------------------------------------------");

    // 查询特定时间范围的订单
    let start_date = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let end_date = Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 59).unwrap();

    // 计算起始和结束时间的分片 ID
    let start_shard = router.calculate_shard(start_date, "");
    let end_shard = router.calculate_shard(end_date, "");

    println!(
        "  查询时间范围: {} 到 {}",
        start_date.format("%Y-%m-%d"),
        end_date.format("%Y-%m-%d")
    );
    println!("  起始分片: #{}", start_shard);
    println!("  结束分片: #{}", end_shard);
    println!("  ✓ 按月分片策略下，可能需要查询多个分片");

    // 10. 分片数据迁移
    println!("\n🔟 分片数据迁移演示");
    println!("------------------------------------------");

    // 模拟数据迁移
    let source_shard_id = 0;
    let target_shard_id = 1;
    let migration_count = 1000;

    println!(
        "  从分片 #{} 迁移 {} 条记录到分片 #{}...",
        source_shard_id, migration_count, target_shard_id
    );

    // 模拟迁移过程
    for i in 0..migration_count {
        if i % 200 == 0 {
            println!("    已迁移 {} / {} 条记录", i, migration_count);
        }
    }
    println!("  ✓ 迁移完成: {} 条记录", migration_count);

    // 11. 分片均衡
    println!("\n1️⃣1️⃣ 分片均衡演示");
    println!("------------------------------------------");

    // 获取各分片的数据分布
    let mut shard_distribution = HashMap::new();
    for i in 0..12 {
        let shard_id = i;
        // 模拟数据分布（不均匀）
        let count = if i < 6 {
            5000 + i as i64 * 1000
        } else {
            1000 + i as i64 * 200
        };
        shard_distribution.insert(shard_id, count);
    }

    println!("  当前分片数据分布:");
    for (shard_id, count) in &shard_distribution {
        println!("    - 分片 #{}: {} 条记录", shard_id, count);
    }

    // 计算标准差
    let avg: f64 = shard_distribution.values().map(|&v| v as f64).sum::<f64>() / shard_distribution.len() as f64;
    let variance: f64 = shard_distribution
        .values()
        .map(|&v| (v as f64 - avg).powi(2))
        .sum::<f64>()
        / shard_distribution.len() as f64;
    let std_dev = variance.sqrt();

    println!("  📊 分布统计:");
    println!("    - 平均: {:.0} 条", avg);
    println!("    - 标准差: {:.0} 条", std_dev);
    println!("    - 变异系数: {:.2}%", (std_dev / avg) * 100.0);

    if std_dev > avg * 0.3 {
        println!("  ⚠️  数据分布不均，建议进行分片均衡");
    } else {
        println!("  ✓ 数据分布相对均衡");
    }

    // 12. 分片故障转移
    println!("\n1️⃣2️⃣ 分片故障转移演示");
    println!("------------------------------------------");

    let failed_shard_id = 3;
    let backup_shard_id = 11;

    println!("  检测到分片 #{} 故障", failed_shard_id);
    println!("  启动故障转移流程...");

    // 模拟故障转移
    println!("  ✓ 将流量重定向到备份分片 #{}", backup_shard_id);
    println!("  ✓ 更新路由表");
    println!("  ✓ 通知应用层");
    println!("  ✓ 故障转移完成");

    // 13. 分片扩容
    println!("\n1️⃣3️⃣ 分片扩容演示");
    println!("------------------------------------------");

    let old_shard_count = 12;
    let new_shard_count = 24;

    println!("  当前分片数: {}", old_shard_count);
    println!("  目标分片数: {}", new_shard_count);
    println!("  开始扩容...");

    // 创建新的路由器配置
    let new_config = ShardConfig::new("monthly", new_shard_count, "orders", "sqlite:./shards/{shard}.db");

    // 模拟扩容过程
    for i in old_shard_count..new_shard_count {
        let connection_url = new_config.generate_connection_string(i);
        println!("  ✓ 创建新分片: {} ({})", i, connection_url);
    }

    println!("  ✓ 扩容完成: {} -> {} 分片", old_shard_count, new_shard_count);
    println!("  ⚠️  注意: 扩容后需要重新分配数据");

    println!("\n========================================");
    println!("✨ 分片管理示例运行完成！");

    println!("\n💡 分片策略选择建议:");
    println!("  - 年分片: 适合历史数据归档、年度报表");
    println!("  - 月分片: 适合订单、日志等时间序列数据");
    println!("  - 日分片: 适合高频日志、实时数据");
    println!("  - 哈希分片: 适合均匀分布、无时间特征的数据");

    Ok(())
}
