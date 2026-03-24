// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Sharding 集成测试
//!
//! 测试分片模块的时间边界、哈希分布均匀性、路由器高级功能等

use chrono::{TimeZone, Utc};
use dbnexus::{ShardConfig, ShardRouter};
use std::sync::Arc;

fn get_database_url() -> Option<String> {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        return Some(url);
    }

    if cfg!(feature = "sqlite") {
        return Some("sqlite::memory:".to_string());
    }

    None
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

    let router = ShardRouter::with_config_sync(&config);

    let total = router.total_shards();
    let strategy = router.strategy_name();

    assert!(total > 0, "Should have shards configured");
    assert!(!strategy.is_empty(), "Should have a strategy name");

    let shards = router.all_shards();
    assert!(!shards.is_empty(), "Should have some shards");

    println!("Total shards: {}, Strategy: {}", total, strategy);
}

/// TEST-SHARD-016: ShardRouter 异步初始化测试
#[tokio::test]
async fn test_shard_router_async_init() {
    let Some(url) = get_database_url() else {
        return;
    };

    let config = ShardConfig::new("yearly", 2, "test", &url);
    let router = ShardRouter::with_config(&config).await.unwrap();
    assert_eq!(router.total_shards(), 2);
    assert_eq!(router.strategy_name(), "yearly");
}

/// TEST-SHARD-017: ShardRouter 连接池管理测试
#[tokio::test]
async fn test_shard_router_pool_management() {
    let Some(url) = get_database_url() else {
        return;
    };

    let config = ShardConfig::new("yearly", 3, "test", &url);
    let router = ShardRouter::with_config(&config).await.unwrap();

    assert_eq!(router.pool_count(), 3);
    let initialized = router.initialized_shards();
    assert_eq!(initialized.len(), 3);

    for shard_id in 0..3 {
        assert!(router.has_pool(shard_id));
        assert!(router.get_pool(shard_id).is_some());
    }
}

/// TEST-SHARD-018: ShardRouter 同步初始化测试
#[test]
fn test_shard_router_sync_init() {
    let config = ShardConfig::new("monthly", 4, "data", "sqlite:./test_data/{shard}.db");
    let router = ShardRouter::with_config_sync(&config);

    assert_eq!(router.total_shards(), 4);
    assert_eq!(router.strategy_name(), "monthly");
    assert_eq!(router.pool_count(), 0);
    assert_eq!(router.all_shards().len(), 4);
}

/// TEST-SHARD-019: ShardRouter 动态注册连接池测试
#[tokio::test]
async fn test_shard_router_dynamic_pool_registration() {
    let config = ShardConfig::new("yearly", 2, "dynamic", "sqlite:./test_dynamic_{shard}.db");
    let mut router = ShardRouter::with_config_sync(&config);

    assert_eq!(router.pool_count(), 0);

    let Some(url) = get_database_url() else {
        return;
    };

    let pool0 = dbnexus::DbPool::new(&url).await.unwrap();
    router.set_pool(0, Arc::new(pool0)).unwrap();

    assert_eq!(router.pool_count(), 1);
    assert!(router.has_pool(0));
    assert!(!router.has_pool(1));

    let pool1 = dbnexus::DbPool::new(&url).await.unwrap();
    router.set_pool(1, Arc::new(pool1)).unwrap();

    assert_eq!(router.pool_count(), 2);

    let pool2 = dbnexus::DbPool::new(&url).await.unwrap();
    let result = router.set_pool(99, Arc::new(pool2));
    assert!(result.is_err());
}

/// TEST-SHARD-020: ShardRouter 克隆测试
#[tokio::test]
async fn test_shard_router_clone() {
    let Some(url) = get_database_url() else {
        return;
    };

    let config = ShardConfig::new("yearly", 2, "clone_test", &url);
    let router = ShardRouter::with_config(&config).await.unwrap();

    let router_clone = router.clone();

    assert_eq!(router_clone.total_shards(), router.total_shards());
    assert_eq!(router_clone.strategy_name(), router.strategy_name());
    assert_eq!(router_clone.all_shards().len(), router.all_shards().len());
    // 克隆后的路由器共享连接池 Arc 引用
    assert_eq!(router_clone.pool_count(), router.pool_count());
}
