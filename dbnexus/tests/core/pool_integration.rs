// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池集成测试
//!
//! 测试连接池的创建、管理、连接健康检查等功能

use dbnexus::DbPool;
use std::time::Duration;

#[path = "../common/mod.rs"]
mod common;

/// TEST-I-001: 连接健康检查测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_connection_health_check() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config)
        .await
        .expect("TEST-I-001: Failed to create test pool for connection health check");

    // 获取一个会话
    let mut session = pool
        .get_session("admin")
        .await
        .expect("TEST-I-001: Failed to get admin session for health check");

    // 获取底层连接进行健康检查
    let conn = session
        .connection()
        .expect("TEST-I-001: Failed to get underlying connection from session");

    // 执行健康检查
    let is_healthy = pool.check_connection_health(conn).await;
    assert!(is_healthy, "Connection should be healthy");
}

/// TEST-I-002: 清理无效连接测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_clean_invalid_connections() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config)
        .await
        .expect("TEST-I-002: Failed to create test pool for cleaning invalid connections");

    // 初始状态应该没有无效连接
    let removed = pool.clean_invalid_connections().await;
    assert_eq!(removed, 0, "No invalid connections should be removed initially");
}

/// TEST-I-003: 验证并重建连接测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_validate_and_recreate_connections() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config.clone())
        .await
        .expect("TEST-I-003: Failed to create test pool for validating and recreating connections");

    // 初始验证应该不会重新创建任何连接（所有连接都是有效的）
    let _recreated = pool.validate_and_recreate_connections().await;

    // 先获取一个连接以触发连接池初始化
    let _session = pool
        .get_session("admin")
        .await
        .expect("TEST-I-003: Failed to get session to initialize pool");

    let status = pool.status();
    assert!(status.total >= config.min_connections as u32);
}

/// TEST-I-004: 连接池状态测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_status_after_operations() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config.clone())
        .await
        .expect("TEST-I-004: Failed to create test pool for status operations test");

    // 先获取一个连接以触发连接池初始化
    let _session = pool
        .get_session("admin")
        .await
        .expect("TEST-I-004: Failed to get session to initialize pool");

    let initial_status = pool.status();
    assert!(initial_status.total >= config.min_connections as u32);

    // 获取多个会话（使用安全角色 admin 和 system）
    let mut sessions = Vec::new();
    for i in 0..3 {
        // 交替使用 admin 和 system 角色
        let role = if i % 2 == 0 { "admin" } else { "system" };
        let session = pool
            .get_session(role)
            .await
            .expect(&format!("TEST-I-004: Failed to get session for {}", role));
        sessions.push(session);
    }

    let status_after_acquire = pool.status();
    // 初始化连接 + 3个会话 = 4 个活动连接
    assert_eq!(
        status_after_acquire.active, 4,
        "Should have 4 active connections (1 init + 3 sessions)"
    );

    // 释放所有会话（通过离开作用域）
    drop(sessions);

    // 等待连接返回到池中
    tokio::time::sleep(Duration::from_millis(100)).await;

    let final_status = pool.status();
    assert!(
        final_status.total >= config.min_connections as u32,
        "Should have at least {} total connections after release",
        config.min_connections
    );
    // 初始化连接仍在使用中，所以 active = 1
    assert_eq!(
        final_status.active, 1,
        "Should have 1 active connection (init session still held)"
    );
}

/// TEST-I-005: 连续健康检查测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_sequential_health_checks() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config)
        .await
        .expect("TEST-I-005: Failed to create test pool for sequential health checks");

    // 连续执行多次健康检查
    for i in 0..5 {
        let mut session = pool.get_session("admin").await.expect(&format!(
            "TEST-I-005: Failed to get admin session for health check {}",
            i
        ));
        let conn = session
            .connection()
            .expect(&format!("TEST-I-005: Failed to get connection for health check {}", i));
        let is_healthy = pool.check_connection_health(conn).await;
        assert!(is_healthy, "Connection {} should be healthy", i);
    }
}

