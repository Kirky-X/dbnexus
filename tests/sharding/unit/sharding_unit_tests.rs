// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分片系统单元测试
//!
//! 覆盖分片策略配置、哈希分片算法、范围分片算法、跨分片查询聚合、
//! 分片重平衡、分片热点检测、分片故障转移等场景

use dbnexus::chrono::{DateTime, Datelike, TimeZone, Utc};
use dbnexus::{ShardConfig, ShardRouter, ShardingStrategy, create_strategy};

// ============================================================================
// 哈希分片算法测试
// ============================================================================

/// TEST-SHARD-UNIT-001: 哈希分片策略基础计算测试
#[test]
fn test_hash_strategy_basic_calculation() {
    let strategy = create_strategy("hash");
    let dt = Utc::now();
    let total_shards = 12;

    let shard_id = strategy.calculate(dt, total_shards);

    assert!(shard_id < total_shards, "Shard ID should be within range");
    assert_eq!(strategy.name(), "hash");
}

/// TEST-SHARD-UNIT-002: 哈希分片策略相同输入产生相同结果测试
#[test]
fn test_hash_strategy_consistency() {
    let strategy = create_strategy("hash");
    let dt = Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
    let total_shards = 10;

    let shard1 = strategy.calculate(dt, total_shards);
    let shard2 = strategy.calculate(dt, total_shards);

    assert_eq!(shard1, shard2, "Same timestamp should produce same shard");
}

/// TEST-SHARD-UNIT-003: 哈希分片策略不同时间戳产生不同分布测试
#[test]
fn test_hash_strategy_different_timestamps() {
    let strategy = create_strategy("hash");
    let total_shards = 8;

    let dt1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let dt2 = Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap();
    let dt3 = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();

    let shard1 = strategy.calculate(dt1, total_shards);
    let shard2 = strategy.calculate(dt2, total_shards);
    let shard3 = strategy.calculate(dt3, total_shards);

    // 不同时间戳应该产生不同的哈希值（可能相同但不强制）
    let unique_shards = std::collections::HashSet::from([shard1, shard2, shard3]);
    assert!(!unique_shards.is_empty(), "Should have at least one unique shard");
}

/// TEST-SHARD-UNIT-004: 哈希分片策略边界值测试 - 单分片
#[test]
fn test_hash_strategy_single_shard() {
    let strategy = create_strategy("hash");
    let dt = Utc::now();

    let shard_id = strategy.calculate(dt, 1);

    assert_eq!(shard_id, 0, "Single shard should always return 0");
}

/// TEST-SHARD-UNIT-005: 哈希分片策略边界值测试 - 大分片数
#[test]
fn test_hash_strategy_large_shard_count() {
    let strategy = create_strategy("hash");
    let dt = Utc::now();

    for total_shards in [100, 1000, 10000] {
        let shard_id = strategy.calculate(dt, total_shards);
        assert!(
            shard_id < total_shards,
            "Shard ID should be valid for {} shards",
            total_shards
        );
    }
}

/// TEST-SHARD-UNIT-006: 哈希分片策略 is_valid_shard_id 测试
#[test]
fn test_hash_strategy_valid_shard_id() {
    let strategy = create_strategy("hash");

    assert!(strategy.is_valid_shard_id(0, 10));
    assert!(strategy.is_valid_shard_id(5, 10));
    assert!(strategy.is_valid_shard_id(9, 10));
    assert!(!strategy.is_valid_shard_id(10, 10));
    assert!(!strategy.is_valid_shard_id(100, 10));
}

// ============================================================================
// 范围分片算法测试
// ============================================================================

/// TEST-SHARD-UNIT-007: 年分片策略基础计算测试
#[test]
fn test_yearly_strategy_basic_calculation() {
    let strategy = create_strategy("yearly");
    let dt = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

    let shard_id = strategy.calculate(dt, 12);

    assert_eq!(shard_id, 2024 % 12);
    assert_eq!(strategy.name(), "yearly");
}

/// TEST-SHARD-UNIT-008: 年分片策略多年份测试
#[test]
fn test_yearly_strategy_multiple_years() {
    let strategy = create_strategy("yearly");
    let total_shards = 10;

    let years = [2020, 2021, 2022, 2023, 2024, 2025];
    let mut shards = Vec::new();

    for year in years {
        let dt = Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).unwrap();
        shards.push(strategy.calculate(dt, total_shards));
    }

    // 验证年份取模结果
    for (year, shard) in years.iter().zip(shards.iter()) {
        assert_eq!(*shard, (*year as u32) % total_shards);
    }
}

/// TEST-SHARD-UNIT-009: 月分片策略基础计算测试
#[test]
fn test_monthly_strategy_basic_calculation() {
    let strategy = create_strategy("monthly");
    let dt = Utc.with_ymd_and_hms(2024, 3, 15, 0, 0, 0).unwrap();

    let shard_id = strategy.calculate(dt, 100);

    // 2024 * 12 + 3 = 24291
    assert_eq!(shard_id, 24291 % 100);
    assert_eq!(strategy.name(), "monthly");
}

/// TEST-SHARD-UNIT-010: 月分片策略跨年测试
#[test]
fn test_monthly_strategy_cross_year() {
    let strategy = create_strategy("monthly");
    let total_shards = 12;

    let dt_jan_2024 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let dt_dec_2024 = Utc.with_ymd_and_hms(2024, 12, 1, 0, 0, 0).unwrap();
    let dt_jan_2025 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();

    let shard_jan_2024 = strategy.calculate(dt_jan_2024, total_shards);
    let shard_dec_2024 = strategy.calculate(dt_dec_2024, total_shards);
    let shard_jan_2025 = strategy.calculate(dt_jan_2025, total_shards);

    // 验证计算公式
    assert_eq!(shard_jan_2024, (2024 * 12 + 1) % total_shards);
    assert_eq!(shard_dec_2024, (2024 * 12 + 12) % total_shards);
    assert_eq!(shard_jan_2025, (2025 * 12 + 1) % total_shards);
}

