// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 结构化健康导出（DbPool::health_snapshot）测试
//!
//! 需要 sqlite + health-check + runtime-tokio-rustls feature。

#![cfg(all(
    feature = "runtime-tokio-rustls",
    feature = "sqlite",
    feature = "health-check"
))]

use std::sync::Arc;

fn temp_db_url(tag: &str) -> (String, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("dbnexus_t406_{}_{}.db", tag, std::process::id()));
    (format!("sqlite:{}?mode=rwc", path.display()), path)
}

// 快照结构与池饱和度

#[tokio::test]
async fn test_health_snapshot_shape_and_fields() {
    let (url, path) = temp_db_url("shape");
    let pool = Arc::new(dbnexus::DbPool::new(&url).await.unwrap());

    // 触发一次真实查询，让连接池出现活跃/归还计数
    let _rows = pool.query_rows("SELECT 1 AS x", "admin").await;

    let snap = pool.health_snapshot().await;
    let obj = snap
        .as_object()
        .expect("health_snapshot should return a JSON object");

    // 顶层字段：status + pool + slow_queries + replicas
    assert!(
        obj.contains_key("status")
            && obj.contains_key("pool")
            && obj.contains_key("slow_queries")
            && obj.contains_key("replicas"),
        "the snapshot should contain status/pool/slow_queries/replicas, actual keys: {:?}",
        obj.keys().collect::<Vec<_>>()
    );

    // status 为三态字符串之一
    let status = snap["status"].as_str().unwrap();
    assert!(
        ["healthy", "degraded", "unhealthy"].contains(&status),
        "status should be healthy/degraded/unhealthy, actual: {status}"
    );

    // pool 子对象：饱和度字段与 PoolStatus 对齐
    let pool_obj = &snap["pool"];
    assert!(pool_obj["total"].is_u64(), "pool.total is missing");
    assert!(pool_obj["active"].is_u64(), "pool.active is missing");
    assert!(pool_obj["idle"].is_u64(), "pool.idle is missing");
    assert!(
        pool_obj["wait_count"].is_u64(),
        "pool.wait_count is missing"
    );
    assert!(
        pool_obj["max_connections"].is_u64(),
        "pool.max_connections is missing"
    );
    let saturation = pool_obj["saturation"]
        .as_f64()
        .expect("pool.saturation is missing");
    assert!(
        (0.0..=1.0).contains(&saturation),
        "saturation should be within [0,1], actual: {saturation}"
    );

    // slow_queries 子对象
    assert!(
        snap["slow_queries"]["count"].is_u64(),
        "slow_queries.count is missing"
    );

    // replicas 为数组（无副本提供者时为空数组）
    assert!(
        snap["replicas"].as_array().unwrap().is_empty(),
        "replicas should be an empty array when no replica provider is registered"
    );

    // 与既有 status() 数据一致（复用口径）
    let st = pool.status();
    assert_eq!(snap["pool"]["total"].as_u64().unwrap(), st.total as u64);
    assert_eq!(snap["pool"]["active"].as_u64().unwrap(), st.active as u64);

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn test_health_snapshot_unhealthy_without_connections() {
    let (url, path) = temp_db_url("unhealthy");
    // 显式 min_connections=0：被测语义是「零连接池 → unhealthy」。
    // pool-warmup 特性下 DbPool::new 会预热 min_connections（默认 5）个连接，
    // 那时的池确实 healthy——测试必须显式构造零连接前提才能在两种特性组合下
    // 稳定成立（workspace 特性统一会意外开启 pool-warmup）。
    let config = dbnexus::DbConfig {
        url: url.clone(),
        pool_config: dbnexus::PoolConfig {
            min_connections: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let pool = dbnexus::DbPool::with_config(config).await.unwrap();

    // 未 warmup、未使用的池：total == 0 → unhealthy
    let snap = pool.health_snapshot().await;
    assert_eq!(
        snap["status"].as_str().unwrap(),
        "unhealthy",
        "a zero-connection pool should report unhealthy, actual: {snap}"
    );
    assert_eq!(snap["pool"]["total"].as_u64().unwrap(), 0);
    assert_eq!(
        snap["pool"]["saturation"].as_f64().unwrap(),
        1.0,
        "a zero-connection pool's saturation should be 1.0 (full-load semantics)"
    );

    let _ = std::fs::remove_file(&path);
}

// 副本状态提供者注入（副本路由可接入）

#[tokio::test]
async fn test_health_snapshot_replica_provider() {
    let (url, path) = temp_db_url("replicas");
    let pool = Arc::new(dbnexus::DbPool::new(&url).await.unwrap());

    // 注册副本状态提供者（mock：两个副本条目）
    pool.set_replica_health_provider(Some(Arc::new(|| {
        vec![
            serde_json::json!({"url": "replica-a", "healthy": true, "lag_ms": 5.0}),
            serde_json::json!({"url": "replica-b", "healthy": false, "lag_ms": null}),
        ]
    })))
    .await;

    let snap = pool.health_snapshot().await;
    let replicas = snap["replicas"].as_array().unwrap();
    assert_eq!(
        replicas.len(),
        2,
        "the replica provider's entries should appear in the snapshot"
    );
    assert_eq!(replicas[0]["url"], "replica-a");
    assert_eq!(replicas[0]["healthy"], true);
    assert_eq!(replicas[1]["url"], "replica-b");
    assert_eq!(replicas[1]["healthy"], false);

    // 清除提供者 → 回到空数组
    pool.set_replica_health_provider(None).await;
    let snap = pool.health_snapshot().await;
    assert!(snap["replicas"].as_array().unwrap().is_empty());

    let _ = std::fs::remove_file(&path);
}

// 慢查询计数（metrics collector 注入路径）

#[cfg(feature = "metrics")]
#[tokio::test]
async fn test_health_snapshot_slow_query_count_from_metrics() {
    let (url, path) = temp_db_url("slow");
    let pool = Arc::new(dbnexus::DbPool::new(&url).await.unwrap());

    // 注入 metrics collector 并记一条慢查询（threshold=1ms，100ms 查询必中）
    let collector = Arc::new(dbnexus::observability::MetricsCollector::new());
    collector.set_slow_query_threshold(1);
    collector.record_query("SELECT", std::time::Duration::from_millis(100), true, None);
    pool.set_metrics_collector(Some(collector.clone())).await;

    let snap = pool.health_snapshot().await;
    assert_eq!(
        snap["slow_queries"]["count"].as_u64().unwrap(),
        1,
        "the slow query count should come from the metrics collector"
    );
    assert_eq!(
        snap["slow_queries"]["threshold_ms"].as_u64().unwrap(),
        1,
        "the slow query threshold should be exported with the snapshot"
    );

    let _ = std::fs::remove_file(&path);
}