/// TEST-I-006: 健康检查超时处理测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_health_check_timeout_handling() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config)
        .await
        .expect("TEST-I-006: Failed to create test pool for health check timeout handling");

    // 获取一个有效的连接
    let mut session = pool
        .get_session("admin")
        .await
        .expect("TEST-I-006: Failed to get admin session for timeout handling test");
    let conn = session
        .connection()
        .expect("TEST-I-006: Failed to get connection for timeout handling test");

    // 健康检查应该在合理时间内完成（5秒超时）
    let start = std::time::Instant::now();
    let is_healthy = pool.check_connection_health(conn).await;
    let elapsed = start.elapsed();

    assert!(is_healthy, "Connection should be healthy");
    assert!(
        elapsed < Duration::from_secs(5),
        "Health check should complete within 5 seconds"
    );
}

/// TEST-I-007: 大量连接后的健康检查测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_health_check_after_heavy_usage() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config)
        .await
        .expect("TEST-I-007: Failed to create test pool for heavy usage health check");

    // 模拟使用（使用较小数量，避免超出连接池限制）
    let mut sessions = Vec::new();
    let num_sessions = std::cmp::min(5, pool.config().max_connections as usize);

    for i in 0..num_sessions {
        match pool.get_session(&format!("test_role_{}", i)).await {
            Ok(session) => sessions.push(session),
            Err(_) => {
                // 如果获取失败，跳过这个会话
                // 继续处理已获取的会话
            }
        }
    }

    // 逐个释放并检查健康
    for (i, mut session) in sessions.into_iter().enumerate() {
        let conn = session.connection().expect(&format!(
            "TEST-I-007: Failed to get connection for session {} after heavy usage",
            i
        ));
        let is_healthy = pool.check_connection_health(conn).await;
        assert!(is_healthy, "Connection {} should be healthy after heavy usage", i);
    }
}

/// TEST-I-008: 并发健康检查测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_concurrent_health_checks() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config)
        .await
        .expect("TEST-I-008: Failed to create test pool for concurrent health checks");

    let pool = std::sync::Arc::new(pool);
    let mut handles = Vec::new();

    // 并发执行多个健康检查
    for _ in 0..5 {
        let pool = pool.clone();
        let handle = tokio::spawn(async move {
            let mut session = pool
                .get_session("admin")
                .await
                .expect("TEST-I-008: Failed to get admin session for concurrent health check");
            let conn = session
                .connection()
                .expect("TEST-I-008: Failed to get connection for concurrent health check");
            pool.check_connection_health(conn).await
        });
        handles.push(handle);
    }

    // 等待所有健康检查完成
    let results = futures::future::join_all(handles).await;

    // 所有健康检查都应该成功
    for (i, result) in results.into_iter().enumerate() {
        assert!(
            result.expect(&format!("TEST-I-008: Health check task {} panicked", i)),
            "Health check {} should succeed",
            i
        );
    }
}

/// TEST-I-009: 连接池配置边界测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_config_boundaries() {
    // 测试最小配置
    let config = common::get_test_config();
    let pool = DbPool::with_config(config)
        .await
        .expect("TEST-I-009: Failed to create test pool for config boundaries test");

    // 先获取一个连接以触发连接池初始化
    let _session = pool
        .get_session("admin")
        .await
        .expect("TEST-I-009: Failed to get session to initialize pool");

    let status = pool.status();

    assert!(status.total >= 1, "Pool should have at least 1 connection");
    assert!(status.total >= status.active, "Total should be >= active");
    assert!(status.total >= status.idle, "Total should be >= idle");
}

/// TEST-I-010: 连接获取超时测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_connection_acquire_with_small_pool() {
    // 创建一个小连接池
    use dbnexus::config::DbConfig;

    let db_config = common::get_test_config();
    let config = DbConfig {
        url: db_config.url,
        max_connections: 2,
        min_connections: 1,
        idle_timeout: 300,
        acquire_timeout: 1000, // 1000毫秒超时
        permissions_path: None,
        migrations_dir: None,
        auto_migrate: false,
        migration_timeout: 60,
        admin_role: "admin".to_string(),
        warmup_timeout: 30,
        warmup_retries: 3,
    };

    let pool = DbPool::with_config(config)
        .await
        .expect("TEST-I-010: Failed to create test pool with small size for acquire timeout test");

    // 获取两个连接（达到最大限制）- 使用安全角色
    let _session1 = pool
        .get_session("admin")
        .await
        .expect("TEST-I-010: Failed to get first session from small pool");
    let _session2 = pool
        .get_session("system")
        .await
        .expect("TEST-I-010: Failed to get second session from small pool");

    // 第三个获取可能会超时或等待（取决于实现）
    // 这个测试验证池能够处理连接耗尽的情况
    let result = pool.get_session("admin").await;

    // 结果可能是成功（如果实现了等待队列）或超时
    // 我们不强制要求超时行为，因为这取决于具体实现
    assert!(
        result.is_ok() || result.is_err(),
        "Pool should handle connection exhaustion gracefully"
    );
}

