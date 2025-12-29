//! 并发集成测试
//!
//! 测试连接池和数据库操作的并发场景，包括并发会话获取、并发健康检查、
//! 并发数据库操作、连接池压力测试和竞争条件测试

use dbnexus::DbPool;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
mod common;

/// TEST-CONC-001: 并发会话获取测试
#[tokio::test]
async fn test_concurrent_session_acquisition() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    let num_tasks = 10;
    let mut handles = Vec::new();

    // 并发获取会话
    for i in 0..num_tasks {
        let pool = pool.clone();
        let handle = tokio::spawn(async move { pool.get_session(&format!("user{}", i)).await });
        handles.push(handle);
    }

    // 等待所有任务完成
    let results = futures::future::join_all(handles).await;

    // 验证所有会话都成功获取
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.is_ok(), "Session {} should be acquired successfully", i);
    }

    // 验证连接池状态
    let status = pool.status();
    assert!(status.total >= 1, "Pool should have at least 1 connection");
}

/// TEST-CONC-002: 并发会话释放测试
#[tokio::test]
async fn test_concurrent_session_release() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    let num_sessions = 5;
    let mut sessions = Vec::new();

    // 快速获取多个会话
    for i in 0..num_sessions {
        let session = pool
            .get_session(&format!("user{}", i))
            .await
            .expect("Failed to get session");
        sessions.push(session);
    }

    let pool_clone = pool.clone();
    let release_handle = tokio::spawn(async move {
        // 释放所有会话
        drop(sessions);
        // 等待连接返回池中
        tokio::time::sleep(Duration::from_millis(200)).await;
        pool_clone.status()
    });

    let status = release_handle.await.expect("Release task should complete");

    // 验证连接已返回池中
    assert!(
        status.idle >= 1 || status.active < num_sessions as u32,
        "Connections should be released back to pool"
    );
}

/// TEST-CONC-003: 并发健康检查测试
#[tokio::test]
async fn test_concurrent_health_checks() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    let num_checks = 20;
    let mut handles = Vec::new();

    for _i in 0..num_checks {
        let pool = pool.clone();
        let handle = tokio::spawn(async move {
            let mut session = pool.get_session("admin").await.expect("Failed to get session");
            let conn = session.connection().expect("Failed to get connection");
            pool.check_connection_health(conn).await
        });
        handles.push(handle);
    }

    // 等待所有健康检查完成
    let results = futures::future::join_all(handles).await;

    // 验证所有健康检查都成功
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.unwrap_or(false), "Health check {} should succeed", i);
    }
}

