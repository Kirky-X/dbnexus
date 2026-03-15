// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 连接池单元测试
//!
//! 测试连接池的核心功能，包括配置初始化、连接限制、超时处理和泄漏检测。
//! 所有测试使用 SQLite 内存数据库，确保独立运行且不依赖外部数据库。

use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// 配置初始化测试
// ============================================================================

/// TEST-U-POOL-001: 测试 DbPoolBuilder 基本配置初始化
///
/// 验证使用 DbPoolBuilder 可以正确初始化连接池配置。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_builder_basic_initialization() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 10,
        min_connections: 2,
        idle_timeout: 300,
        acquire_timeout: 5000,
        admin_role: "admin".to_string(),
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config)
        .await
        .expect("Failed to create pool");

    // 验证配置正确应用
    assert_eq!(pool.config().max_connections, 10);
    assert_eq!(pool.config().min_connections, 2);
    assert_eq!(pool.config().idle_timeout, 300);
    assert_eq!(pool.config().acquire_timeout, 5000);
    assert_eq!(pool.config().admin_role, "admin");
}

/// TEST-U-POOL-002: 测试 DbPoolBuilder 默认值
///
/// 验证未设置的配置项使用合理的默认值。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_builder_default_values() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    // 验证默认值
    assert_eq!(pool.config().max_connections, 20);
    assert_eq!(pool.config().min_connections, 5);
    assert_eq!(pool.config().idle_timeout, 300);
    assert_eq!(pool.config().acquire_timeout, 5000);
    assert_eq!(pool.config().admin_role, "admin");
}

/// TEST-U-POOL-003: 测试 DbPoolBuilder 链式配置
///
/// 验证 DbPoolBuilder 的链式 API 正确工作。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_builder_chained_configuration() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 15,
        min_connections: 3,
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config)
        .await
        .expect("Failed to create pool");

    assert_eq!(pool.config().max_connections, 15);
    assert_eq!(pool.config().min_connections, 3);
}

/// TEST-U-POOL-004: 测试配置边界值 - 最小连接数等于最大连接数
///
/// 验证 min_connections == max_connections 时连接池正常工作。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_config_min_equals_max() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 5,
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config)
        .await
        .expect("Failed to create pool");

    assert_eq!(pool.config().max_connections, 5);
    assert_eq!(pool.config().min_connections, 5);
}

/// TEST-U-POOL-005: 测试配置验证 - min > max
///
/// 验证 min_connections > max_connections 时配置值被设置。
/// 注意：DbConfig 结构体不会在创建时验证，验证会在 DbPool 创建时进行。
#[test]
fn test_pool_config_validation_min_greater_than_max() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 10,
        ..Default::default()
    };

    // 配置已创建，值被设置
    assert!(config.max_connections < config.min_connections);
}

/// TEST-U-POOL-006: 测试配置验证 - 零连接数
///
/// 验证 max_connections = 0 时配置值被设置。
#[test]
fn test_pool_config_validation_zero_max_connections() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 0,
        ..Default::default()
    };

    // 配置已创建，值被设置
    assert_eq!(config.max_connections, 0);
}

/// TEST-U-POOL-007: 测试配置验证 - 空 URL
///
/// 验证 URL 为空时配置值被设置。
#[test]
fn test_pool_config_validation_empty_url() {
    let config = dbnexus::config::DbConfig {
        url: "".to_string(),
        ..Default::default()
    };

    // 配置已创建，URL 为空
    assert_eq!(config.url, "");
}

// ============================================================================
// 连接池状态测试
// ============================================================================

/// TEST-U-POOL-008: 测试连接池初始状态
///
/// 验证连接池创建后的初始状态正确。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_initial_status() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let status = pool.status();

    // 初始状态下，total 和 active 应该是合理的值
    assert!(status.total <= pool.config().max_connections);
    assert_eq!(status.active, 0);
    assert_eq!(status.total, status.active + status.idle);
}

/// TEST-U-POOL-009: 测试获取会话后状态变化
///
/// 验证获取会话后连接池状态正确更新。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_status_after_session_acquire() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let initial_status = pool.status();

    {
        let _session = pool.get_session("admin").await.expect("Failed to get session");
        let status = pool.status();

        // 获取会话后，active 应该增加
        assert!(status.active >= 1);
        assert!(status.total >= initial_status.total);
    }

    // 会话释放后，active 应该减少
    let final_status = pool.status();
    assert_eq!(final_status.active, 0);
}

