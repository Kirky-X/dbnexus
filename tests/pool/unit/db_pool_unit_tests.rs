// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
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

use dbnexus::foundation::PoolConfig;
use dbnexus::{DbConfig, DbPool, DbPoolBuilder};

#[path = "../../common/mod.rs"]
mod common;

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
    assert!(
        status.total >= 1,
        "pool should have at least 1 connection after warmup, got {}",
        status.total
    );
}

/// TEST-U-DPOOL-002: DbPool::with_config 应成功
#[tokio::test]
async fn test_db_pool_with_config() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        pool_config: PoolConfig {
            max_connections: 10,
            min_connections: 1,
            ..Default::default()
        },
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
    let pool = DbPoolBuilder::new().url("sqlite::memory:").build().await;
    assert!(pool.is_ok(), "builder with url should succeed");
}

/// TEST-U-DPOOL-006: DbPoolBuilder 通过 config 构造
#[tokio::test]
async fn test_db_pool_builder_with_config() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        pool_config: PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        ..Default::default()
    };
    let pool = DbPoolBuilder::new().config(config).build().await;
    assert!(pool.is_ok(), "builder with config should succeed");
    let pool = pool.unwrap();
    assert_eq!(pool.config().pool_config.max_connections, 5);
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
    assert_eq!(pool.config().pool_config.max_connections, 15);
    assert_eq!(pool.config().pool_config.min_connections, 2);
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
        pool_config: PoolConfig {
            min_connections: 0,
            max_connections: 20,
            ..Default::default()
        },
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
    let pool = common::make_sqlite_memory_pool().await;
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
        pool_config: PoolConfig {
            max_connections: 7,
            ..Default::default()
        },
        admin_role: "sa".to_string(),
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.unwrap();
    assert_eq!(pool.config().pool_config.max_connections, 7);
    assert_eq!(pool.config().admin_role, "sa");
    assert_eq!(pool.config().url, "sqlite::memory:");
}

/// TEST-U-DPOOL-012: get_actual_config() 应与 config() 一致
#[tokio::test]
async fn test_db_pool_get_actual_config() {
    let pool = common::make_sqlite_memory_pool().await;
    let cfg1 = pool.config();
    let cfg2 = pool.get_actual_config();
    assert_eq!(cfg1.url, cfg2.url);
    assert_eq!(cfg1.pool_config.max_connections, cfg2.pool_config.max_connections);
}

// ============================================================================
// get_session 权限测试
// ============================================================================

/// TEST-U-DPOOL-013: get_session("admin") 应成功
#[tokio::test]
async fn test_db_pool_get_session_admin() {
    let pool = common::make_sqlite_memory_pool().await;
    let session = pool.get_session("admin").await;
    assert!(session.is_ok(), "admin role should always be allowed");
    let session = session.unwrap();
    assert_eq!(session.role(), "admin");
}

/// TEST-U-DPOOL-014: get_session("system") 应成功（无权限配置时安全角色）
#[tokio::test]
async fn test_db_pool_get_session_system() {
    let pool = common::make_sqlite_memory_pool().await;
    let session = pool.get_session("system").await;
    assert!(
        session.is_ok(),
        "system role should be allowed without permission config"
    );
}

/// TEST-U-DPOOL-015: 无权限配置时 get_session("guest") 应失败
#[cfg(feature = "permission")]
#[tokio::test]
async fn test_db_pool_get_session_unauthorized_role_fails() {
    let pool = common::make_sqlite_memory_pool().await;
    let result = pool.get_session("guest").await;
    assert!(
        result.is_err(),
        "non-safe role should be rejected without permission config"
    );
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
    let pool = common::make_sqlite_memory_pool().await;
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
    let pool = common::make_sqlite_memory_pool().await;
    let cloned = pool.clone();
    // 克隆的 pool 也应能获取 session
    let session = cloned.get_session("admin").await;
    assert!(session.is_ok(), "cloned pool should be usable");
}

/// TEST-U-DPOOL-018: Clone 后 config 应相等
#[tokio::test]
async fn test_db_pool_clone_config_equal() {
    let pool = common::make_sqlite_memory_pool().await;
    let cloned = pool.clone();
    assert_eq!(pool.config().url, cloned.config().url);
    assert_eq!(
        pool.config().pool_config.max_connections,
        cloned.config().pool_config.max_connections
    );
}

// ============================================================================
// 边界测试
// ============================================================================

/// TEST-U-DPOOL-019: max_connections=1 边界
#[tokio::test]
async fn test_db_pool_max_connections_one() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        pool_config: PoolConfig {
            max_connections: 1,
            min_connections: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.unwrap();
    let _session = pool.get_session("admin").await.expect("first session should succeed");
    // session 持有期间连接被占用；drop 后释放
}

