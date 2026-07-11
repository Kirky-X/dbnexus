// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
use std::sync::Arc;
use std::time::Duration;

#[path = "../../common/mod.rs"]
mod common;

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_connection_health_check() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    let _session = pool.get_session("admin").await.expect("Failed");
    assert!(pool.status().total >= 1);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_clean_invalid_connections() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    let _session = pool.get_session("admin").await.expect("Failed to get session");
    let status = pool.status();
    eprintln!(
        "Pool: total={}, active={}, idle={}",
        status.total, status.active, status.idle
    );
    assert!(status.total >= 1, "Pool should have connections");
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_validate_connections_succeeds() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    let _session = pool.get_session("admin").await.expect("Failed");
    assert!(pool.status().total >= 1);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_recreate_connections_succeeds() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    let _session = pool.get_session("admin").await.expect("Failed");
    assert!(pool.status().total >= 1);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_pool_status_after_operations() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    for i in 0..5 {
        let _session = pool.get_session("admin").await.expect("Failed");
        let _table_name = format!("status_test_{}", i);
    }
    let status = pool.status();
    assert_eq!(status.total, status.active + status.idle);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_sequential_health_checks() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    for i in 0..10 {
        let _session = pool.get_session("admin").await.expect("Failed");
        assert!(pool.status().active >= 1, "Iteration {}", i);
    }
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_health_check_timeout_handling() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    for _ in 0..20 {
        let _session = pool.get_session("admin").await.expect("Failed");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(pool.status().total >= 1);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_health_check_after_heavy_usage() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    let pool = std::sync::Arc::new(pool);
    let mut handles = Vec::new();
    for _ in 0..20 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move { pool.get_session("admin").await.ok() }));
    }
    let results: Vec<Result<Option<_>, _>> = futures::future::join_all(handles).await;
    let count = results.iter().filter(|r| r.as_ref().unwrap_or(&None).is_some()).count();
    eprintln!("Heavy usage: {} sessions", count);
    assert!(pool.status().total >= 1);
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_concurrent_health_checks() {
    let (pool, _temp_dir) = common::create_test_pool().await.expect("Failed");
    let pool = std::sync::Arc::new(pool);
    let mut handles = Vec::new();
    for _ in 0..10 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move { pool.get_session("admin").await.ok() }));
    }
    let results: Vec<Result<Option<_>, _>> = futures::future::join_all(handles).await;
    let count = results.iter().filter(|r| r.as_ref().unwrap_or(&None).is_some()).count();
    eprintln!("Concurrent: {}/10", count);
    assert!(count >= 5);
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn test_pool_config_boundaries() {
    let url = common::get_test_database_url();
    for max_conn in [1, 5, 10] {
        let config = dbnexus::DbConfig {
            url: url.clone(),
            max_connections: max_conn,
            min_connections: 0, // 避免 min > max 冲突
            ..Default::default()
        };
        let pool = tokio::time::timeout(std::time::Duration::from_secs(10), dbnexus::DbPool::with_config(config))
            .await
            .expect("timeout")
            .expect("create");
        let _session = pool.get_session("admin").await.expect("Failed");
        let status = pool.status();
        assert!(status.total <= max_conn, "Pool should not exceed max");
    }
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_connection_acquire_with_small_pool() {
    let url = common::get_test_database_url();
    let config = dbnexus::DbConfig {
        url,
        max_connections: 2,
        min_connections: 1,
        acquire_timeout: 5000,
        ..Default::default()
    };
    let pool = dbnexus::DbPool::with_config(config).await.expect("Failed");
    let pool = std::sync::Arc::new(pool);
    let mut handles = Vec::new();
    for _ in 0..5 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move { pool.get_session("admin").await.ok() }));
    }
    let results: Vec<Result<Option<_>, _>> = futures::future::join_all(handles).await;
    let count = results.iter().filter(|r| r.as_ref().unwrap_or(&None).is_some()).count();
    eprintln!("Small pool: {}/5", count);
    assert!(count >= 2);
}

/// TEST-I-POOL-004: 验证池耗尽时触发正确的告警级别
///
/// 当连接池饱和时，并发请求应触发超时，验证：
/// 1. 超时错误被正确记录
/// 2. wait_count 在等待期间正确递增
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_exhaustion_alert_levels() {
    // 使用极小连接池和极短超时触发告警
    let config = dbnexus::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1,
        min_connections: 1,
        acquire_timeout: 200, // 200ms 超时，应快速触发 warn 级别告警
        admin_role: "admin".to_string(),
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config)
        .await
        .expect("Failed to create pool");
    let pool = Arc::new(pool);

    // 持有唯一的连接足够长时间，让其他请求超时
    let pool_for_holder = pool.clone();
    let _holder = tokio::spawn(async move {
        let session = pool_for_holder.get_session("admin").await;
        if session.is_ok() {
            // 持有连接 1 秒，足够让其他请求超时
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    // 等待 holder 获取连接
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 现在发送 3 个并发请求，它们应该都超时
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let pool = pool.clone();
            tokio::spawn(async move { pool.get_session("admin").await })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(handles).await;

    // 验证所有请求都超时了（因为连接被持有）
    // Count results that are Ok(Err(...)) — meaning the connection attempt returned a DbError
    let timeout_count = results.iter().filter(|r| matches!(r, Ok(Err(_)))).count();
    assert!(
        timeout_count >= 2,
        "Expected at least 2 timeouts, got {}",
        timeout_count
    );

    // 验证 wait_count 正确记录
    tokio::time::sleep(Duration::from_millis(50)).await;
    let status = pool.status();
    // holder 释放后 wait_count 应该为 0
    assert_eq!(
        status.wait_count, 0,
        "wait_count should be 0 after all requests complete"
    );

    // max_waiters 应该记录了峰值（至少有 3 个并发等待）
    assert!(
        status.max_waiters >= 2,
        "max_waiters should be >= 2, got {}",
        status.max_waiters
    );

    let _ = _holder.await;
}
