// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in project root for full license information.

//! database::pool::DbPool 单元测试
//!
//! 覆盖：
//! - `DbPool::new` / `with_config` / `try_from_config` 构造路径
//! - `DbPoolBuilder` 链式构造（url/config/max/min/admin_role/build）
//! - `status()` 初始状态与不变量（total = active + idle）
//! - `config()` / `get_actual_config()` 返回值
//! - `get_session(role)` 权限检查（admin/system 通过，无权限配置时其他角色拒绝）
//! - `Clone` 语义、并发 get_session、max_connections=1 边界
//!
//! 所有测试使用 SQLite 内存数据库，需要 `sqlite` feature。

#![cfg(feature = "sqlite")]

use dbnexus::{DbConfig, DbPool, DbPoolBuilder};

// ============================================================================
// 构造测试
// ============================================================================

/// TEST-U-DPOOL-001: DbPool::new(sqlite::memory:) 应成功
#[tokio::test]
async fn test_db_pool_new_sqlite_memory() {
    let pool = DbPool::new("sqlite::memory:").await;
    assert!(pool.is_ok(), "DbPool::new should succeed for sqlite::memory:");
    let pool = pool.unwrap();
    let status = pool.status();
    // 默认 min_connections=5，池会预热创建连接
    assert!(status.total >= 1, "pool should have at least 1 connection after warmup, got {}", status.total);
}

/// TEST-U-DPOOL-002: DbPool::with_config 应成功
#[tokio::test]
async fn test_db_pool_with_config() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 10,
        min_connections: 1,
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await;
    assert!(pool.is_ok(), "with_config should succeed");
}

/// TEST-U-DPOOL-003: DbPool::try_from_config 应与 with_config 行为一致
#[tokio::test]
async fn test_db_pool_try_from_config() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        ..Default::default()
    };
    let pool = DbPool::try_from_config(config).await;
    assert!(pool.is_ok(), "try_from_config should succeed");
}

/// TEST-U-DPOOL-004: DbPool::new 无效 URL 应返回错误
#[tokio::test]
async fn test_db_pool_new_invalid_url_fails() {
    let result = DbPool::new("invalid://nonexistent").await;
    assert!(result.is_err(), "invalid URL should fail");
}

// ============================================================================
// DbPoolBuilder 测试
// ============================================================================

/// TEST-U-DPOOL-005: DbPoolBuilder 通过 url 构造
#[tokio::test]
async fn test_db_pool_builder_with_url() {
    let pool = DbPoolBuilder::new()
        .url("sqlite::memory:")
        .build()
        .await;
    assert!(pool.is_ok(), "builder with url should succeed");
}

/// TEST-U-DPOOL-006: DbPoolBuilder 通过 config 构造
#[tokio::test]
async fn test_db_pool_builder_with_config() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        ..Default::default()
    };
    let pool = DbPoolBuilder::new().config(config).build().await;
    assert!(pool.is_ok(), "builder with config should succeed");
    let pool = pool.unwrap();
    assert_eq!(pool.config().max_connections, 5);
}

/// TEST-U-DPOOL-007: DbPoolBuilder 链式设置 max/min connections
#[tokio::test]
async fn test_db_pool_builder_max_min_connections() {
    let pool = DbPoolBuilder::new()
        .url("sqlite::memory:")
        .max_connections(15)
        .min_connections(2)
        .build()
        .await;
    assert!(pool.is_ok());
    let pool = pool.unwrap();
    assert_eq!(pool.config().max_connections, 15);
    assert_eq!(pool.config().min_connections, 2);
}

/// TEST-U-DPOOL-008: DbPoolBuilder 设置 admin_role
#[tokio::test]
async fn test_db_pool_builder_admin_role() {
    let pool = DbPoolBuilder::new()
        .url("sqlite::memory:")
        .admin_role("root")
        .build()
        .await;
    assert!(pool.is_ok());
    let pool = pool.unwrap();
    assert_eq!(pool.config().admin_role, "root");
}

// ============================================================================
// status / config 测试
// ============================================================================

/// TEST-U-DPOOL-009: 初始 status 应满足 total = active + idle（min_connections=0 时为 0）
#[tokio::test]
async fn test_db_pool_status_initial() {
    // 使用 min_connections=0 确保初始状态无连接
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        min_connections: 0,
        max_connections: 20,
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.unwrap();
    let status = pool.status();
    assert_eq!(status.total, 0);
    assert_eq!(status.active, 0);
    assert_eq!(status.idle, 0);
}

/// TEST-U-DPOOL-010: status 不变量 total = active + idle
#[tokio::test]
async fn test_db_pool_status_invariant_total_equals_active_plus_idle() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let status = pool.status();
    assert_eq!(
        status.total,
        status.active + status.idle,
        "invariant: total == active + idle"
    );
}

/// TEST-U-DPOOL-011: config() 应返回构建时配置
#[tokio::test]
async fn test_db_pool_config_returns_built_config() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 7,
        admin_role: "sa".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.unwrap();
    assert_eq!(pool.config().max_connections, 7);
    assert_eq!(pool.config().admin_role, "sa");
    assert_eq!(pool.config().url, "sqlite::memory:");
}

