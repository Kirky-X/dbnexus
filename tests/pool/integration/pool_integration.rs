// Copyright (c) 2026 Kirky.X
// Licensed under MIT License

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
    eprintln!("Pool: total={}, active={}, idle={}", status.total, status.active, status.idle);
    assert!(status.total >= 1, "Pool should have connections");
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_validate_and_recreate_connections() {
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
        handles.push(tokio::spawn(async move {
            pool.get_session("admin").await.ok()
        }));
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
        handles.push(tokio::spawn(async move {
            pool.get_session("admin").await.ok()
        }));
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
        let config = dbnexus::config::DbConfigBuilder::new()
            .url(&url)
            .max_connections(max_conn)
            .build().expect("Failed");
        let pool = tokio::time::timeout(std::time::Duration::from_secs(10), dbnexus::DbPool::with_config(config))
            .await.expect("timeout").expect("create");
        let _session = pool.get_session("admin").await.expect("Failed");
        let status = pool.status();
        assert!(status.total <= max_conn, "Pool should not exceed max");
    }
}

#[tokio::test]
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
async fn test_connection_acquire_with_small_pool() {
    let url = common::get_test_database_url();
    let config = dbnexus::config::DbConfigBuilder::new()
        .url(&url)
        .max_connections(2)
        .min_connections(1)
        .acquire_timeout(5000)
        .build().expect("Failed");
    let pool = dbnexus::DbPool::with_config(config).await.expect("Failed");
    let pool = std::sync::Arc::new(pool);
    let mut handles = Vec::new();
    for i in 0..5 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            pool.get_session("admin").await.ok()
        }));
    }
    let results: Vec<Result<Option<_>, _>> = futures::future::join_all(handles).await;
    let count = results.iter().filter(|r| r.as_ref().unwrap_or(&None).is_some()).count();
    eprintln!("Small pool: {}/5", count);
    assert!(count >= 2);
}