/// TEST-I-011: 健康检查与数据库类型兼容性测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_health_check_compatibility() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config)
        .await
        .expect("TEST-I-011: Failed to create test pool for database compatibility test");

    let mut session = pool
        .get_session("admin")
        .await
        .expect("TEST-I-011: Failed to get admin session for compatibility test");
    let conn = session
        .connection()
        .expect("TEST-I-011: Failed to get connection for compatibility test");

    // 无论数据库类型如何，健康检查都应该返回有效结果
    let is_healthy = pool.check_connection_health(conn).await;

    // 在正常情况下应该返回 true
    assert!(is_healthy, "Connection should be healthy for any database type");

    // 验证池状态正常
    let status = pool.status();
    assert!(status.total > 0, "Pool should have connections");
}

/// TEST-I-012: 连接复用与健康检查测试
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_connection_reuse_with_health_checks() {
    let config = common::get_test_config();
    let pool = DbPool::with_config(config)
        .await
        .expect("TEST-I-012: Failed to create test pool for connection reuse test");

    // 多次获取和释放同一角色的会话
    for i in 0..10 {
        {
            let mut session = pool.get_session("admin").await.expect(&format!(
                "TEST-I-012: Failed to get admin session for reuse iteration {}",
                i
            ));
            let conn = session.connection().expect(&format!(
                "TEST-I-012: Failed to get connection for reuse iteration {}",
                i
            ));

            // 执行健康检查
            let is_healthy = pool.check_connection_health(conn).await;
            assert!(is_healthy, "Connection {} should be healthy", i);
        } // session 在此处被释放
        // 短暂等待确保连接返回池中
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // 验证池状态仍然正常
    let status = pool.status();
    assert!(status.total >= 1, "Pool should still have connections");
}

/// TEST-I-015: 数据库 URL 格式验证测试
///
/// 验证配置验证器能正确检测无效的 URL 格式
#[tokio::test]
async fn test_config_url_validation() {
    use dbnexus::config::DbConfig;

    // 有效 URL - 使用正确的 YAML 格式和有效的 URL
    let valid_configs = vec![
        "url: 'sqlite://test.db'\nmax_connections: 10\nmin_connections: 1",
        "url: 'sqlite://:memory:'\nmax_connections: 10\nmin_connections: 1",
        "url: 'sqlite://./test.db'\nmax_connections: 10\nmin_connections: 1",
        "url: 'postgres://localhost:5432/mydb'\nmax_connections: 10\nmin_connections: 1",
        "url: 'postgresql://user:pass@localhost:5432/mydb'\nmax_connections: 10\nmin_connections: 1",
        "url: 'mysql://localhost:3306/mydb'\nmax_connections: 10\nmin_connections: 1",
    ];

    for yaml in valid_configs {
        let result = DbConfig::from_yaml_str(yaml);
        assert!(result.is_ok(), "URL config should be valid: {}", yaml);
        let config = result.unwrap();
        assert!(!config.url.is_empty(), "URL should not be empty");
    }

    // 无效 URL - 这些应该解析失败
    let invalid_urls = [
        "",                 // empty string
        "invalid-url",      // missing protocol separator
        "://localhost",     // missing protocol
        "http://localhost", // unsupported protocol
    ];

    for (idx, url) in invalid_urls.iter().enumerate() {
        // 使用 from_env 风格的测试方式
        let yaml = format!("url: '{}'\nmax_connections: 10\nmin_connections: 1", url);
        let result = DbConfig::from_yaml_str(&yaml);

        // 这些无效 URL 应该导致解析或验证失败
        assert!(result.is_err(), "Test {}: '{}' should be invalid", idx, url);
    }
}