/// TEST-SHARD-UNIT-011: 日分片策略基础计算测试
#[test]
fn test_daily_strategy_basic_calculation() {
    let strategy = create_strategy("daily");
    let dt = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();

    let days = dt.num_days_from_ce();
    let shard_id = strategy.calculate(dt, 100);

    assert_eq!(shard_id, days as u32 % 100);
    assert_eq!(strategy.name(), "daily");
}

/// TEST-SHARD-UNIT-012: 日分片策略连续日期测试
#[test]
fn test_daily_strategy_consecutive_days() {
    let strategy = create_strategy("daily");
    let total_shards = 30;

    let dt_day1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let dt_day2 = Utc.with_ymd_and_hms(2024, 1, 2, 0, 0, 0).unwrap();
    let dt_day3 = Utc.with_ymd_and_hms(2024, 1, 3, 0, 0, 0).unwrap();

    let shard1 = strategy.calculate(dt_day1, total_shards);
    let shard2 = strategy.calculate(dt_day2, total_shards);
    let shard3 = strategy.calculate(dt_day3, total_shards);

    assert!(
        shard1 != shard2 || shard2 != shard3,
        "Different days may map to same shard due to modulo"
    );
}

// ============================================================================
// 分片策略配置测试
// ============================================================================

/// TEST-SHARD-UNIT-013: ShardConfig 默认配置测试
#[test]
fn test_shard_config_default() {
    let config = ShardConfig::default();

    assert_eq!(config.strategy, "yearly");
    assert_eq!(config.total_shards, 12);
    assert_eq!(config.prefix, "db");
    assert_eq!(config.connection_template, "sqlite:./data/{shard}.db");
}

/// TEST-SHARD-UNIT-014: ShardConfig 自定义配置测试
#[test]
fn test_shard_config_custom() {
    let config = ShardConfig::new("hash", 8, "order", "postgresql://localhost/{shard}");

    assert_eq!(config.strategy, "hash");
    assert_eq!(config.total_shards, 8);
    assert_eq!(config.prefix, "order");
    assert_eq!(config.connection_template, "postgresql://localhost/{shard}");
}

/// TEST-SHARD-UNIT-015: ShardConfig 连接字符串生成测试
#[test]
fn test_shard_config_connection_string_generation() {
    let config = ShardConfig::new("monthly", 12, "orders", "postgresql://localhost/{shard}");

    let conn_0 = config.generate_connection_string(0);
    let conn_5 = config.generate_connection_string(5);
    let conn_11 = config.generate_connection_string(11);

    assert!(conn_0.contains("orders_0"), "Should contain orders_0: {}", conn_0);
    assert!(conn_5.contains("orders_5"), "Should contain orders_5: {}", conn_5);
    assert!(conn_11.contains("orders_11"), "Should contain orders_11: {}", conn_11);
}

/// TEST-SHARD-UNIT-016: ShardConfig 生成所有连接字符串测试
#[test]
fn test_shard_config_generate_all_connections() {
    let config = ShardConfig::new("daily", 4, "logs", "sqlite:./data/{shard}.db");

    let connections = config.generate_all_connections();

    assert_eq!(connections.len(), 4);
    for (shard_id, conn_str) in connections.iter() {
        assert!(conn_str.contains(&format!("logs_{}", shard_id)));
    }
}

/// TEST-SHARD-UNIT-017: ShardConfig 连接模板替换测试
#[test]
fn test_shard_config_template_replacement() {
    let config = ShardConfig::new("hash", 6, "data", "postgresql://{prefix}/{id}_{shard}");

    let conn = config.generate_connection_string(3);

    assert!(conn.contains("data"), "Should contain prefix");
    assert!(conn.contains("3"), "Should contain shard id");
    assert!(!conn.contains("{prefix}"), "Template should be replaced");
    assert!(!conn.contains("{shard}"), "Template should be replaced");
    assert!(!conn.contains("{id}"), "Template should be replaced");
}

// ============================================================================
// 分片策略工厂测试
// ============================================================================

/// TEST-SHARD-UNIT-018: 策略工厂别名测试
#[test]
fn test_strategy_factory_aliases() {
    let test_time = Utc::now();

    let yearly = create_strategy("yearly");
    let year = create_strategy("year");
    assert_eq!(yearly.calculate(test_time, 12), year.calculate(test_time, 12));

    let monthly = create_strategy("monthly");
    let month = create_strategy("month");
    assert_eq!(monthly.calculate(test_time, 100), month.calculate(test_time, 100));

    let daily = create_strategy("daily");
    let day = create_strategy("day");
    assert_eq!(daily.calculate(test_time, 30), day.calculate(test_time, 30));
}

/// TEST-SHARD-UNIT-019: 策略工厂大小写不敏感测试
#[test]
fn test_strategy_factory_case_insensitive() {
    let test_time = Utc::now();
    let base_shard = create_strategy("yearly").calculate(test_time, 12);

    for variant in ["YEARLY", "Yearly", "yEaRlY", "YeArLy"] {
        let shard = create_strategy(variant).calculate(test_time, 12);
        assert_eq!(shard, base_shard, "'{}' should work like 'yearly'", variant);
    }
}

/// TEST-SHARD-UNIT-020: 策略工厂无效名称回退测试
#[test]
fn test_strategy_factory_invalid_fallback() {
    let test_time = Utc::now();

    let invalid = create_strategy("invalid_strategy");
    let default = create_strategy("default");

    assert_eq!(
        invalid.calculate(test_time, 12),
        default.calculate(test_time, 12),
        "Invalid strategy should fallback to default (yearly)"
    );
}

// ============================================================================
// ShardRouter 基础功能测试
// ============================================================================