/// TEST-U-DPOOL-020: 串行 get_session 多次应成功（连接回收）
#[tokio::test]
async fn test_db_pool_serial_get_session_reuses_connection() {
    let pool = common::make_sqlite_memory_pool().await;
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
    let pool = Arc::new(common::make_sqlite_memory_pool().await);
    let max = pool.config().pool_config.max_connections;

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
    assert!(status.total >= status.active, "invariant violated: total >= active");
}

// ============================================================================
// Drop 测试
// ============================================================================

/// TEST-U-DPOOL-022: DbPool drop 不应 panic
#[tokio::test]
async fn test_db_pool_drop_no_panic() {
    let pool = common::make_sqlite_memory_pool().await;
    // 显式 drop，不应 panic
    drop(pool);
}

/// TEST-U-DPOOL-023: 带 session 的 DbPool drop 不应 panic
#[tokio::test]
async fn test_db_pool_drop_with_active_session_no_panic() {
    let pool = common::make_sqlite_memory_pool().await;
    let _session = pool.get_session("admin").await.unwrap();
    // pool 和 session 同时超出作用域，不应 panic
    drop(pool);
    drop(_session);
}

// ============================================================================
// try_from 同步构造器测试（permission feature 启用时返回 Err）
// ============================================================================

/// TEST-U-DPOOL-024: try_from 在 permission feature 启用时应返回 Err
#[cfg(feature = "permission")]
#[test]
fn test_db_pool_try_from_with_permission_returns_err() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        pool_config: PoolConfig {
            max_connections: 5,
            ..Default::default()
        },
        ..Default::default()
    };
    let result = DbPool::try_from(&config);
    assert!(
        result.is_err(),
        "try_from should return Err when permission feature is enabled (sync constructor cannot init cache)"
    );
}

// ============================================================================
// DbPoolBuilder 边界测试
// ============================================================================

/// TEST-U-DPOOL-025: DbPoolBuilder::build() 未提供 url 或 config 应返回 Err
#[tokio::test]
async fn test_db_pool_builder_no_url_no_config_fails() {
    let result = DbPoolBuilder::new().build().await;
    assert!(result.is_err(), "build() without url or config should fail");
}

/// TEST-U-DPOOL-026: DbPoolBuilder::admin_role 在 config 已设置时应修改 config
#[tokio::test]
async fn test_db_pool_builder_admin_role_with_config() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        admin_role: "initial".to_string(),
        ..Default::default()
    };
    let pool = DbPoolBuilder::new()
        .config(config)
        .admin_role("root")
        .build()
        .await
        .unwrap();
    assert_eq!(pool.config().admin_role, "root");
}

/// TEST-U-DPOOL-027: DbPoolBuilder::max_connections 在只有 url 时应创建 config
#[tokio::test]
async fn test_db_pool_builder_max_connections_with_url_only() {
    let pool = DbPoolBuilder::new()
        .url("sqlite::memory:")
        .max_connections(13)
        .build()
        .await
        .unwrap();
    assert_eq!(pool.config().pool_config.max_connections, 13);
}

/// TEST-U-DPOOL-028: DbPoolBuilder::min_connections 在只有 url 时应创建 config
#[tokio::test]
async fn test_db_pool_builder_min_connections_with_url_only() {
    let pool = DbPoolBuilder::new()
        .url("sqlite::memory:")
        .min_connections(2)
        .build()
        .await
        .unwrap();
    assert_eq!(pool.config().pool_config.min_connections, 2);
}

// ============================================================================
// parse_health_check_interval 单元测试
// ============================================================================

/// TEST-U-DPOOL-030: parse_health_check_interval 边界值
#[cfg(feature = "pool-health-check")]
#[test]
fn test_parse_health_check_interval_boundaries() {
    use dbnexus::DbPool;

    // 空字符串返回默认 30
    assert_eq!(DbPool::parse_health_check_interval(""), 30);
    // 有效值
    assert_eq!(DbPool::parse_health_check_interval("60"), 60);
    assert_eq!(DbPool::parse_health_check_interval("5"), 5);
    assert_eq!(DbPool::parse_health_check_interval("300"), 300);
    // 小于下限 → 5
    assert_eq!(DbPool::parse_health_check_interval("1"), 5);
    assert_eq!(DbPool::parse_health_check_interval("4"), 5);
    // 大于上限 → 300
    assert_eq!(DbPool::parse_health_check_interval("301"), 300);
    assert_eq!(DbPool::parse_health_check_interval("1000"), 300);
    // 无效字符串 → 默认 30
    assert_eq!(DbPool::parse_health_check_interval("abc"), 30);
    assert_eq!(DbPool::parse_health_check_interval("12.5"), 30);
    assert_eq!(DbPool::parse_health_check_interval("-5"), 30);
}

