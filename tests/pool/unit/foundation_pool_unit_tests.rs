// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in project root for full license information.

//! foundation::pool 模块单元测试
//!
//! 覆盖：
//! - `PoolConfig` 默认值、`validate()` 各错误路径
//! - `PoolStatus` 默认值与 Clone
//! - `PoolError` / `PoolConfigError` Display
//! - `new_in_memory()` 工厂函数与 `MemoryPool` 的 acquire/release/get_session/health_check/shutdown
//! - 并发 acquire 到达 max_connections 后报 `PoolExhausted`

use dbnexus::foundation::pool::{
    new_in_memory, PoolConfig, PoolConfigError, PoolError, PoolStatus, PoolLifecycle, PoolReader,
    PoolWriter,
};

// ============================================================================
// PoolConfig 测试（foundation::pool 版本，含 url 字段）
// ============================================================================

/// TEST-U-FPOOL-001: PoolConfig 默认值
#[test]
fn test_pool_config_default_has_expected_values() {
    let config = PoolConfig::default();
    assert_eq!(config.url, "");
    assert_eq!(config.max_connections, 20);
    assert_eq!(config.min_connections, 5);
    assert_eq!(config.idle_timeout, 300);
    assert_eq!(config.acquire_timeout, 5000);
}

/// TEST-U-FPOOL-002: PoolConfig 自定义字段
#[test]
fn test_pool_config_custom_preserves_values() {
    let config = PoolConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 50,
        min_connections: 10,
        idle_timeout: 600,
        acquire_timeout: 10000,
    };
    assert_eq!(config.url, "sqlite::memory:");
    assert_eq!(config.max_connections, 50);
    assert_eq!(config.min_connections, 10);
}

/// TEST-U-FPOOL-003: validate() 有效配置应返回 Ok
#[test]
fn test_pool_config_validate_ok() {
    let config = PoolConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 10,
        min_connections: 2,
        ..Default::default()
    };
    assert!(config.validate().is_ok());
}

/// TEST-U-FPOOL-004: validate() 空 url 应返回 MissingField
#[test]
fn test_pool_config_validate_missing_url() {
    let config = PoolConfig {
        url: String::new(),
        max_connections: 10,
        ..Default::default()
    };
    let err = config.validate().expect_err("empty url should fail validation");
    assert!(matches!(err, PoolConfigError::MissingField(ref f) if f == "url"));
}

/// TEST-U-FPOOL-005: validate() max_connections=0 应返回 InvalidValue
#[test]
fn test_pool_config_validate_max_zero() {
    let config = PoolConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 0,
        min_connections: 0,
        ..Default::default()
    };
    let err = config.validate().expect_err("max=0 should fail");
    assert!(matches!(err, PoolConfigError::InvalidValue { ref field, .. } if field == "max_connections"));
}

/// TEST-U-FPOOL-006: validate() min>max 应返回 InvalidValue
#[test]
fn test_pool_config_validate_min_exceeds_max() {
    let config = PoolConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 10,
        ..Default::default()
    };
    let err = config.validate().expect_err("min>max should fail");
    assert!(matches!(err, PoolConfigError::InvalidValue { ref field, .. } if field == "min_connections"));
}

/// TEST-U-FPOOL-007: validate() min=max 应通过（边界）
#[test]
fn test_pool_config_validate_min_equals_max() {
    let config = PoolConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: 5,
        min_connections: 5,
        ..Default::default()
    };
    assert!(config.validate().is_ok());
}

// ============================================================================
// PoolStatus 测试
// ============================================================================

/// TEST-U-FPOOL-008: PoolStatus 默认值
#[test]
fn test_pool_status_default() {
    let status = PoolStatus::default();
    assert_eq!(status.active_connections, 0);
    assert_eq!(status.max_connections, 20);
    assert_eq!(status.idle_connections, 0);
}