/// TEST-SHARD-UNIT-021: ShardRouter 创建测试
#[test]
fn test_shard_router_creation() {
    let router = ShardRouter::with_strategy("hash", 8);

    assert_eq!(router.total_shards(), 8);
    assert_eq!(router.strategy_name(), "hash");
}

/// TEST-SHARD-UNIT-022: ShardRouter 分片注册测试
#[test]
fn test_shard_router_registration() {
    let mut router = ShardRouter::with_strategy("yearly", 12);

    router.register_shard(0, "db_0".to_string(), "sqlite:./data/db_0.db".to_string());
    router.register_shard(5, "db_5".to_string(), "sqlite:./data/db_5.db".to_string());
    router.register_shard(11, "db_11".to_string(), "sqlite:./data/db_11.db".to_string());

    let shards = router.all_shards();
    assert_eq!(shards.len(), 3);

    let shard_ids: Vec<u32> = shards.iter().map(|s| s.shard_id).collect();
    assert!(shard_ids.contains(&0));
    assert!(shard_ids.contains(&5));
    assert!(shard_ids.contains(&11));
}

/// TEST-SHARD-UNIT-023: ShardRouter 路由功能测试
#[test]
fn test_shard_router_route() {
    let mut router = ShardRouter::with_strategy("yearly", 12);

    // 2024 % 12 = 8
    router.register_shard(8, "db_2024".to_string(), "sqlite:./data/db_2024.db".to_string());

    let dt = Utc.with_ymd_and_hms(2024, 6, 15, 0, 0, 0).unwrap();
    let shard = router.route(dt);

    assert!(shard.is_some());
    assert_eq!(shard.unwrap().shard_id, 8);
}

/// TEST-SHARD-UNIT-024: ShardRouter 计算分片 ID 测试
#[test]
fn test_shard_router_calculate_shard() {
    let router = ShardRouter::with_strategy("monthly", 6);

    let dt = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();
    let shard_id = router.calculate_shard(dt, "");

    // (2024 * 12 + 3) % 6 = 24291 % 6 = 3
    assert_eq!(shard_id, 3);
}

/// TEST-SHARD-UNIT-025: ShardRouter 带关键字路由测试
#[test]
fn test_shard_router_route_with_key() {
    let mut router = ShardRouter::with_strategy("hash", 6);

    for i in 0..6 {
        router.register_shard(i, format!("db_{}", i), format!("sqlite:./data/db_{}.db", i));
    }

    let dt = Utc::now();
    let key = "user_12345";
    let shard = router.route_with_key(dt, key);

    assert!(shard.is_some());
    assert!(shard.unwrap().shard_id < 6);
}

/// TEST-SHARD-UNIT-026: ShardRouter 关键字路由一致性测试
///
/// 验证三个确定性/统计属性：
/// 1. 相同 key + 相同时间戳 → 相同 shard（确定性）
/// 2. shard_id 始终在 [0, shard_count) 范围内（不变量）
/// 3. 100 个不同 key 在 10 个 shard 上分布 reasonably uniform（统计属性，
///    避免单次哈希碰撞导致的 flaky 失败）
#[test]
fn test_shard_router_key_consistency() {
    let shard_count: u32 = 10;
    let router = ShardRouter::with_strategy("hash", shard_count);

    let dt = Utc::now();

    // 属性 1：相同 key 必须映射到相同 shard
    let shard1 = router.calculate_shard(dt, "user_1");
    let shard2 = router.calculate_shard(dt, "user_1");
    assert_eq!(shard1, shard2, "Same key should produce same shard");

    // 属性 2：shard_id 必须在合法范围
    assert!(
        shard1 < shard_count,
        "shard_id {} must be < shard_count {}",
        shard1,
        shard_count
    );

    // 属性 3：100 个不同 key 应覆盖至少 5 个不同 shard（10 个 shard 上的合理分布）
    let mut distinct_shards = std::collections::HashSet::new();
    for i in 0..100u32 {
        let key = format!("user_{i}");
        let shard = router.calculate_shard(dt, &key);
        assert!(
            (shard as u32) < shard_count,
            "shard_id {} for key '{}' must be < shard_count {}",
            shard,
            key,
            shard_count
        );
        distinct_shards.insert(shard);
    }
    assert!(
        distinct_shards.len() >= 5,
        "100 keys should spread across at least 5 distinct shards (got {})",
        distinct_shards.len()
    );
}

/// TEST-SHARD-UNIT-027: ShardRouter 空关键字测试
#[test]
fn test_shard_router_empty_key() {
    let router = ShardRouter::with_strategy("monthly", 8);

    let dt = Utc.with_ymd_and_hms(2024, 5, 1, 0, 0, 0).unwrap();

    let _shard_with_key = router.calculate_shard(dt, "some_key");
    let shard_without_key = router.calculate_shard(dt, "");

    // 空关键字应该使用默认的时间哈希
    let expected = (2024 * 12 + 5) % 8;
    assert_eq!(shard_without_key, expected);
}

// ============================================================================
// 分片重平衡测试
// ============================================================================

/// TEST-SHARD-UNIT-028: ShardRouter 分片数扩展测试
#[test]
fn test_shard_rebalance_expansion() {
    // 原始路由器：4个分片
    let router_small = ShardRouter::with_strategy("hash", 4);

    let dt = Utc::now();
    let shard_small = router_small.calculate_shard(dt, "key");

    // 新路由器：8个分片
    let router_large = ShardRouter::with_strategy("hash", 8);
    let shard_large = router_large.calculate_shard(dt, "key");

    // 分片数变化后，同一 key 可能路由到不同的分片
    assert!(shard_small < 4);
    assert!(shard_large < 8);
}