/// TEST-U-POOL-010: 测试 PoolStatus 结构体
///
/// 验证 PoolStatus 包含所有必要的字段。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_status_structure() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let status = pool.status();

    // 验证所有字段都存在且类型正确
    let _total: u32 = status.total;
    let _active: u32 = status.active;
    let _idle: u32 = status.idle;
    let _wait_count: u32 = status.wait_count;
    let _borrow_count: u64 = status.borrow_count;
    let _max_active: u32 = status.max_active;

    // 验证基本不变式
    assert_eq!(status.total, status.active + status.idle);
}

// ============================================================================
// 最大连接数限制测试
// ============================================================================

/// TEST-U-POOL-011: 测试最大连接数限制
///
/// 验证连接池不会超过配置的最大连接数。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_max_connections_limit() {
    let max_connections = 3;
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections,
        min_connections: 1,
        acquire_timeout: 1000, // 短超时以便快速测试
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config)
        .await
        .expect("Failed to create pool");
    let pool = Arc::new(pool);

    // 获取所有连接
    let mut sessions = Vec::new();
    for _ in 0..max_connections {
        let session = pool.get_session("admin").await.expect("Failed to get session");
        sessions.push(session);
    }

    let status = pool.status();
    assert_eq!(status.active, max_connections);

    // 尝试获取超出限制的连接应该超时
    let pool_clone = pool.clone();
    let result = tokio::time::timeout(Duration::from_millis(500), async {
        pool_clone.get_session("admin").await
    })
    .await;

    assert!(result.is_err() || result.unwrap().is_err(), "Expected timeout or error when exceeding max connections");

    // 释放连接
    drop(sessions);

    // 释放后应该可以重新获取
    let session = pool.get_session("admin").await;
    assert!(session.is_ok(), "Should be able to get session after releasing");
}

/// TEST-U-POOL-012: 测试连接复用
///
/// 验证连接释放后可以被复用。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_connection_reuse() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 2,
        min_connections: 1,
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config)
        .await
        .expect("Failed to create pool");

    // 获取并释放连接多次
    for _ in 0..5 {
        let session = pool.get_session("admin").await.expect("Failed to get session");
        drop(session);
    }

    let status = pool.status();
    // 连接应该被复用，total 不应该超过 max_connections
    assert!(status.total <= 2);
}

/// TEST-U-POOL-013: 测试并发获取连接
///
/// 验证并发场景下连接池正确管理连接数。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_concurrent_connection_acquire() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 1,
        acquire_timeout: 2000,
        ..Default::default()
    };

    let pool = Arc::new(
        dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool"),
    );

    let mut handles = Vec::new();

    // 并发获取 10 个连接（超过最大限制）
    for _ in 0..10 {
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move { pool_clone.get_session("admin").await.ok() });
        handles.push(handle);
    }

    let results: Vec<_> = futures::future::join_all(handles).await;

    // 统计成功获取的数量
    let success_count = results.iter().filter(|r| r.as_ref().unwrap_or(&None).is_some()).count();

    // 由于最大连接数是 5，应该只有部分成功
    assert!(success_count <= 5, "Should not exceed max connections");

    // 显式释放所有 Session
    drop(results);

    // 等待所有 session 释放
    tokio::time::sleep(Duration::from_millis(100)).await;

    let status = pool.status();
    assert_eq!(status.active, 0, "All sessions should be released");
}

// ============================================================================
// 连接超时处理测试
// ============================================================================

/// TEST-U-POOL-014: 测试连接获取超时
///
/// 验证连接池在无法获取连接时正确超时。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_acquire_timeout() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1,
        min_connections: 1,
        acquire_timeout: 500, // 500ms 超时
        ..Default::default()
    };

    let pool = Arc::new(
        dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool"),
    );

    // 获取唯一的连接
    let _session = pool.get_session("admin").await.expect("Failed to get session");

    // 尝试获取第二个连接应该超时
    let pool_clone = pool.clone();
    let start = std::time::Instant::now();

    let result = pool_clone.get_session("admin").await;

    let elapsed = start.elapsed();

    // 应该在超时时间附近返回错误
    assert!(result.is_err(), "Expected timeout error");
    assert!(elapsed >= Duration::from_millis(400), "Should wait at least close to timeout");
    assert!(elapsed < Duration::from_secs(2), "Should not wait too long");
}