/// TEST-CONC-004: 并发数据库操作测试
#[tokio::test]
async fn test_concurrent_database_operations() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    // 创建测试表
    let setup_session = pool.get_session("admin").await.expect("Failed to get session");
    setup_session
        .execute_raw("CREATE TABLE IF NOT EXISTS concurrency_test (id INTEGER PRIMARY KEY, value INTEGER)")
        .await
        .expect("Failed to create test table");
    drop(setup_session);

    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    // 并发执行插入操作
    for i in 0..10 {
        let pool = pool.clone();
        let counter = counter.clone();
        let handle = tokio::spawn(async move {
            let session = pool.get_session("admin").await.expect("Failed to get session");
            let result = session
                .execute_raw(&format!(
                    "INSERT INTO concurrency_test (id, value) VALUES ({}, {})",
                    i, i
                ))
                .await;
            if result.is_ok() {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    // 等待所有插入完成
    futures::future::join_all(handles).await;

    // 验证插入数量
    let insert_count = counter.load(Ordering::SeqCst);
    assert!(
        insert_count >= 1,
        "At least some inserts should succeed, got {}",
        insert_count
    );

    // 清理测试表（使用 IF EXISTS 避免错误）
    let cleanup_session = pool.get_session("admin").await.expect("Failed to get session");
    let _ = cleanup_session
        .execute_raw("DROP TABLE IF EXISTS concurrency_test")
        .await;
}

/// TEST-CONC-005: 连接池压力测试
#[tokio::test]
async fn test_connection_pool_stress() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    let num_cycles = 50;
    let sessions_per_cycle = 3;

    for cycle in 0..num_cycles {
        let mut handles = Vec::new();

        // 每个周期创建多个会话
        for i in 0..sessions_per_cycle {
            let pool = pool.clone();
            let handle = tokio::spawn(async move { pool.get_session(&format!("user{}", i)).await });
            handles.push(handle);
        }

        // 等待所有会话获取完成
        let sessions: Vec<_> = futures::future::join_all(handles)
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        // 验证本周期获取的会话数量
        assert_eq!(
            sessions.len(),
            sessions_per_cycle,
            "Cycle {} should have {} sessions",
            cycle,
            sessions_per_cycle
        );

        // 释放所有会话
        drop(sessions);

        // 短暂等待让连接返回池中
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // 验证连接池仍然正常工作
    let status = pool.status();
    assert!(status.total >= 1, "Pool should still have connections");
}

/// TEST-CONC-006: 竞争条件测试 - 快速获取和释放
#[tokio::test]
async fn test_race_condition_rapid_acquire_release() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    let num_operations = 100;
    let success_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for _ in 0..num_operations {
        let pool = pool.clone();
        let success_count = success_count.clone();
        let handle = tokio::spawn(async move {
            // 快速获取和释放
            let session = pool.get_session("admin").await;
            if session.is_ok() {
                drop(session);
                success_count.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    let count = success_count.load(Ordering::SeqCst);
    assert_eq!(
        count, num_operations,
        "All {} operations should succeed",
        num_operations
    );
}

/// TEST-CONC-007: 并发角色会话测试
#[tokio::test]
async fn test_concurrent_role_sessions() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    let roles = ["admin", "user", "reader", "writer"];
    let iterations = 5;

    for _ in 0..iterations {
        let mut handles = Vec::new();

        for role in roles.iter() {
            let pool = pool.clone();
            let role = role.to_string();
            let handle = tokio::spawn(async move { pool.get_session(&role).await });
            handles.push(handle);
        }

        // 等待所有角色会话获取完成
        let results = futures::future::join_all(handles).await;

        // 验证所有会话都成功获取
        for (i, result) in results.into_iter().enumerate() {
            assert!(result.is_ok(), "Session for role {} should be acquired", roles[i]);
        }
    }
}

/// TEST-CONC-008: 并发事务测试
#[tokio::test]
async fn test_concurrent_transactions() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    // 创建测试表
    let setup_session = pool.get_session("admin").await.expect("Failed to get session");
    setup_session
        .execute_raw("CREATE TABLE IF NOT EXISTS transaction_test (id INTEGER PRIMARY KEY, counter INTEGER)")
        .await
        .expect("Failed to create test table");
    setup_session
        .execute_raw("INSERT INTO transaction_test (id, counter) VALUES (1, 0)")
        .await
        .expect("Failed to insert initial value");
    drop(setup_session);

    let pool_clone = pool.clone();
    let mut handles = Vec::new();

    // 并发执行事务
    for _ in 0..5 {
        let pool = pool_clone.clone();
        let handle = tokio::spawn(async move {
            let mut session = pool.get_session("admin").await.expect("Failed to get session");
            session.begin_transaction().await.expect("Failed to begin transaction");

            // 读取当前值
            let result = session
                .execute_raw("SELECT counter FROM transaction_test LIMIT 1")
                .await;

            // 提交事务
            session.commit().await.expect("Failed to commit");

            result
        });
        handles.push(handle);
    }

    // 等待所有事务完成
    let results = futures::future::join_all(handles).await;

    // 验证所有事务都成功
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.is_ok(), "Transaction {} should succeed", i);
    }

    // 清理（使用 IF EXISTS 避免错误）
    let cleanup_session = pool.get_session("admin").await.expect("Failed to get session");
    let _ = cleanup_session
        .execute_raw("DROP TABLE IF EXISTS transaction_test")
        .await;
}

/// TEST-CONC-009: 连接池容量边界测试
#[tokio::test]
async fn test_pool_capacity_boundary() {
    use dbnexus::config::DbConfig;

    // 创建小容量连接池
    let config = common::get_test_config();
    let pool_config = DbConfig {
        url: config.url,
        max_connections: 3,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 3000,
        permissions_path: None,
    };

    let pool = DbPool::with_config(pool_config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    // 获取所有可用连接
    let mut sessions = Vec::new();
    for i in 0..3 {
        let session = pool
            .get_session(&format!("user{}", i))
            .await
            .expect("Failed to get session");
        sessions.push(session);
    }

    let pool_clone = pool.clone();
    let mut handles = Vec::new();

    // 尝试获取超出容量的连接
    for i in 0..5 {
        let pool = pool_clone.clone();
        let handle = tokio::spawn(async move {
            tokio::time::timeout(Duration::from_millis(500), pool.get_session(&format!("extra{}", i))).await
        });
        handles.push(handle);
    }

    // 等待超时任务
    let results: Vec<Result<Result<_, _>, _>> = futures::future::join_all(handles).await;

    // 部分应该超时
    let timeout_count = results
        .iter()
        .filter(|r| r.is_err() || r.as_ref().unwrap().is_err())
        .count();
    assert!(
        timeout_count > 0,
        "Some connections should timeout when pool is exhausted"
    );

    // 释放连接后应该能获取新连接
    drop(sessions);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let new_session = pool.get_session("new_user").await;
    assert!(new_session.is_ok(), "Should be able to get session after release");
}

/// TEST-CONC-010: 并发清理无效连接测试
#[tokio::test]
async fn test_concurrent_clean_invalid_connections() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    let mut handles = Vec::new();

    // 并发执行清理操作
    for _ in 0..5 {
        let pool = pool.clone();
        let handle = tokio::spawn(async move { pool.clean_invalid_connections().await });
        handles.push(handle);
    }

    // 等待所有清理操作完成
    let results = futures::future::join_all(handles).await;

    // 验证清理操作都成功完成（返回0表示没有无效连接）
    for (i, result) in results.into_iter().enumerate() {
        assert_eq!(result.unwrap(), 0, "Clean {} should find no invalid connections", i);
    }
}

/// TEST-CONC-011: 并发验证和重新创建连接测试
#[tokio::test]
async fn test_concurrent_validate_and_recreate() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    let mut handles = Vec::new();

    // 并发执行验证操作
    for _i in 0..5 {
        let pool = pool.clone();
        let handle = tokio::spawn(async move { pool.validate_and_recreate_connections().await });
        handles.push(handle);
    }

    // 等待所有验证操作完成
    let results = futures::future::join_all(handles).await;

    // 验证所有验证操作都成功完成（unwrap 确保没有错误）
    for result in results.into_iter() {
        let _recreated = result.unwrap();
    }
}

/// TEST-CONC-012: 大规模并发压力测试
#[tokio::test]
async fn test_large_scale_concurrent_stress() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config).await.expect("Failed to create pool");
    let pool = Arc::new(pool);

    let num_tasks = 50;
    let operations_per_task = 10;

    let total_operations = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for task_id in 0..num_tasks {
        let pool = pool.clone();
        let total_operations = total_operations.clone();
        let handle = tokio::spawn(async move {
            for op in 0..operations_per_task {
                let session = pool.get_session(&format!("task{}_op{}", task_id, op)).await;
                if session.is_ok() {
                    total_operations.fetch_add(1, Ordering::SeqCst);
                    drop(session);
                }
                // 短暂休眠避免过度竞争
                tokio::time::sleep(Duration::from_micros(100)).await;
            }
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    futures::future::join_all(handles).await;

    let total = total_operations.load(Ordering::SeqCst);
    assert!(
        total > 0,
        "Should complete at least some operations under concurrent stress"
    );
}