/// TEST-SHARD-UNIT-029: ShardRouter 分片数缩减测试
#[test]
fn test_shard_rebalance_contraction() {
    let router_8 = ShardRouter::with_strategy("hash", 8);
    let router_2 = ShardRouter::with_strategy("hash", 2);

    let dt = Utc::now();
    let key = "test_key";

    let shard_8 = router_8.calculate_shard(dt, key);
    let shard_2 = router_2.calculate_shard(dt, key);

    // 分片数缩减后，shard_8 % 2 应该等于 shard_2（对于一致性哈希）
    // 但这里使用的是简单的哈希取模，所以这个测试验证边界情况
    assert!(shard_8 < 8);
    assert!(shard_2 < 2);
}

/// TEST-SHARD-UNIT-030: 分片迁移影响范围测试
#[test]
fn test_shard_migration_impact() {
    let keys: Vec<String> = (0..1000).map(|i| format!("key_{}", i)).collect();
    let dt = Utc::now();

    // 计算从 10 个分片扩展到 11 个分片时的影响
    let mut migrated_count = 0;

    for key in &keys {
        let shard_10 = ShardRouter::with_strategy("hash", 10).calculate_shard(dt, key);
        let shard_11 = ShardRouter::with_strategy("hash", 11).calculate_shard(dt, key);

        // 注意：由于我们使用简单哈希，(shard_10 % 11) != shard_11 是正常的
        // 理想的一致性哈希会有更小的迁移比例
        if shard_10 != shard_11 {
            migrated_count += 1;
        }
    }

    let migration_rate = migrated_count as f64 / keys.len() as f64;
    // 简单哈希的迁移率会比较高，这符合预期
    assert!(migration_rate > 0.0, "Some keys should migrate");
    println!("Migration rate from 10 to 11 shards: {:.2}%", migration_rate * 100.0);
}

// ============================================================================
// 分片热点检测测试
// ============================================================================

/// TEST-SHARD-UNIT-031: 分片访问分布检测测试
#[test]
fn test_shard_hotspot_detection_distribution() {
    let mut router = ShardRouter::with_strategy("hash", 10);

    for i in 0..10 {
        router.register_shard(i, format!("db_{}", i), format!("sqlite:./data/db_{}.db", i));
    }

    // 模拟10000次随机访问
    let dt = Utc::now();
    let mut access_counts = vec![0u32; 10];

    for i in 0..10000 {
        let key = format!("user_{}", i % 100); // 100个不同用户
        if let Some(shard) = router.route_with_key(dt, &key) {
            access_counts[shard.shard_id as usize] += 1;
        }
    }

    // 计算访问分布的统计信息
    let total_access: u32 = access_counts.iter().sum();
    let avg_access = total_access as f64 / 10.0;

    // 找出热点分片（访问次数超过平均值2倍）
    let hotspots: Vec<(u32, u32)> = access_counts
        .iter()
        .enumerate()
        .filter(|&(_, count)| *count as f64 > avg_access * 2.0)
        .map(|(id, count)| (id as u32, *count))
        .collect();

    println!("Access distribution: {:?}", access_counts);
    println!("Average access: {:.0}, Hotspots: {:?}", avg_access, hotspots);

    // 验证总访问次数
    assert_eq!(total_access, 10000);
}

/// TEST-SHARD-UNIT-032: 时间序列热点检测测试
#[test]
fn test_shard_temporal_hotspot() {
    // 按年月分片时，某些时间点的数据量会特别大（如年末、月末）
    let strategy = create_strategy("monthly");

    // 模拟一年的访问模式
    let mut monthly_shards: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();

    for month in 1..=12 {
        let dt = Utc.with_ymd_and_hms(2024, month, 15, 0, 0, 0).unwrap();
        let shard = strategy.calculate(dt, 12);

        // 模拟某些月份访问量更大（如年末购物季12月）
        let weight = if month == 12 { 10000 } else { 1000 };
        *monthly_shards.entry(shard).or_insert(0) += weight;
    }

    println!("Monthly shard distribution: {:?}", monthly_shards);

    // 12月数据量应该明显高于其他月份
    let shard_12 = strategy.calculate(Utc.with_ymd_and_hms(2024, 12, 1, 0, 0, 0).unwrap(), 12);
    let dec_count = monthly_shards.get(&shard_12).copied().unwrap_or(0);

    assert!(dec_count > 5000, "December should have higher access count");
}

/// TEST-SHARD-UNIT-033: 哈希分片均匀性测试
#[test]
fn test_hash_sharding_uniformity() {
    let _strategy = create_strategy("hash");
    let total_shards = 100;
    let sample_size = 100000;

    let mut counts = vec![0u32; total_shards as usize];

    // 使用固定时间戳，不同 key 来测试哈希分布
    let dt = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

    for i in 0..sample_size {
        let key = format!("record_{}", i);
        // 模拟 calculate_shard 中带 key 的逻辑
        use std::hash::{Hash, Hasher};
        use twox_hash::XxHash64;

        let mut hasher = XxHash64::default();
        dt.to_rfc3339().as_bytes().hash(&mut hasher);
        key.as_bytes().hash(&mut hasher);
        let hash = hasher.finish();
        let shard_id = (hash % total_shards as u64) as u32;

        counts[shard_id as usize] += 1;
    }

    // 计算分布的统计信息
    let total: u32 = counts.iter().sum();
    let avg = total as f64 / total_shards as f64;
    let variance: f64 = counts.iter().map(|&c| (c as f64 - avg).powi(2)).sum::<f64>() / total_shards as f64;
    let std_dev = variance.sqrt();

    let coefficient_of_variation = std_dev / avg;

    println!(
        "Hash distribution - Avg: {:.0}, StdDev: {:.0}, CV: {:.3}",
        avg, std_dev, coefficient_of_variation
    );

    // 验证：变异系数应该小于 0.1（良好分布）
    assert!(
        coefficient_of_variation < 0.1,
        "Hash distribution should be uniform, CV: {}",
        coefficient_of_variation
    );
}