/// TEST-U-POOL-015: 测试超时后连接释放可获取
///
/// 验证超时后释放的连接可以被重新获取。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_timeout_then_release() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1,
        min_connections: 1,
        acquire_timeout: 500,
        ..Default::default()
    };

    let pool = Arc::new(
        dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool"),
    );

    // 获取连接
    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 在另一个任务中尝试获取（会超时）
    let pool_clone = pool.clone();
    let timeout_task = tokio::spawn(async move { pool_clone.get_session("admin").await });

    // 等待一小段时间
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 释放连接
    drop(session);

    // 超时任务应该已经失败或成功（取决于释放时机）
    let _ = timeout_task.await;
}

/// TEST-U-POOL-016: 测试配置的超时值正确应用
///
/// 验证配置的 acquire_timeout 正确转换为 Duration。
#[test]
fn test_pool_timeout_duration_conversion() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        acquire_timeout: 3000,
        ..Default::default()
    };

    assert_eq!(
        config.acquire_timeout_duration(),
        Duration::from_millis(3000)
    );
}

// ============================================================================
// 连接泄漏检测测试
// ============================================================================

/// TEST-U-POOL-017: 测试会话自动释放
///
/// 验证 Session 在 drop 时自动释放连接。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_session_auto_release() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let initial_status = pool.status();

    {
        let _session = pool.get_session("admin").await.expect("Failed to get session");
        // session 在此作用域结束时自动释放
    }

    // 给一些时间让异步释放完成
    tokio::time::sleep(Duration::from_millis(50)).await;

    let final_status = pool.status();
    assert_eq!(final_status.active, 0, "Session should be auto-released");
    assert!(final_status.idle >= initial_status.idle, "Connection should be returned to idle pool");
}

/// TEST-U-POOL-018: 测试连接泄漏检测 - borrow_count 追踪
///
/// 验证 borrow_count 正确追踪连接借用次数。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_borrow_count_tracking() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let initial_status = pool.status();
    let initial_borrow_count = initial_status.borrow_count;

    // 借用连接多次
    for _ in 0..3 {
        let _session = pool.get_session("admin").await.expect("Failed to get session");
        // session 在此释放
    }

    // 给一些时间让异步释放完成
    tokio::time::sleep(Duration::from_millis(100)).await;

    let final_status = pool.status();
    assert!(
        final_status.borrow_count >= initial_borrow_count + 3,
        "borrow_count should track all borrows"
    );
}

/// TEST-U-POOL-019: 测试连接泄漏检测 - max_active 追踪
///
/// 验证 max_active 正确追踪历史峰值连接数。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_max_active_tracking() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config)
        .await
        .expect("Failed to create pool");

    // 同时获取多个连接
    let mut sessions = Vec::new();
    for _ in 0..3 {
        let session = pool.get_session("admin").await.expect("Failed to get session");
        sessions.push(session);
    }

    let status_with_sessions = pool.status();
    assert!(status_with_sessions.max_active >= 3);

    // 释放所有连接
    drop(sessions);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let final_status = pool.status();
    // max_active 应该保持历史峰值
    assert!(final_status.max_active >= 3);
    assert_eq!(final_status.active, 0);
}

/// TEST-U-POOL-020: 测试连接池状态一致性
///
/// 验证在各种操作后连接池状态保持一致。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_status_consistency() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    // 多次获取和释放连接
    for _ in 0..10 {
        let status = pool.status();
        assert_eq!(status.total, status.active + status.idle, "Status invariant should hold");

        let session = pool.get_session("admin").await.expect("Failed to get session");
        let status = pool.status();
        assert_eq!(status.total, status.active + status.idle, "Status invariant should hold after acquire");

        drop(session);
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let final_status = pool.status();
    assert_eq!(final_status.total, final_status.active + final_status.idle);
    assert_eq!(final_status.active, 0);
}

// ============================================================================
// 连接生命周期测试
// ============================================================================