/// TEST-U-FPOOL-009: PoolStatus Clone 应保留字段
#[test]
fn test_pool_status_clone() {
    let status = PoolStatus {
        active_connections: 5,
        max_connections: 20,
        idle_connections: 15,
    };
    let cloned = status.clone();
    assert_eq!(status.active_connections, cloned.active_connections);
    assert_eq!(status.max_connections, cloned.max_connections);
    assert_eq!(status.idle_connections, cloned.idle_connections);
}

/// TEST-U-FPOOL-010: PoolStatus Debug 应非空
#[test]
fn test_pool_status_debug() {
    let status = PoolStatus::default();
    let debug = format!("{:?}", status);
    assert!(debug.contains("PoolStatus"));
    assert!(debug.contains("active_connections"));
}

// ============================================================================
// PoolError Display 测试
// ============================================================================

/// TEST-U-FPOOL-011: PoolError::AcquireTimeout Display
#[test]
fn test_pool_error_acquire_timeout_display() {
    let err = PoolError::AcquireTimeout;
    let msg = err.to_string();
    assert!(msg.contains("acquire") || msg.contains("timeout"), "msg = {}", msg);
}

/// TEST-U-FPOOL-012: PoolError::PoolExhausted Display
#[test]
fn test_pool_error_pool_exhausted_display() {
    let err = PoolError::PoolExhausted;
    let msg = err.to_string();
    assert!(msg.contains("exhausted"), "msg = {}", msg);
}

/// TEST-U-FPOOL-013: PoolError::ConnectionFailed Display 包含原因
#[test]
fn test_pool_error_connection_failed_display() {
    let err = PoolError::ConnectionFailed("network unreachable".to_string());
    let msg = err.to_string();
    assert!(msg.contains("network unreachable"), "msg = {}", msg);
}

/// TEST-U-FPOOL-014: PoolError::HealthCheckFailed Display
#[test]
fn test_pool_error_health_check_failed_display() {
    let err = PoolError::HealthCheckFailed("ping timeout".to_string());
    let msg = err.to_string();
    assert!(msg.contains("ping timeout"), "msg = {}", msg);
}

// ============================================================================
// PoolConfigError Display 测试
// ============================================================================

/// TEST-U-FPOOL-015: PoolConfigError::MissingField Display
#[test]
fn test_pool_config_error_missing_field_display() {
    let err = PoolConfigError::MissingField("url".to_string());
    let msg = err.to_string();
    assert!(msg.contains("url"), "msg = {}", msg);
}

/// TEST-U-FPOOL-016: PoolConfigError::InvalidValue Display 包含 field 和 reason
#[test]
fn test_pool_config_error_invalid_value_display() {
    let err = PoolConfigError::InvalidValue {
        field: "max_connections".to_string(),
        reason: "must be positive".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("max_connections"), "msg = {}", msg);
    assert!(msg.contains("must be positive"), "msg = {}", msg);
}

// ============================================================================
// new_in_memory() 与 MemoryPool 测试
// ============================================================================

/// TEST-U-FPOOL-017: new_in_memory() 应返回可用 PoolConnector
#[test]
fn test_new_in_memory_returns_pool() {
    let pool = new_in_memory();
    let status = pool.status();
    assert_eq!(status.max_connections, 20);
    assert_eq!(status.active_connections, 0);
}

/// TEST-U-FPOOL-018: MemoryPool 初始 status 应为 active=0
#[tokio::test]
async fn test_memory_pool_status_initial() {
    let pool = new_in_memory();
    let status = pool.status();
    assert_eq!(status.active_connections, 0);
    assert_eq!(status.max_connections, 20);
    assert_eq!(status.idle_connections, 20);
}

/// TEST-U-FPOOL-019: MemoryPool acquire 后 active 应增加
#[tokio::test]
async fn test_memory_pool_acquire_increments_active() {
    let pool = new_in_memory();
    let _conn = pool.acquire().await.expect("acquire should succeed");
    let status = pool.status();
    assert_eq!(status.active_connections, 1);
    assert_eq!(status.idle_connections, 19);
}