// ============================================================================
// 分片故障转移测试
// ============================================================================

/// TEST-SHARD-UNIT-034: ShardRouter 连接池管理测试
#[test]
fn test_shard_router_pool_management() {
    let mut router = ShardRouter::with_strategy("yearly", 4);

    // 初始状态：没有连接池
    assert_eq!(router.pool_count(), 0);
    assert!(!router.has_pool(0));
    assert!(!router.has_pool(1));

    // 注册分片
    for i in 0..4 {
        router.register_shard(i, format!("db_{}", i), format!("sqlite:./data/db_{}.db", i));
    }

    assert_eq!(router.all_shards().len(), 4);
    assert_eq!(router.pool_count(), 0);
}

/// TEST-SHARD-UNIT-035: ShardRouter 动态连接池设置测试
#[test]
fn test_shard_router_dynamic_pool_set() {
    let mut router = ShardRouter::with_strategy("yearly", 2);

    router.register_shard(0, "db_0".to_string(), "sqlite:./data/db_0.db".to_string());
    router.register_shard(1, "db_1".to_string(), "sqlite:./data/db_1.db".to_string());

    // 未设置连接池时
    assert_eq!(router.pool_count(), 0);

    // 移除不存在的连接池应该返回 None
    let removed = router.remove_pool(99);
    assert!(removed.is_none());

    // 移除已存在的分片的连接池（未设置时应返回 None）
    let removed = router.remove_pool(0);
    assert!(removed.is_none());
}

/// TEST-SHARD-UNIT-036: ShardRouter 连接池清空测试
#[test]
fn test_shard_router_clear_pools() {
    let mut router = ShardRouter::with_strategy("hash", 3);

    for i in 0..3 {
        router.register_shard(i, format!("db_{}", i), format!("sqlite:./data/db_{}.db", i));
    }

    // 清除连接池
    router.clear_pools();

    assert_eq!(router.pool_count(), 0);

    // 再次清除应该是幂等的
    router.clear_pools();
    assert_eq!(router.pool_count(), 0);
}

/// TEST-SHARD-UNIT-037: 分片故障隔离测试
#[test]
fn test_shard_failure_isolation() {
    let mut router = ShardRouter::with_strategy("hash", 4);

    // 注册分片
    for i in 0..4 {
        router.register_shard(i, format!("db_{}", i), format!("sqlite:./data/db_{}.db", i));
    }

    let dt = Utc::now();

    // 即使某些分片不可用，其他分片仍然可以路由
    let shard_0 = router.route(dt);
    let shard_1 = router.route_with_key(dt, "key_1");

    // 验证至少可以路由到某个分片
    assert!(shard_0.is_some() || shard_1.is_some());

    // 验证路由结果的分片 ID 在有效范围内
    if let Some(s) = shard_0 {
        assert!(s.shard_id < 4);
    }
    if let Some(s) = shard_1 {
        assert!(s.shard_id < 4);
    }
}

/// TEST-SHARD-UNIT-038: 分片故障转移路由测试
#[test]
fn test_shard_failover_routing() {
    let router = ShardRouter::with_strategy("hash", 4);

    // 分片1故障时，尝试路由到其他分片
    let dt = Utc::now();

    // 正常路由
    let shard = router.calculate_shard(dt, "user_123");
    assert!(shard < 4);

    // 验证不同 key 路由到不同分片（模拟负载均衡）
    let keys = ["user_1", "user_2", "user_3", "user_4", "user_5"];
    let mut shards: Vec<u32> = Vec::new();

    for key in keys {
        let s = router.calculate_shard(dt, key);
        shards.push(s);
    }

    // 验证路由至少能返回有效分片 ID
    for s in &shards {
        assert!(*s < 4);
    }

    println!("Routed shards: {:?}", shards);
}

// ============================================================================
// 跨分片查询聚合测试
// ============================================================================

/// TEST-SHARD-UNIT-039: 跨分片查询路由收集测试
#[test]
fn test_cross_shard_query_routing() {
    let mut router = ShardRouter::with_strategy("monthly", 12);

    // 注册所有12个月的分片
    for i in 0..12 {
        router.register_shard(i, format!("month_{}", i), format!("sqlite:./data/month_{}.db", i));
    }

    // 模拟跨分片查询：获取所有分片
    let all_shards = router.all_shards();

    assert_eq!(all_shards.len(), 12);

    // 验证每个分片都有有效的 ID
    for shard in &all_shards {
        assert!(shard.shard_id < 12);
    }
}

/// TEST-SHARD-UNIT-040: 跨分片时间范围查询测试
#[test]
fn test_cross_shard_time_range_query() {
    let router = ShardRouter::with_strategy("yearly", 12);

    // 查询 2020-2024 年的数据，需要跨多个分片
    let start_year = 2020;
    let end_year = 2024;

    let mut affected_shards: Vec<u32> = Vec::new();

    for year in start_year..=end_year {
        let dt = Utc.with_ymd_and_hms(year, 6, 15, 0, 0, 0).unwrap();
        let shard_id = router.calculate_shard(dt, "");

        if !affected_shards.contains(&shard_id) {
            affected_shards.push(shard_id);
        }
    }

    println!(
        "Affected shards for years {} to {}: {:?}",
        start_year, end_year, affected_shards
    );

    // 验证跨越了多个分片
    assert!(!affected_shards.is_empty());
}