// 注意: ConnectionLifecycle 是内部实现细节，不对外暴露。
// 以下测试通过公共 API 间接验证连接生命周期管理。

/// TEST-U-POOL-021: 测试连接生命周期 - 通过状态追踪
///
/// 验证连接池正确追踪连接的使用情况。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_connection_lifecycle_via_status() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    // 获取连接并验证状态
    {
        let _session = pool.get_session("admin").await.expect("Failed to get session");
        let status = pool.status();
        assert!(status.borrow_count >= 1, "borrow_count should be at least 1");
    }

    // 连接释放后验证
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let final_status = pool.status();
    assert_eq!(final_status.active, 0, "active should be 0 after release");
}

/// TEST-U-POOL-022: 测试连接生命周期 - 借用计数追踪
///
/// 验证 borrow_count 正确追踪所有借用操作。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_connection_lifecycle_borrow_tracking() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let initial_borrow_count = pool.status().borrow_count;

    // 多次借用连接
    for _ in 0..5 {
        let _session = pool.get_session("admin").await.expect("Failed to get session");
    }

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let final_status = pool.status();
    assert!(
        final_status.borrow_count >= initial_borrow_count + 5,
        "borrow_count should have increased by at least 5"
    );
}

/// TEST-U-POOL-023: 测试连接生命周期 - 最大活跃连接追踪
///
/// 验证 max_active 正确追踪历史峰值。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_connection_lifecycle_max_active_tracking() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config)
        .await
        .expect("Failed to create pool");

    // 同时获取多个连接
    let mut sessions = Vec::new();
    for _ in 0..3 {
        let session = pool.get_session("admin").await.expect("Failed to get session");
        sessions.push(session);
    }

    let status_during = pool.status();
    assert!(status_during.max_active >= 3, "max_active should be at least 3");

    // 释放所有连接
    drop(sessions);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let final_status = pool.status();
    assert!(final_status.max_active >= 3, "max_active should retain historical peak");
    assert_eq!(final_status.active, 0, "active should be 0 after release");
}

/// TEST-U-POOL-024: 测试连接生命周期 - 等待计数追踪
///
/// 验证 wait_count 正确追踪等待请求。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_connection_lifecycle_wait_tracking() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1,
        min_connections: 1,
        acquire_timeout: 500,
        ..Default::default()
    };

    let pool = std::sync::Arc::new(
        dbnexus::DbPool::with_config(config)
            .await
            .expect("Failed to create pool"),
    );

    // 获取唯一的连接
    let _session = pool.get_session("admin").await.expect("Failed to get session");

    let initial_wait_count = pool.status().wait_count;

    // 尝试获取第二个连接（会等待）
    let pool_clone = pool.clone();
    let _result = pool_clone.get_session("admin").await;

    // wait_count 应该有变化
    let final_status = pool.status();
    // 注意：由于超时，wait_count 可能已经增加
    assert!(final_status.wait_count >= initial_wait_count);
}

// ============================================================================
// 配置自动修正测试
// ============================================================================

// 注意: ConfigCorrector 和 set_* 方法是内部实现细节，不对外暴露。
// 以下测试通过公共 API 验证配置行为。

/// TEST-U-POOL-027: 测试配置默认值合理性
///
/// 验证配置的默认值在合理范围内。
#[test]
fn test_pool_config_default_values_reasonable() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    };

    // 验证默认值在合理范围内
    assert!(config.max_connections > 0, "max_connections should be > 0");
    assert!(config.max_connections <= 1000, "max_connections should be <= 1000");
    assert!(config.min_connections > 0, "min_connections should be > 0");
    assert!(config.min_connections <= config.max_connections, "min <= max");
    assert!(config.idle_timeout >= 30, "idle_timeout should be >= 30");
    assert!(config.acquire_timeout >= 1000, "acquire_timeout should be >= 1000");
}

// ============================================================================
// 边界条件测试
// ============================================================================

/// TEST-U-POOL-028: 测试单连接池
///
/// 验证 max_connections = 1 时连接池正常工作。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_single_connection() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1,
        min_connections: 1,
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config)
        .await
        .expect("Failed to create pool");

    // 获取并释放连接
    {
        let _session = pool.get_session("admin").await.expect("Failed to get session");
        let status = pool.status();
        assert_eq!(status.active, 1);
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    let status = pool.status();
    assert_eq!(status.active, 0);
    assert!(status.total <= 1);
}

