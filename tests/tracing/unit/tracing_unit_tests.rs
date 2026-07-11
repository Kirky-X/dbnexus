// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! TracingGuard 单元测试
//!
//! 测试 [`dbnexus::TracingGuard::init_with_otlp`] 的初始化、重复初始化安全、
//! 错误类型 Display。由于全局 tracing subscriber 只能设置一次（进程级限制），
//! 涉及 init 的测试通过 `TEST_MUTEX` 串行化。

#![allow(clippy::expect_fun_call)]

use std::sync::Mutex;

/// 串行化 init 测试：tracing 全局 subscriber 只能设置一次，并行调用会导致竞争
static TEST_MUTEX: Mutex<()> = Mutex::new(());

// ============================================================================
// TEST-UNIT-TRACING-001: TracingError Display 实现
// ============================================================================

/// 验证 TracingError 各变体的 Display 输出包含关键信息
#[test]
fn test_tracing_error_display() {
    let err = dbnexus::TracingError::ExporterInit("connection refused".to_string());
    let msg = err.to_string();
    assert!(
        msg.contains("OTLP exporter"),
        "ExporterInit should mention OTLP exporter: {}",
        msg
    );
    assert!(
        msg.contains("connection refused"),
        "ExporterInit should contain detail: {}",
        msg
    );

    let err = dbnexus::TracingError::ProviderSetup("invalid config".to_string());
    assert!(err.to_string().contains("Tracer provider"), "ProviderSetup: {}", err);

    let err = dbnexus::TracingError::AlreadyInitialized;
    let msg = err.to_string();
    assert!(msg.contains("already initialized"), "AlreadyInitialized: {}", msg);

    let err = dbnexus::TracingError::SubscriberSetup("conflict".to_string());
    assert!(err.to_string().contains("subscriber"), "SubscriberSetup: {}", err);
}

// ============================================================================
// TEST-UNIT-TRACING-002: init_with_otlp 首次调用（Ok 或 AlreadyInitialized）
// ============================================================================

/// 验证 init_with_otlp 可调用且不 panic。
///
/// 由于全局 subscriber 只能设置一次，本测试可能在其他测试之后运行，
/// 此时返回 `AlreadyInitialized` 也属正常行为。
///
/// 注意：`init_with_otlp` 使用 Tokio batch exporter，必须在 Tokio 运行时内调用。
#[tokio::test]
async fn test_init_with_otlp_callable() {
    let _guard = TEST_MUTEX.lock().expect("TEST_MUTEX poisoned");
    let result = dbnexus::TracingGuard::init_with_otlp("http://localhost:4317");
    match result {
        Ok(guard) => {
            // 首次成功：guard 可以 drop
            drop(guard);
        }
        Err(dbnexus::TracingError::AlreadyInitialized) => {
            // 已被其他测试初始化：正常
        }
        Err(e) => {
            // OTLP exporter 创建可能因 tonic 初始化失败（非 panic）—— 可接受
            // 但不应出现 panic
            eprintln!(
                "init_with_otlp returned error (acceptable if exporter init fails): {}",
                e
            );
        }
    }
}

// ============================================================================
// TEST-UNIT-TRACING-003: 重复初始化返回 AlreadyInitialized
// ============================================================================

/// 验证第二次调用 init_with_otlp 返回 AlreadyInitialized（不 panic）
#[tokio::test]
async fn test_init_with_otlp_repeat_returns_error() {
    let _guard = TEST_MUTEX.lock().expect("TEST_MUTEX poisoned");
    let first = dbnexus::TracingGuard::init_with_otlp("http://localhost:4317");
    let second = dbnexus::TracingGuard::init_with_otlp("http://localhost:4317");

    // 至少第二次应该返回 AlreadyInitialized（首次可能 Ok 或 AlreadyInitialized）
    match (&first, &second) {
        (Ok(_), Err(dbnexus::TracingError::AlreadyInitialized)) => {
            // 首次成功、第二次正确拒绝
        }
        (Err(dbnexus::TracingError::AlreadyInitialized), Err(dbnexus::TracingError::AlreadyInitialized)) => {
            // 两者都已被其他测试初始化
        }
        (Err(_), Err(dbnexus::TracingError::AlreadyInitialized)) => {
            // 首次因 exporter 失败、第二次因 OnceLock 未设置但仍返回 AlreadyInitialized
            // （exporter 失败时 OnceLock 未 set，但 global subscriber 可能已被其他测试设置）
        }
        _ => {
            panic!(
                "Unexpected combination: first={:?}, second={:?}",
                first.is_ok(),
                second.is_ok()
            );
        }
    }
}

// ============================================================================
// TEST-UNIT-TRACING-004: TracingGuard 是 Send（可跨 tokio 任务传递）
// ============================================================================

#[test]
fn test_tracing_guard_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<dbnexus::TracingGuard>();
}