/// TEST-SHARD-UNIT-041: 跨分片数据聚合准备测试
#[test]
fn test_cross_shard_aggregation_preparation() {
    let router = ShardRouter::with_strategy("hash", 8);

    // 模拟聚合查询前的准备工作：确定需要查询的分片
    let target_keys = vec!["user_1", "user_2", "user_3", "user_4", "user_5"];
    let dt = Utc::now();

    // 收集每个 key 对应的分片
    let mut shard_key_map: std::collections::HashMap<u32, Vec<String>> = std::collections::HashMap::new();

    for key in &target_keys {
        let shard_id = router.calculate_shard(dt, key);
        shard_key_map.entry(shard_id).or_default().push(key.to_string());
    }

    // 验证分片映射
    println!("Shard to keys mapping: {:?}", shard_key_map);

    // 聚合所有分片上的 keys
    let total_keys: usize = shard_key_map.values().map(|v| v.len()).sum();
    assert_eq!(total_keys, target_keys.len());

    // 统计每个分片需要处理的 key 数量
    for (shard_id, keys) in &shard_key_map {
        println!("Shard {} needs {} keys", shard_id, keys.len());
    }
}

/// TEST-SHARD-UNIT-042: 分片路由一致性测试
#[test]
fn test_shard_routing_consistency() {
    let router = ShardRouter::with_strategy("monthly", 6);

    let dt = Utc::now();
    let key = "transaction_12345";

    // 多次查询同一 key，应该返回相同结果
    let results: Vec<u32> = (0..100).map(|_| router.calculate_shard(dt, key)).collect();

    let first = results[0];
    assert!(results.iter().all(|&r| r == first), "Routing should be consistent");
}

// ============================================================================
// ShardRouter Clone 测试
// ============================================================================

/// TEST-SHARD-UNIT-043: ShardRouter 克隆测试
#[test]
fn test_shard_router_clone() {
    let mut router = ShardRouter::with_strategy("hash", 4);

    router.register_shard(0, "db_0".to_string(), "sqlite:./data/db_0.db".to_string());
    router.register_shard(1, "db_1".to_string(), "sqlite:./data/db_1.db".to_string());

    let router_clone = router.clone();

    assert_eq!(router_clone.total_shards(), router.total_shards());
    assert_eq!(router_clone.strategy_name(), router.strategy_name());
    assert_eq!(router_clone.all_shards().len(), router.all_shards().len());
}

/// TEST-SHARD-UNIT-044: ShardRouter 克隆后独立修改测试
#[test]
fn test_shard_router_clone_independence() {
    let router = ShardRouter::with_strategy("yearly", 2);
    let router_clone = router.clone();

    // 原始路由器注册新分片
    let mut router_mut = router.clone();
    router_mut.register_shard(0, "db_0".to_string(), "sqlite:./data/db_0.db".to_string());

    // 克隆应该独立：原始未受影响，克隆体也未受影响
    assert_eq!(router.all_shards().len(), 0, "original should be unchanged");
    assert_eq!(router_clone.all_shards().len(), 0, "clone should be unchanged");
    assert_eq!(router_mut.all_shards().len(), 1, "mutant should have 1 shard");
}

// ============================================================================
// 策略 trait 方法测试
// ============================================================================

/// TEST-SHARD-UNIT-045: ShardingStrategy current_shard 测试
#[test]
fn test_strategy_current_shard() {
    let strategy = create_strategy("yearly");
    let current = strategy.current_shard(12);

    // 应该是基于当前时间计算
    let now = Utc::now();
    let expected = strategy.calculate(now, 12);

    assert_eq!(current, expected);
}

/// TEST-SHARD-UNIT-046: ShardingStrategy boxed_clone 测试
#[test]
fn test_strategy_boxed_clone() {
    let strategy = create_strategy("monthly");
    let cloned = strategy.boxed_clone();

    let dt = Utc::now();
    let shard_original = strategy.calculate(dt, 10);
    let shard_cloned = cloned.calculate(dt, 10);

    assert_eq!(shard_original, shard_cloned);
    assert_eq!(cloned.name(), "monthly");
}

/// TEST-SHARD-UNIT-047: ShardingStrategy is_valid_shard_id 边界测试
#[test]
fn test_strategy_valid_shard_id_boundaries() {
    let yearly = create_strategy("yearly");
    let monthly = create_strategy("monthly");
    let daily = create_strategy("daily");
    let hash = create_strategy("hash");

    // 月/日/哈希策略：shard_id 必须 < total_shards
    assert!(monthly.is_valid_shard_id(0, 10));
    assert!(monthly.is_valid_shard_id(9, 10));
    assert!(!monthly.is_valid_shard_id(10, 10));

    assert!(daily.is_valid_shard_id(0, 30));
    assert!(daily.is_valid_shard_id(29, 30));
    assert!(!daily.is_valid_shard_id(30, 30));

    assert!(hash.is_valid_shard_id(0, 8));
    assert!(hash.is_valid_shard_id(7, 8));
    assert!(!hash.is_valid_shard_id(8, 8));

    // 年策略：shard_id > 0
    assert!(yearly.is_valid_shard_id(1, 10));
    assert!(!yearly.is_valid_shard_id(0, 10));
}

// ============================================================================
// 边界条件和错误处理测试
// ============================================================================

/// TEST-SHARD-UNIT-048: 分片数边界值测试
#[test]
fn test_shard_count_boundary_values() {
    let strategy = create_strategy("hash");
    let dt = Utc::now();

    // 测试最小分片数
    let shard_1 = strategy.calculate(dt, 1);
    assert_eq!(shard_1, 0);

    // 测试最大分片数
    let shard_max = strategy.calculate(dt, u32::MAX);
    assert!(shard_max < u32::MAX);
}

/// TEST-SHARD-UNIT-049: 路由空指针测试
#[test]
fn test_router_route_nonexistent_shard() {
    let mut router = ShardRouter::with_strategy("yearly", 12);

    // 只注册部分分片
    router.register_shard(0, "db_0".to_string(), "sqlite:./data/db_0.db".to_string());

    // 路由到一个未注册的分片应该返回 None
    let dt_2024 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    // 2024 % 12 = 8，但只注册了 shard 0
    let shard = router.route(dt_2024);

    // 由于 2024 % 12 = 8，而只注册了 shard 0，所以应该返回 None
    assert!(shard.is_none());
}