// ============================================================================
// check_connection_health 测试
// ============================================================================

/// TEST-U-DPOOL-031: check_connection_health 对健康的 SeaORM 连接应返回 true
#[tokio::test]
async fn test_check_connection_health_healthy_seaorm() {
    use dbnexus::DbConnection;
    let pool = common::make_sqlite_memory_pool().await;
    let sea_conn = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    let conn = DbConnection::SeaOrm(sea_conn);
    let is_healthy = pool.check_connection_health(&conn).await;
    assert!(is_healthy, "healthy SeaORM connection should return true");
}

// ============================================================================
// 健康检查与清理测试（pool-health-check feature）
// ============================================================================

/// TEST-U-DPOOL-032: clean_invalid_connections 对健康池应返回 0
#[cfg(feature = "pool-health-check")]
#[tokio::test]
async fn test_clean_invalid_connections_healthy_pool() {
    let pool = common::make_sqlite_memory_pool().await;
    let removed = pool.clean_invalid_connections().await;
    assert_eq!(removed, 0, "no invalid connections should be removed from healthy pool");
}

/// TEST-U-DPOOL-033: validate_and_recreate_connections 对健康池应返回 0
#[cfg(feature = "pool-health-check")]
#[tokio::test]
async fn test_validate_and_recreate_connections_healthy_pool() {
    let pool = common::make_sqlite_memory_pool().await;
    let recreated = pool.validate_and_recreate_connections().await;
    assert!(recreated.is_ok(), "should succeed on healthy pool");
    assert_eq!(
        recreated.unwrap(),
        0,
        "no connections should be recreated on healthy pool"
    );
}

// ============================================================================
// pool_metrics 测试（metrics feature）
// ============================================================================

/// TEST-U-DPOOL-034: pool_metrics 在未设置 collector 时应返回全零
#[cfg(feature = "metrics")]
#[tokio::test]
async fn test_pool_metrics_no_collector_returns_zeros() {
    let pool = common::make_sqlite_memory_pool().await;
    let metrics = pool.pool_metrics();
    assert_eq!(metrics.slow_acquires, 0);
    assert_eq!(metrics.timeout_errors, 0);
    assert_eq!(metrics.critical_timeouts, 0);
}

// ============================================================================
// run_auto_migrate 测试（auto-migrate feature）
// ============================================================================

/// TEST-U-DPOOL-035: run_auto_migrate 在无 migrations_dir 时应返回 Ok(0)
#[cfg(feature = "auto-migrate")]
#[tokio::test]
async fn test_run_auto_migrate_no_dir_returns_zero() {
    let config = DbConfig {
        url: "sqlite::memory:".to_string(),
        auto_migrate: true,
        migrations_dir: None,
        ..Default::default()
    };
    let pool = DbPool::with_config(config).await.unwrap();
    let result = pool.run_auto_migrate().await;
    assert!(result.is_ok(), "should succeed when no migrations_dir set");
    assert_eq!(result.unwrap(), 0, "should apply 0 migrations when no dir");
}

// ============================================================================
// DbConnection 方法测试
// ============================================================================

/// TEST-U-DPOOL-036: DbConnection::as_sea_orm 应返回 Ok
#[tokio::test]
async fn test_db_connection_as_sea_orm_ok() {
    use dbnexus::DbConnection;
    let sea_conn = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    let conn = DbConnection::SeaOrm(sea_conn);
    assert!(
        conn.as_sea_orm().is_ok(),
        "as_sea_orm on SeaOrm variant should return Ok"
    );
}

/// TEST-U-DPOOL-037: DbConnection::is_duckdb 应返回 false（SeaORM 连接）
#[tokio::test]
async fn test_db_connection_is_duckdb_seaorm() {
    use dbnexus::DbConnection;
    let sea_conn = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    let conn = DbConnection::SeaOrm(sea_conn);
    assert!(!conn.is_duckdb(), "SeaOrm connection should not be duckdb");
}

/// TEST-U-DPOOL-038: DbConnection::Debug 应不 panic
#[tokio::test]
async fn test_db_connection_debug_format() {
    use dbnexus::DbConnection;
    let sea_conn = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    let conn = DbConnection::SeaOrm(sea_conn);
    let debug_str = format!("{:?}", conn);
    assert!(
        debug_str.contains("SeaOrm"),
        "debug output should contain SeaOrm: {}",
        debug_str
    );
}