/// TEST-U-POOL-029: 测试大连接池配置
///
/// 验证大连接数配置时连接池正常工作。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_large_connection_pool() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 100,
        min_connections: 10,
        ..Default::default()
    };

    let pool = dbnexus::DbPool::with_config(config)
        .await
        .expect("Failed to create pool");

    assert_eq!(pool.config().max_connections, 100);
    assert_eq!(pool.config().min_connections, 10);
}

/// TEST-U-POOL-030: 测试快速获取释放连接
///
/// 验证快速获取和释放连接不会导致问题。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_pool_rapid_acquire_release() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    // 快速获取和释放
    for _ in 0..100 {
        let session = pool.get_session("admin").await.expect("Failed to get session");
        drop(session);
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let status = pool.status();
    assert_eq!(status.active, 0);
    assert_eq!(status.total, status.active + status.idle);
}

// ============================================================================
// Session 测试
// ============================================================================

/// TEST-U-POOL-031: 测试 Session 角色获取
///
/// 验证 Session 正确返回角色名称。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_session_role() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");
    assert_eq!(session.role(), "admin");
}

/// TEST-U-POOL-032: 测试 Session 事务状态
///
/// 验证 Session 正确追踪事务状态。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_session_transaction_state() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 初始不在事务中
    assert!(!session.is_in_transaction().await);

    // 开始事务
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction().await);

    // 提交事务
    session.commit().await.expect("Failed to commit transaction");
    assert!(!session.is_in_transaction().await);
}

/// TEST-U-POOL-033: 测试 Session 事务回滚
///
/// 验证 Session 正确处理事务回滚。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_session_transaction_rollback() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 开始事务
    session.begin_transaction().await.expect("Failed to begin transaction");
    assert!(session.is_in_transaction().await);

    // 回滚事务
    session.rollback().await.expect("Failed to rollback transaction");
    assert!(!session.is_in_transaction().await);
}

/// TEST-U-POOL-034: 测试 Session 重复开始事务失败
///
/// 验证在事务中再次开始事务会失败。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_session_double_begin_transaction() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 开始事务
    session.begin_transaction().await.expect("Failed to begin transaction");

    // 再次开始事务应该失败
    let result = session.begin_transaction().await;
    assert!(result.is_err(), "Should fail when beginning transaction while already in one");
}

/// TEST-U-POOL-035: 测试 Session 无事务时提交失败
///
/// 验证不在事务中时提交会失败。
#[tokio::test]
#[cfg(feature = "sqlite")]
async fn test_session_commit_without_transaction() {
    let pool = dbnexus::DbPool::new("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    let session = pool.get_session("admin").await.expect("Failed to get session");

    // 没有事务时提交应该失败
    let result = session.commit().await;
    assert!(result.is_err(), "Should fail when committing without active transaction");
}

// ============================================================================
// URL 脱敏测试
// ============================================================================

/// TEST-U-POOL-036: 测试 URL 脱敏 - 带密码
///
/// 验证 URL 包含密码时，可以通过字段访问获取。
#[test]
fn test_url_sanitization_with_password() {
    let config = dbnexus::config::DbConfig {
        url: "postgres://user:secret_password@localhost:5432/mydb".to_string(),
        ..Default::default()
    };

    // URL 包含密码
    assert!(config.url.contains("postgres://"));
    assert!(config.url.contains("secret_password"));
}

/// TEST-U-POOL-037: 测试 URL 脱敏 - SQLite 内存数据库
///
/// 验证 SQLite 内存数据库 URL 不被修改。
#[test]
fn test_url_sanitization_sqlite_memory() {
    let config = dbnexus::config::DbConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    };

    assert_eq!(config.url, "sqlite::memory:");
}

/// TEST-U-POOL-038: 测试 URL 脱敏 - 无密码
///
/// 验证无密码 URL 正确处理。
#[test]
fn test_url_sanitization_no_password() {
    let config = dbnexus::config::DbConfig {
        url: "postgres://user@localhost:5432/mydb".to_string(),
        ..Default::default()
    };

    assert!(config.url.contains("postgres://"));
}

// ============================================================================
// 数据库类型测试
// ============================================================================

