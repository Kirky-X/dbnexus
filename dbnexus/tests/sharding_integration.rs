// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Sharding 集成测试
//!
//! 测试分片模块的时间边界、哈希分布均匀性、路由器高级功能等

use chrono::TimeZone;
use chrono::Utc;
use dbnexus::sharding::{DailyStrategy, MonthlyStrategy, ShardConfig, ShardRouter, ShardingStrategy, YearlyStrategy};
mod common;

/// TEST-SHARD-001: 闰年2月29日分片测试
#[test]
fn test_sharding_leap_year_feb29() {
    let yearly = YearlyStrategy;
    let daily = DailyStrategy;

    let leap_day = Utc.timestamp_opt(1709294400, 0).unwrap();

    let yearly_shard = yearly.calculate(leap_day, 12);
    let daily_shard = daily.calculate(leap_day, 365);

    assert_eq!(yearly_shard, 2024 % 12, "Yearly shard for 2024 should be {}", 2024 % 12);
    assert!((0..365).contains(&daily_shard), "Daily shard should be in valid range");

    let non_leap_day = Utc.timestamp_opt(1677604800, 0).unwrap();
    let non_leap_yearly = yearly.calculate(non_leap_day, 12);

    assert_ne!(
        yearly_shard, non_leap_yearly,
        "Different years should have different shards"
    );
}

/// TEST-SHARD-002: 年边界测试（12月31日→1月1日）
#[test]
fn test_sharding_year_boundary() {
    let yearly = YearlyStrategy;

    let last_day_2023 = Utc.timestamp_opt(1703980800, 0).unwrap();
    let first_day_2024 = Utc.timestamp_opt(1704067200, 0).unwrap();

    let last_shard = yearly.calculate(last_day_2023, 12);
    let first_shard = yearly.calculate(first_day_2024, 12);

    assert_ne!(last_shard, first_shard, "Different years should have different shards");
    assert_eq!(last_shard, 2023 % 12, "Last day of 2023 should use 2023 % 12");
    assert_eq!(first_shard, 2024 % 12, "First day of 2024 should use 2024 % 12");
}

/// TEST-SHARD-003: 月边界测试
#[test]
fn test_sharding_month_boundary() {
    let monthly = MonthlyStrategy;

    let jan_last = Utc.timestamp_opt(1706745600, 0).unwrap();
    let feb_first = Utc.timestamp_opt(1706745600 + 86400, 0).unwrap();
    let feb_last = Utc.timestamp_opt(1709241600, 0).unwrap();

    let jan_shard = monthly.calculate(jan_last, 100);
    let feb_shard = monthly.calculate(feb_first, 100);
    let feb_last_shard = monthly.calculate(feb_last, 100);

    assert_eq!(feb_shard, feb_last_shard, "Same month should have same shard");
    println!("Jan: {}, Feb: {}", jan_shard, feb_shard);
    assert!(jan_shard < 100 && feb_shard < 100, "All shards should be valid");
}

/// TEST-SHARD-004: 日边界测试
#[test]
fn test_sharding_day_boundary() {
    let daily = DailyStrategy;

    let day1 = Utc.timestamp_opt(1718414400, 0).unwrap();
    let day2 = Utc.timestamp_opt(1718500800, 0).unwrap();

    let shard1 = daily.calculate(day1, 30);
    let shard2 = daily.calculate(day2, 30);

    assert_ne!(shard1, shard2, "Different days should have different shards");
}

