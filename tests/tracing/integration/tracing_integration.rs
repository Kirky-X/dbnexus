// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! TracingGuard 集成测试
//!
//! 测试 TracingGuard 与数据库 Session 的集成：
//! - TracingGuard drop 不 panic
//! - tracing feature 启用时 Session 操作正常（span 自动创建或 no-op）
//! - tracing 不破坏核心数据库功能

#![allow(clippy::expect_fun_call)]

#[path = "../../common/mod.rs"]
mod common;

// ============================================================================
// TEST-TRACING-INT-001: TracingGuard drop 触发 flush 不 panic
// ============================================================================

/// 验证 TracingGuard drop 调用 shutdown_tracer_provider 不 panic
#[tokio::test]
async fn test_tracing_guard_drop_safe() {
    // 尝试初始化（可能已被其他测试初始化，返回 AlreadyInitialized）
    let guard_result = dbnexus::TracingGuard::init_with_otlp("http://localhost:4317");
    if let Ok(guard) = guard_result {
        // 显式 drop，验证 shutdown_tracer_provider 不 panic
        drop(guard);
    }
    // 即使未初始化（AlreadyInitialized），此测试也验证不 panic
}

// ============================================================================
// TEST-TRACING-INT-002: tracing feature 启用时 Session 操作正常
// ============================================================================

/// 验证 tracing feature 启用时数据库核心操作不受影响
#[tokio::test]
async fn test_session_operations_with_tracing() {
    let pool = common::make_sqlite_memory_pool().await;
    let session = pool.get_session("admin").await.expect("get_session failed");

    // 执行 DDL（tracing::instrument 自动创建 span 或 no-op）
    let result = session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS tracing_test (id INTEGER PRIMARY KEY)")
        .await;
    assert!(
        result.is_ok(),
        "DDL should succeed with tracing feature: {:?}",
        result.err()
    );

    // 执行 DML
    let result = session.execute_raw("INSERT INTO tracing_test (id) VALUES (1)").await;
    assert!(
        result.is_ok(),
        "DML should succeed with tracing feature: {:?}",
        result.err()
    );

    // 执行查询
    let result = session.execute_raw("SELECT * FROM tracing_test").await;
    assert!(
        result.is_ok(),
        "SELECT should succeed with tracing feature: {:?}",
        result.err()
    );
}

// ============================================================================
// TEST-TRACING-INT-003: tracing 不破坏连接池生命周期
// ============================================================================

/// 验证 tracing feature 启用时连接池创建、session 获取/释放、status 正常
#[tokio::test]
async fn test_pool_lifecycle_with_tracing() {
    let pool = common::make_sqlite_memory_pool().await;

    {
        let session = pool.get_session("admin").await.expect("get_session failed");
        assert!(!session.role().is_empty(), "session should have role");
        // session drop 时 span（如有）应正确结束
    }

    let status = pool.status();
    assert_eq!(
        status.total,
        status.active + status.idle,
        "pool status should be consistent: total={} active={} idle={}",
        status.total,
        status.active,
        status.idle
    );
}