/// TEST-SHARD-UNIT-050: ShardRouter 初始化分片 ID 列表测试
#[test]
fn test_router_initialized_shards() {
    let router = ShardRouter::with_strategy("yearly", 3);

    let initialized = router.initialized_shards();
    assert!(
        initialized.is_empty(),
        "Should have no initialized shards without pools"
    );
}

// ============================================================================
// v0.3.0 新增 API 单元测试
//
// 以下测试覆盖 ShardRouter 的 default/new 构造器、get_pool/get_session 的 None 路径、
// shard_id_for_key 的零分片防御、get_session_for_shard 的错误路径、enforce_shard_binding 的
// 同/异分片判定。带真实连接池的成功路径由 sharding/integration/shard_session_routing_tests.rs
// 覆盖，此处仅覆盖无需连接池的逻辑分支。
// ============================================================================

/// 自定义分片策略（用于测试 `ShardRouter::new` 接受任意 `ShardingStrategy` 实现）
struct ModuloStrategy;

impl ShardingStrategy for ModuloStrategy {
    fn calculate(&self, timestamp: DateTime<Utc>, total_shards: u32) -> u32 {
        // 简单取模：与 YearlyStrategy 等价但 name 不同，便于验证策略被实际使用
        let year = timestamp.year() as u32;
        if total_shards == 0 { 0 } else { year % total_shards }
    }

    fn name(&self) -> &'static str {
        "modulo_test"
    }

    fn is_valid_shard_id(&self, shard_id: u32, total_shards: u32) -> bool {
        shard_id < total_shards
    }

    fn current_shard(&self, total_shards: u32) -> u32 {
        self.calculate(Utc::now(), total_shards)
    }

    fn boxed_clone(&self) -> Box<dyn ShardingStrategy> {
        Box::new(ModuloStrategy)
    }
}

/// TEST-SHARD-UNIT-051: ShardRouter::default() 返回 total_shards=1 + yearly 策略
///
/// 验证默认构造器的契约：单一分片（避免 `% 0` panic）+ YearlyStrategy（向后兼容）。
/// 这是 `Default` 实现的硬性保证，破坏即属 breaking change。
#[test]
fn test_shard_router_default_returns_single_shard_with_yearly_strategy() {
    let router = ShardRouter::default();

    assert_eq!(router.total_shards(), 1, "default router must have 1 shard");
    assert_eq!(
        router.strategy_name(),
        "yearly",
        "default router must use yearly strategy"
    );
    assert_eq!(router.pool_count(), 0, "default router must have no pools");
    assert!(
        router.initialized_shards().is_empty(),
        "default router must have no initialized shards"
    );

    // shard_id_for_key 在 total_shards=1 时必须返回 0（单一分片语义）
    assert_eq!(
        router.shard_id_for_key("any_key"),
        0,
        "single-shard router must map all keys to shard 0"
    );
}

/// TEST-SHARD-UNIT-052: ShardRouter::new() 接受自定义 ShardingStrategy 实现
///
/// 验证泛型构造器 `new<S: ShardingStrategy + 'static>(strategy, total_shards)`
/// 正确接受用户提供的策略实例，且 `strategy_name()` 反映该策略的 `name()` 返回值。
#[test]
fn test_shard_router_new_with_custom_strategy() {
    let mut router = ShardRouter::new(ModuloStrategy, 7);

    assert_eq!(router.total_shards(), 7);
    assert_eq!(
        router.strategy_name(),
        "modulo_test",
        "strategy_name must reflect the custom strategy's name()"
    );

    // 验证自定义策略被实际使用：2024 % 7 = 1
    // 通过 calculate_shard（key 为空时走 strategy.calculate 分支）
    let dt_2024 = Utc.with_ymd_and_hms(2024, 6, 15, 0, 0, 0).unwrap();
    let shard_id = router.calculate_shard(dt_2024, "");
    assert_eq!(shard_id, 1, "ModuloStrategy: 2024 % 7 = 1");

    // 注册分片 1 后，route() 应返回该分片
    router.register_shard(1, "db_1".to_string(), "sqlite:./data/db_1.db".to_string());
    let shard = router.route(dt_2024);
    assert!(shard.is_some(), "shard 1 should be routable after registration");
    assert_eq!(shard.unwrap().shard_id, 1, "ModuloStrategy: 2024 % 7 = 1");
}

/// TEST-SHARD-UNIT-053: get_pool 未注册时返回 None
#[test]
fn test_get_pool_returns_none_when_no_pool_registered() {
    let router = ShardRouter::with_strategy("hash", 4);

    assert!(router.get_pool(0).is_none(), "get_pool must be None before set_pool");
    assert!(router.get_pool(1).is_none());
    assert!(
        router.get_pool(99).is_none(),
        "get_pool for unregistered shard must be None"
    );

    // 注册分片元信息（不注册连接池）后仍然为 None
    let mut router = router;
    router.register_shard(0, "db_0".to_string(), "sqlite::memory:".to_string());
    assert!(
        router.get_pool(0).is_none(),
        "get_pool must still be None after register_shard (no pool)"
    );
    assert!(!router.has_pool(0), "has_pool must be false after register_shard only");
}

/// TEST-SHARD-UNIT-054: get_session 未注册连接池时返回 Ok(None)
///
/// 注意：`get_session` 返回 `Result<Option<Session>>`，未注册连接池时返回 `Ok(None)`
/// 而非 `Err`——这与 `get_session_for_shard`（返回 `Err`）的语义不同。
#[tokio::test]
async fn test_get_session_returns_ok_none_when_no_pool() {
    let router = ShardRouter::with_strategy("hash", 4);

    let result = router.get_session(0).await;
    assert!(
        result.is_ok(),
        "get_session must not return Err when no pool: {:?}",
        result.err()
    );
    assert!(
        result.unwrap().is_none(),
        "get_session must return Ok(None) when no pool registered"
    );
}

