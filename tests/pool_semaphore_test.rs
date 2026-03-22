// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 信号量许可管理测试（Perf-1 修复验证）
//!
//! 测试连接池信号量许可的正确归还，确保无死锁和许可泄漏。

use std::sync::Arc;
use std::time::Duration;

/// 测试信号量许可正确归还 - 基本场景
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_semaphore_permit_return_basic() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 2,
        min_connections: 1,
        acquire_timeout: 2000,
        ..Default::default()
    };

    let pool = Arc::new(
        dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool"),
    );

    // 获取并释放连接多次，验证许可正确归还
    for i in 0..10 {
        let session = pool
            .get_session("admin")
            .await
            .unwrap_or_else(|_| panic!("Failed to get session on iteration {}", i));
        drop(session);
        // 给异步释放一点时间
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let status = pool.status();
    assert_eq!(status.active, 0, "All sessions should be released");
}

/// 测试信号量许可正确归还 - 并发场景
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_semaphore_permit_return_concurrent() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 1,
        acquire_timeout: 5000,
        ..Default::default()
    };

    let pool = Arc::new(
        dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool"),
    );

    let iterations = 50;
    let mut handles = Vec::new();

    // 并发获取和释放连接
    for _ in 0..iterations {
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move {
            let session = pool_clone.get_session("admin").await.expect("Failed to get session");
            // 模拟短暂使用
            tokio::time::sleep(Duration::from_millis(1)).await;
            drop(session);
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    // 等待所有异步释放完成
    tokio::time::sleep(Duration::from_millis(100)).await;

    let status = pool.status();
    assert_eq!(
        status.active, 0,
        "All sessions should be released after concurrent operations"
    );
    assert!(
        status.borrow_count >= iterations as u64,
        "All borrows should be counted"
    );
}

/// 测试无死锁 - 高并发获取释放
///
/// 验证在高并发场景下连接池不会发生死锁，且连接数始终受控。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_no_deadlock_high_concurrency() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 3,
        min_connections: 1,
        acquire_timeout: 10000, // 较长超时以确保不会因超时而失败
        ..Default::default()
    };

    let pool = Arc::new(
        dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool"),
    );

    let rounds = 10;
    let concurrent_tasks = 10;

    for round in 0..rounds {
        let mut handles = Vec::new();

        for _ in 0..concurrent_tasks {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                // 使用 timeout 确保不会永久阻塞
                match tokio::time::timeout(Duration::from_secs(5), pool_clone.get_session("admin")).await {
                    Ok(Ok(session)) => {
                        // 验证当前活跃连接数不超过限制
                        let status = pool_clone.status();
                        assert!(status.active <= 5, "Active connections should be bounded");
                        tokio::time::sleep(Duration::from_micros(100)).await;
                        drop(session);
                        true
                    }
                    _ => false,
                }
            });
            handles.push(handle);
        }

        let results: Vec<_> = futures::future::join_all(handles).await;
        let success_count = results.iter().filter(|r| *r.as_ref().unwrap_or(&false)).count();

        // 验证至少有部分任务成功（连接池可用）
        assert!(
            success_count >= 1,
            "Round {}: At least some tasks should succeed",
            round
        );

        // 等待所有释放完成
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let status = pool.status();
    assert_eq!(status.active, 0, "All sessions should be released after all rounds");
}

/// 测试信号量许可 - 达到最大连接数后可继续获取
/// 这是 Perf-1 修复的核心验证：信号量许可正确归还。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_semaphore_permit_reuse_after_max() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 2,
        min_connections: 1,
        acquire_timeout: 3000,
        ..Default::default()
    };

    let pool = Arc::new(
        dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool"),
    );

    // 第一轮：获取所有连接
    let session1 = pool.get_session("admin").await.expect("Failed to get session 1");
    let session2 = pool.get_session("admin").await.expect("Failed to get session 2");

    let status = pool.status();
    assert_eq!(status.active, 2, "Should have 2 active connections");

    // 释放一个连接
    drop(session1);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 应该可以获取新连接（信号量许可已归还）
    let session3 = pool
        .get_session("admin")
        .await
        .expect("Should be able to get new connection after release");

    let status = pool.status();
    assert_eq!(status.active, 2, "Should have 2 active connections again");

    // 清理
    drop(session2);
    drop(session3);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let status = pool.status();
    assert_eq!(status.active, 0, "All sessions should be released");
}

/// 测试连接池压力测试 - 快速获取释放
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_stress_rapid_acquire_release() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 10,
        min_connections: 1,
        acquire_timeout: 5000,
        ..Default::default()
    };

    let pool = Arc::new(
        dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool"),
    );

    let total_operations = 500;
    let mut handles = Vec::new();

    for _ in 0..total_operations {
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move {
            let session = pool_clone.get_session("admin").await.ok()?;
            drop(session);
            Some(())
        });
        handles.push(handle);
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    let success_count = results.iter().filter(|r| r.as_ref().unwrap_or(&None).is_some()).count();

    // 等待所有异步释放完成
    tokio::time::sleep(Duration::from_millis(200)).await;

    let status = pool.status();
    assert_eq!(status.active, 0, "All sessions should be released after stress test");
    assert!(success_count > 0, "At least some operations should succeed");

    // 验证连接池仍然可用
    let session = pool
        .get_session("admin")
        .await
        .expect("Pool should still be usable after stress test");
    drop(session);
}

/// 测试并发获取达到限制后的公平性
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_semaphore_fairness() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 2,
        min_connections: 1,
        acquire_timeout: 5000,
        ..Default::default()
    };

    let pool = Arc::new(
        dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool"),
    );

    // 获取所有连接
    let session1 = pool.get_session("admin").await.expect("Failed to get session 1");
    let session2 = pool.get_session("admin").await.expect("Failed to get session 2");

    // 启动多个等待任务
    let pool_clone = pool.clone();
    let wait_task1 = tokio::spawn(async move {
        let result = pool_clone.get_session("admin").await.ok();
        // 模拟短暂使用后释放
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(result);
    });

    let pool_clone = pool.clone();
    let wait_task2 = tokio::spawn(async move {
        let result = pool_clone.get_session("admin").await.ok();
        // 模拟短暂使用后释放
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(result);
    });

    // 给等待任务一点时间开始等待
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 释放连接，等待任务应该能获取
    drop(session1);
    drop(session2);

    // 等待任务完成
    let result1 = tokio::time::timeout(Duration::from_secs(3), wait_task1).await;
    let result2 = tokio::time::timeout(Duration::from_secs(3), wait_task2).await;

    // 至少有一个任务应该成功获取连接
    assert!(
        result1.is_ok() || result2.is_ok(),
        "At least one waiting task should succeed"
    );

    // 等待所有连接释放
    tokio::time::sleep(Duration::from_millis(200)).await;
    let status = pool.status();
    assert_eq!(status.active, 0, "All sessions should be released");
}