/// TEST-SHARD-005: 哈希分布均匀性测试
#[test]
fn test_sharding_hash_distribution_uniformity() {
    let yearly = YearlyStrategy;

    let num_shards: u32 = 12;
    let sample_size = 10000;

    let mut yearly_counts = vec![0usize; num_shards as usize];

    for day_offset in 0..sample_size {
        let dt = Utc.timestamp_opt(1704067200 + day_offset as i64 * 86400, 0).unwrap();
        yearly_counts[yearly.calculate(dt, num_shards) as usize] += 1;
    }

    let min_expected = sample_size / num_shards as usize / 10;
    for count in &yearly_counts {
        assert!(*count >= min_expected, "Yearly shard distribution too uneven");
    }

    let yearly_mean = sample_size as f64 / num_shards as f64;
    let yearly_variance: f64 = yearly_counts
        .iter()
        .map(|c| {
            let diff = *c as f64 - yearly_mean;
            diff * diff
        })
        .sum::<f64>()
        / num_shards as f64;
    let yearly_stddev = yearly_variance.sqrt();

    println!("Yearly distribution: {:?}", yearly_counts);
    println!("Mean: {:.2}, StdDev: {:.2}", yearly_mean, yearly_stddev);

    assert!(
        yearly_stddev / yearly_mean < 0.5,
        "Distribution too uneven, stddev/mean = {:.2}",
        yearly_stddev / yearly_mean
    );
}

/// TEST-SHARD-006: 卡方分布均匀性验证测试
#[test]
fn test_sharding_chi_square_test() {
    let daily = DailyStrategy;

    let num_shards: u32 = 30;
    let sample_size = 3000;

    let mut observed = vec![0usize; num_shards as usize];

    for day_offset in 0..sample_size {
        let dt = Utc.timestamp_opt(1704067200 + day_offset as i64 * 86400, 0).unwrap();
        let shard = daily.calculate(dt, num_shards);
        observed[shard as usize] += 1;
    }

    let expected = sample_size as f64 / num_shards as f64;
    let chi_square: f64 = observed
        .iter()
        .map(|obs| {
            let diff = *obs as f64 - expected;
            diff * diff / expected
        })
        .sum();

    let critical_value = 50.0;

    println!("Chi-square statistic: {:.2}", chi_square);
    assert!(
        chi_square < critical_value,
        "Chi-square test failed: {:.2} >= {}",
        chi_square,
        critical_value
    );
}

/// TEST-SHARD-007: 分片路由基础测试
#[test]
fn test_shard_router_basic() {
    let mut router = ShardRouter::with_strategy("yearly", 12);

    router.register_shard(0, "db_0".to_string(), "sqlite:./data/db_0.db".to_string());
    router.register_shard(4, "db_4".to_string(), "sqlite:./data/db_4.db".to_string());

    let dt = Utc::now();
    let calculated = router.calculate_shard(dt, "");

    assert!(
        (0..12).contains(&calculated),
        "calculate_shard should return valid shard ID"
    );
    assert!(router.total_shards() > 0, "Should have registered shards");
}

/// TEST-SHARD-008: 带关键字路由功能测试
#[test]
fn test_router_route_with_key() {
    let mut router = ShardRouter::with_strategy("monthly", 6);

    for i in 0..6 {
        router.register_shard(i, format!("db_{}", i), format!("sqlite:./data/db_{}.db", i));
    }

    let dt = Utc::now();

    let mut shards_seen = Vec::new();

    for i in 0..100 {
        let key = format!("user_{}", i);
        let shard = router.route_with_key(dt, &key);
        if let Some(s) = shard {
            if !shards_seen.contains(&s.shard_id) {
                shards_seen.push(s.shard_id);
            }
        }
    }

    assert!(!shards_seen.is_empty(), "Should route some keys to shards");
    println!("Unique shards seen with key routing: {:?}", shards_seen);
}

/// TEST-SHARD-009: 计算分片一致性验证测试
#[test]
fn test_router_calculate_shard_consistency() {
    let router = ShardRouter::with_strategy("yearly", 12);

    let dt = Utc::now();

    let shard1 = router.calculate_shard(dt, "");
    let shard2 = router.calculate_shard(dt, "");
    assert_eq!(shard1, shard2, "Same timestamp should give same shard");

    let dt_2023 = Utc.timestamp_opt(1686835200, 0).unwrap();
    let shard_2023 = router.calculate_shard(dt_2023, "");
    assert_ne!(shard1, shard_2023, "Different years should give different shards");
}