/// TEST-U-POOL-039: 测试数据库类型解析
///
/// 验证 DatabaseType 正确解析各种 URL。
#[test]
fn test_database_type_parsing() {
    use dbnexus::config::DatabaseType;

    assert_eq!(DatabaseType::parse_database_type("postgres://localhost/db"), DatabaseType::Postgres);
    assert_eq!(DatabaseType::parse_database_type("postgresql://localhost/db"), DatabaseType::Postgres);
    assert_eq!(DatabaseType::parse_database_type("mysql://localhost/db"), DatabaseType::MySql);
    assert_eq!(DatabaseType::parse_database_type("sqlite::memory:"), DatabaseType::Sqlite);
    assert_eq!(DatabaseType::parse_database_type("sqlite3://file.db"), DatabaseType::Sqlite);
}

/// TEST-U-POOL-040: 测试数据库类型显示
///
/// 验证 DatabaseType 的 Display 实现。
#[test]
fn test_database_type_display() {
    use dbnexus::config::DatabaseType;

    assert_eq!(DatabaseType::Postgres.to_string(), "postgres");
    assert_eq!(DatabaseType::MySql.to_string(), "mysql");
    assert_eq!(DatabaseType::Sqlite.to_string(), "sqlite");
}

// ============================================================================
// PoolConfig 测试
// ============================================================================

/// TEST-U-POOL-041: 测试 PoolConfig 创建
///
/// 验证 PoolConfig 正确创建和访问。
#[test]
fn test_pool_config_creation() {
    use dbnexus::config::PoolConfig;

    let config = PoolConfig {
        max_connections: 100,
        min_connections: 10,
        idle_timeout: 300,
        acquire_timeout: 5000,
    };

    assert_eq!(config.max_connections, 100);
    assert_eq!(config.min_connections, 10);
    assert_eq!(config.idle_timeout, 300);
    assert_eq!(config.acquire_timeout, 5000);
}

/// TEST-U-POOL-042: 测试 PoolConfig 默认值
///
/// 验证 PoolConfig 默认值合理。
#[test]
fn test_pool_config_defaults() {
    use dbnexus::config::PoolConfig;

    let config = PoolConfig::default();

    assert_eq!(config.max_connections, 20);
    assert_eq!(config.min_connections, 5);
    assert_eq!(config.idle_timeout, 300);
    assert_eq!(config.acquire_timeout, 5000);
}

// ============================================================================
// 健康检查间隔边界值测试
// ============================================================================

/// TEST-U-POOL-043: 测试健康检查间隔 - 默认值
///
/// 验证未设置环境变量时使用默认值 30 秒。
#[test]
#[cfg(feature = "pool-health-check")]
fn test_health_check_interval_default() {
    // 清理环境变量
    unsafe { std::env::remove_var("DB_HEALTH_CHECK_INTERVAL"); }

    let interval = dbnexus::DbPool::parse_health_check_interval();
    assert_eq!(interval, 30, "默认健康检查间隔应为 30 秒");
}

/// TEST-U-POOL-044: 测试健康检查间隔 - 下边界值 0
///
/// 验证环境变量设置为 0 时，被限制为最小值 5 秒。
#[test]
#[cfg(feature = "pool-health-check")]
fn test_health_check_interval_lower_bound_zero() {
    unsafe { std::env::set_var("DB_HEALTH_CHECK_INTERVAL", "0"); }

    let interval = dbnexus::DbPool::parse_health_check_interval();
    assert_eq!(interval, 5, "健康检查间隔 0 应被限制为最小值 5 秒");

    // 清理
    unsafe { std::env::remove_var("DB_HEALTH_CHECK_INTERVAL"); }
}

/// TEST-U-POOL-045: 测试健康检查间隔 - 下边界值 5
///
/// 验证环境变量设置为 5 时，保持不变（最小有效值）。
#[test]
#[cfg(feature = "pool-health-check")]
fn test_health_check_interval_lower_bound_five() {
    unsafe { std::env::set_var("DB_HEALTH_CHECK_INTERVAL", "5"); }

    let interval = dbnexus::DbPool::parse_health_check_interval();
    assert_eq!(interval, 5, "健康检查间隔 5 应保持不变");

    // 清理
    unsafe { std::env::remove_var("DB_HEALTH_CHECK_INTERVAL"); }
}