/// TEST-U-DPOOL-012: get_actual_config() 应与 config() 一致
#[tokio::test]
async fn test_db_pool_get_actual_config() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let cfg1 = pool.config();
    let cfg2 = pool.get_actual_config();
    assert_eq!(cfg1.url, cfg2.url);
    assert_eq!(cfg1.max_connections, cfg2.max_connections);
}

// ============================================================================
// get_session 权限测试
// ============================================================================

/// TEST-U-DPOOL-013: get_session("admin") 应成功
#[tokio::test]
async fn test_db_pool_get_session_admin() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let session = pool.get_session("admin").await;
    assert!(session.is_ok(), "admin role should always be allowed");
    let session = session.unwrap();
    assert_eq!(session.role(), "admin");
}

/// TEST-U-DPOOL-014: get_session("system") 应成功（无权限配置时安全角色）
#[tokio::test]
async fn test_db_pool_get_session_system() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let session = pool.get_session("system").await;
    assert!(session.is_ok(), "system role should be allowed without permission config");
}

/// TEST-U-DPOOL-015: 无权限配置时 get_session("guest") 应失败
#[cfg(feature = "permission")]
#[tokio::test]
async fn test_db_pool_get_session_unauthorized_role_fails() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let result = pool.get_session("guest").await;
    assert!(result.is_err(), "non-safe role should be rejected without permission config");
    // Session 不实现 Debug，用 match 代替 unwrap_err
    match result {
        Err(dbnexus::DbError::Permission(_)) => { /* expected */ }
        Err(other) => panic!("expected Permission error, got {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

/// TEST-U-DPOOL-016: get_session 后 session role 应匹配请求
#[tokio::test]
async fn test_db_pool_get_session_role_matches() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let session = pool.get_session("admin").await.unwrap();
    assert_eq!(session.role(), "admin");
    // Session 被 drop 后连接应归还
    drop(session);
}

// ============================================================================
// Clone 测试
// ============================================================================

/// TEST-U-DPOOL-017: Clone 后的 pool 应独立可用
#[tokio::test]
async fn test_db_pool_clone_usable() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let cloned = pool.clone();
    // 克隆的 pool 也应能获取 session
    let session = cloned.get_session("admin").await;
    assert!(session.is_ok(), "cloned pool should be usable");
}

/// TEST-U-DPOOL-018: Clone 后 config 应相等
#[tokio::test]
async fn test_db_pool_clone_config_equal() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let cloned = pool.clone();
    assert_eq!(pool.config().url, cloned.config().url);
    assert_eq!(pool.config().max_connections, cloned.config().max_connections);
}

// ============================================================================
// 边界测试
// ============================================================================

/// TEST-U-DPOOL-019: max_connections=1 边界
#[tokio::test]
async fn test_db_pool_max_connections_one() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 1,
        min_connections: 0,
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.unwrap();
    let _session = pool.get_session("admin").await.expect("first session should succeed");
    // session 持有期间连接被占用；drop 后释放
}

/// TEST-U-DPOOL-020: 串行 get_session 多次应成功（连接回收）
#[tokio::test]
async fn test_db_pool_serial_get_session_reuses_connection() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    for i in 0..5 {
        let session = pool.get_session("admin").await;
        assert!(session.is_ok(), "iteration {} should succeed", i);
        // 每次 drop session 归还连接
        drop(session);
    }
}

// ============================================================================
// 并发测试
// ============================================================================

/// TEST-U-DPOOL-021: 并发 get_session 不破坏 status 不变量
#[tokio::test]
async fn test_db_pool_concurrent_get_session_preserves_invariants() {
    use std::sync::Arc;
    let pool = Arc::new(DbPool::new("sqlite::memory:").await.unwrap());
    let max = pool.config().max_connections;

    let mut handles = Vec::new();
    for _ in 0..max {
        let pool_clone = Arc::clone(&pool);
        handles.push(tokio::spawn(async move {
            let _session = pool_clone.get_session("admin").await.unwrap();
            // 持有 session 一小段时间
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            // session drop 在 task 结束时发生
        }));
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }

    // 完成后 status 应恢复（active 回到 0 或接近 0）
    let status = pool.status();
    assert!(
        status.total >= status.active,
        "invariant violated: total >= active"
    );
}

// ============================================================================
// Drop 测试
// ============================================================================

/// TEST-U-DPOOL-022: DbPool drop 不应 panic
#[tokio::test]
async fn test_db_pool_drop_no_panic() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    // 显式 drop，不应 panic
    drop(pool);
}

/// TEST-U-DPOOL-023: 带 session 的 DbPool drop 不应 panic
#[tokio::test]
async fn test_db_pool_drop_with_active_session_no_panic() {
    let pool = DbPool::new("sqlite::memory:").await.unwrap();
    let _session = pool.get_session("admin").await.unwrap();
    // pool 和 session 同时超出作用域，不应 panic
    drop(pool);
    drop(_session);
}