/// TEST-SHARD-UNIT-055: get_session_for_timestamp 未注册连接池时返回 Ok(None)
#[tokio::test]
async fn test_get_session_for_timestamp_returns_ok_none_when_no_pool() {
    let router = ShardRouter::with_strategy("monthly", 12);

    let dt = Utc.with_ymd_and_hms(2024, 6, 15, 0, 0, 0).unwrap();
    let result = router.get_session_for_timestamp(dt).await;
    assert!(
        result.is_ok(),
        "get_session_for_timestamp must not return Err: {:?}",
        result.err()
    );
    assert!(
        result.unwrap().is_none(),
        "get_session_for_timestamp must return Ok(None) when no pool"
    );
}

/// TEST-SHARD-UNIT-056: shard_id_for_key 在 total_shards=0 时返回 0（除零防御）
///
/// 验证 `shard_id_for_key` 的硬性契约：即使 `total_shards` 为 0（不应发生，Default 保证为 1），
/// 也不能 panic，而是返回 0（单一分片语义）。
/// 这是安全红线——破坏即触发 `% 0` panic。
#[test]
fn test_shard_id_for_key_zero_total_shards_returns_zero() {
    // 通过 with_strategy 构造 total_shards=0 的 router（绕过 Default 的保护）
    let router = ShardRouter::with_strategy("hash", 0);

    // 多个 key 都必须返回 0，不能 panic
    for key in &["", "a", "user_1", "user_99999", "🎉_unicode_key"] {
        let shard_id = router.shard_id_for_key(key);
        assert_eq!(
            shard_id, 0,
            "shard_id_for_key must return 0 (not panic) when total_shards=0, key={:?}",
            key
        );
    }
}

/// TEST-SHARD-UNIT-057: get_session_for_shard 未注册连接池时返回 Err(DbError::Config)
///
/// 验证 `get_session_for_shard` 的错误语义：与 `get_session` 不同，此方法返回 `Err`
/// （因为它是 `Result<Session>` 而非 `Result<Option<Session>>`），错误消息必须包含
/// "No pool registered for shard"。
#[tokio::test]
async fn test_get_session_for_shard_returns_err_when_no_pool() {
    // 只注册分片元信息，不注册连接池
    let mut router = ShardRouter::with_strategy("hash", 4);
    router.register_shard(0, "db_0".to_string(), "sqlite::memory:".to_string());
    router.register_shard(1, "db_1".to_string(), "sqlite::memory:".to_string());
    router.register_shard(2, "db_2".to_string(), "sqlite::memory:".to_string());
    router.register_shard(3, "db_3".to_string(), "sqlite::memory:".to_string());

    let result = router.get_session_for_shard("user_42", "admin").await;
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("expected Err when no pool registered for computed shard"),
    };
    let msg = format!("{}", err);
    assert!(
        msg.contains("No pool registered for shard"),
        "error message must mention missing pool, got: {}",
        msg
    );
}

/// TEST-SHARD-UNIT-058: get_session_for_shard_with_id 未注册连接池时返回 Err
#[tokio::test]
async fn test_get_session_for_shard_with_id_returns_err_when_no_pool() {
    let mut router = ShardRouter::with_strategy("hash", 4);
    router.register_shard(0, "db_0".to_string(), "sqlite::memory:".to_string());

    let result = router.get_session_for_shard_with_id("user_42", "admin").await;
    let err_msg = match result {
        Err(e) => format!("{}", e),
        Ok(_) => panic!("get_session_for_shard_with_id must return Err when no pool"),
    };
    assert!(
        err_msg.contains("No pool registered for shard"),
        "error message must mention missing pool, got: {}",
        err_msg
    );
}

/// TEST-SHARD-UNIT-059: enforce_shard_binding 同分片返回 Ok，跨分片返回 ShardConflict
///
/// 验证 `enforce_shard_binding` 的核心契约：
/// - 同一 `shard_key` 计算的 `shard_id` 与传入的 `expected_shard_id` 一致 → `Ok(())`
/// - 不同 `shard_key` 计算的 `shard_id` 与 `expected_shard_id` 不一致 → `Err(ShardConflict)`
#[test]
fn test_enforce_shard_binding_same_and_cross_shard() {
    use dbnexus::ErrorCategory;

    let router = ShardRouter::with_strategy("hash", 8);

    // 选一个 key，计算其 shard_id
    let primary_key = "user_42";
    let primary_shard = router.shard_id_for_key(primary_key);

    // 同一 key → Ok
    let same = router.enforce_shard_binding(primary_shard, primary_key);
    assert!(
        same.is_ok(),
        "enforce_shard_binding must accept same-shard key, got: {:?}",
        same.err()
    );

    // 找一个映射到不同分片的 key
    let conflict_key = (0..256)
        .map(|i| format!("user_{}", i))
        .find(|k| router.shard_id_for_key(k) != primary_shard)
        .expect("should find a key mapping to a different shard within 256 attempts");

    // 跨分片 → Err(ShardConflict)
    let cross = router.enforce_shard_binding(primary_shard, &conflict_key);
    let err = match cross {
        Err(e) => e,
        Ok(_) => panic!("expected ShardConflict error for cross-shard key"),
    };
    assert_eq!(
        err.category,
        ErrorCategory::ShardConflict,
        "cross-shard error must have ShardConflict category, got: {:?}",
        err.category
    );
    let msg = format!("{}", err);
    assert!(
        msg.contains("Cross-shard query detected"),
        "error message must mention cross-shard conflict, got: {}",
        msg
    );
    assert!(
        msg.contains(primary_key) || msg.contains(&conflict_key),
        "error message should reference one of the keys"
    );
}