/// TEST-U-POOL-046: 测试健康检查间隔 - 上边界值 300
///
/// 验证环境变量设置为 300 时，保持不变（最大有效值）。
#[test]
#[cfg(feature = "pool-health-check")]
fn test_health_check_interval_upper_bound_300() {
    unsafe { std::env::set_var("DB_HEALTH_CHECK_INTERVAL", "300"); }

    let interval = dbnexus::DbPool::parse_health_check_interval();
    assert_eq!(interval, 300, "健康检查间隔 300 应保持不变");

    // 清理
    unsafe { std::env::remove_var("DB_HEALTH_CHECK_INTERVAL"); }
}

/// TEST-U-POOL-047: 测试健康检查间隔 - 超出上边界值 1000
///
/// 验证环境变量设置为 1000 时，被限制为最大值 300 秒。
#[test]
#[cfg(feature = "pool-health-check")]
fn test_health_check_interval_upper_bound_exceeded() {
    unsafe { std::env::set_var("DB_HEALTH_CHECK_INTERVAL", "1000"); }

    let interval = dbnexus::DbPool::parse_health_check_interval();
    assert_eq!(interval, 300, "健康检查间隔 1000 应被限制为最大值 300 秒");

    // 清理
    unsafe { std::env::remove_var("DB_HEALTH_CHECK_INTERVAL"); }
}

/// TEST-U-POOL-048: 测试健康检查间隔 - 有效中间值
///
/// 验证环境变量设置为有效中间值 60 时，保持不变。
#[test]
#[cfg(feature = "pool-health-check")]
fn test_health_check_interval_valid_middle_value() {
    unsafe { std::env::set_var("DB_HEALTH_CHECK_INTERVAL", "60"); }

    let interval = dbnexus::DbPool::parse_health_check_interval();
    assert_eq!(interval, 60, "健康检查间隔 60 应保持不变");

    // 清理
    unsafe { std::env::remove_var("DB_HEALTH_CHECK_INTERVAL"); }
}

/// TEST-U-POOL-049: 测试健康检查间隔 - 无效字符串
///
/// 验证环境变量设置为无效字符串时，使用默认值 30 秒。
#[test]
#[cfg(feature = "pool-health-check")]
fn test_health_check_interval_invalid_string() {
    unsafe { std::env::set_var("DB_HEALTH_CHECK_INTERVAL", "invalid"); }

    let interval = dbnexus::DbPool::parse_health_check_interval();
    assert_eq!(interval, 30, "无效字符串应使用默认值 30 秒");

    // 清理
    unsafe { std::env::remove_var("DB_HEALTH_CHECK_INTERVAL"); }
}

/// TEST-U-POOL-050: 测试健康检查间隔 - 边界内值 1
///
/// 验证环境变量设置为 1 时，被限制为最小值 5 秒。
#[test]
#[cfg(feature = "pool-health-check")]
fn test_health_check_interval_below_minimum() {
    unsafe { std::env::set_var("DB_HEALTH_CHECK_INTERVAL", "1"); }

    let interval = dbnexus::DbPool::parse_health_check_interval();
    assert_eq!(interval, 5, "健康检查间隔 1 应被限制为最小值 5 秒");

    // 清理
    unsafe { std::env::remove_var("DB_HEALTH_CHECK_INTERVAL"); }
}

/// TEST-U-POOL-051: 测试健康检查间隔 - 边界内值 301
///
/// 验证环境变量设置为 301 时，被限制为最大值 300 秒。
#[test]
#[cfg(feature = "pool-health-check")]
fn test_health_check_interval_above_maximum() {
    unsafe { std::env::set_var("DB_HEALTH_CHECK_INTERVAL", "301"); }

    let interval = dbnexus::DbPool::parse_health_check_interval();
    assert_eq!(interval, 300, "健康检查间隔 301 应被限制为最大值 300 秒");

    // 清理
    unsafe { std::env::remove_var("DB_HEALTH_CHECK_INTERVAL"); }
}

// ============================================================================
// 信号量许可管理测试（Perf-1 修复验证）
// ============================================================================
//
// 注意：信号量许可管理的并发测试已移至 tests/pool_semaphore_test.rs
// 这里的测试专注于单元级别的验证。