/// TEST-U-FPOOL-020: MemoryPool release 后 active 应减少
#[tokio::test]
async fn test_memory_pool_release_decrements_active() {
    let pool = new_in_memory();
    let conn = pool.acquire().await.expect("acquire should succeed");
    assert_eq!(pool.status().active_connections, 1);
    pool.release(conn).await;
    assert_eq!(pool.status().active_connections, 0);
}

/// TEST-U-FPOOL-021: MemoryPool get_session 应返回正确 role 的 Session
#[tokio::test]
async fn test_memory_pool_get_session_role() {
    let pool = new_in_memory();
    let session = pool.get_session("admin").await.expect("get_session should succeed");
    assert_eq!(session.role, "admin");
    assert!(!session.in_transaction);
}

/// TEST-U-FPOOL-022: MemoryPool health_check 应返回 Ok
#[tokio::test]
async fn test_memory_pool_health_check_ok() {
    let pool = new_in_memory();
    let result = pool.health_check().await;
    assert!(result.is_ok(), "health_check should succeed");
}

/// TEST-U-FPOOL-023: MemoryPool shutdown 后 active 应归零
#[tokio::test]
async fn test_memory_pool_shutdown_resets_active() {
    let pool = new_in_memory();
    let _conn1 = pool.acquire().await.unwrap();
    let _conn2 = pool.acquire().await.unwrap();
    assert_eq!(pool.status().active_connections, 2);

    pool.shutdown().await;
    assert_eq!(pool.status().active_connections, 0);
}

/// TEST-U-FPOOL-024: MemoryPool acquire 到达 max 后应返回 PoolExhausted
#[tokio::test]
async fn test_memory_pool_acquire_until_exhausted() {
    let pool = new_in_memory();
    let max = pool.status().max_connections;

    // 占满所有连接
    let mut conns = Vec::new();
    for _ in 0..max {
        conns.push(pool.acquire().await.expect("acquire within limit should succeed"));
    }
    assert_eq!(pool.status().active_connections, max);

    // 再 acquire 应失败
    let result = pool.acquire().await;
    assert!(matches!(result, Err(PoolError::PoolExhausted)), "expected PoolExhausted");
}

/// TEST-U-FPOOL-025: MemoryPool 释放后可重新 acquire（连接回收）
#[tokio::test]
async fn test_memory_pool_release_allows_reacquire() {
    let pool = new_in_memory();
    let conn = pool.acquire().await.unwrap();
    pool.release(conn).await;
    // 释放后应能再次获取
    let conn2 = pool.acquire().await;
    assert!(conn2.is_ok(), "should be able to acquire after release");
    assert_eq!(pool.status().active_connections, 1);
}

/// TEST-U-FPOOL-026: MemoryPool 并发 acquire 不超过 max
#[tokio::test]
async fn test_memory_pool_concurrent_acquire_respects_limit() {
    use std::sync::Arc;
    let pool = Arc::new(new_in_memory());
    let max = pool.status().max_connections;

    let mut handles = Vec::new();
    for _ in 0..(max + 5) {
        let pool_clone = Arc::clone(&pool);
        handles.push(tokio::spawn(async move {
            pool_clone.acquire().await.is_ok()
        }));
    }

    let raw_results = futures::future::join_all(handles).await;
    let results: Vec<bool> = raw_results.into_iter().map(|r| r.unwrap()).collect();
    let success_count = results.iter().filter(|&&ok| ok).count();
    let fail_count = results.iter().filter(|&&ok| !ok).count();

    // 成功数应等于 max，失败数应等于超出部分
    assert_eq!(success_count, max as usize, "success count should equal max_connections");
    assert_eq!(fail_count, 5, "fail count should equal overflow");
}