/// TEST-SHARD-010: 策略工厂函数测试
#[test]
fn test_strategy_factory_invalid_name() {
    use dbnexus::sharding::create_strategy;

    let invalid_strategy = create_strategy("invalid_strategy_name");
    let default_strategy = create_strategy("default");

    let test_time = Utc::now();

    let shard1 = invalid_strategy.calculate(test_time, 12);
    let shard2 = default_strategy.calculate(test_time, 12);

    assert_eq!(shard1, shard2, "Invalid strategy should fall back to default");
}

/// TEST-SHARD-011: 策略别名映射测试
#[test]
fn test_strategy_factory_aliases() {
    use dbnexus::sharding::create_strategy;

    let test_time = Utc::now();

    let yearly1 = create_strategy("yearly");
    let yearly2 = create_strategy("year");
    let monthly1 = create_strategy("monthly");
    let monthly2 = create_strategy("month");
    let daily1 = create_strategy("daily");
    let daily2 = create_strategy("day");

    let y1 = yearly1.calculate(test_time, 12);
    let y2 = yearly2.calculate(test_time, 12);
    let m1 = monthly1.calculate(test_time, 100);
    let m2 = monthly2.calculate(test_time, 100);
    let d1 = daily1.calculate(test_time, 365);
    let d2 = daily2.calculate(test_time, 365);

    assert_eq!(y1, y2, "'year' should alias to 'yearly'");
    assert_eq!(m1, m2, "'month' should alias to 'monthly'");
    assert_eq!(d1, d2, "'day' should alias to 'daily'");
}

/// TEST-SHARD-012: 策略名大小写不敏感测试
#[test]
fn test_strategy_factory_case_insensitive() {
    use dbnexus::sharding::create_strategy;

    let test_time = Utc::now();

    let variants = vec!["YEARLY", "Yearly", "yEaRlY"];
    let base_shard = create_strategy("yearly").calculate(test_time, 12);

    for variant in variants {
        let strategy = create_strategy(variant);
        let shard = strategy.calculate(test_time, 12);
        assert_eq!(shard, base_shard, "'{}' should work like 'yearly'", variant);
    }
}

/// TEST-SHARD-013: ShardConfig 连接字符串模板测试
#[test]
fn test_shard_config_template_parsing() {
    let config = ShardConfig::new("yearly", 12, "orders", "postgresql://localhost/{shard}/{prefix}_{id}");

    let shard_0 = config.generate_connection_string(0);
    let shard_5 = config.generate_connection_string(5);

    assert!(
        shard_0.contains("0") || shard_0.contains("shard"),
        "Should contain shard 0"
    );
    assert!(shard_5.contains("5"), "Should contain shard 5");
    assert!(!shard_0.contains("{shard}"), "Template should be resolved");
}

/// TEST-SHARD-014: 路由器配置集成测试
#[test]
fn test_router_with_config_integration() {
    let config = ShardConfig::new("monthly", 6, "products", "postgresql://localhost/{shard}/products.db");

    let router = ShardRouter::with_config(&config);

    let total = router.total_shards();
    let strategy = router.strategy_name();

    assert!(total > 0, "Should have shards configured");
    assert!(!strategy.is_empty(), "Should have a strategy name");

    let shards = router.all_shards();
    assert!(!shards.is_empty(), "Should have some shards");

    println!("Total shards: {}, Strategy: {}", total, strategy);
}

/// TEST-SHARD-015: 分片策略边界值测试
#[test]
fn test_sharding_strategy_boundaries() {
    let yearly = YearlyStrategy;
    let monthly = MonthlyStrategy;
    let daily = DailyStrategy;

    let dt = Utc::now();

    let shard_1 = yearly.calculate(dt, 1);
    assert_eq!(shard_1, 0, "With 1 shard, result should always be 0");

    let shard_2 = yearly.calculate(dt, 2);
    assert!(shard_2 < 2, "Shard should be 0 or 1");

    let shard_1000 = monthly.calculate(dt, 1000);
    assert!(shard_1000 < 1000, "Shard should be less than 1000");

    let shard_365 = daily.calculate(dt, 365);
    assert!(shard_365 < 365, "Daily shard should be less than 365");
}
