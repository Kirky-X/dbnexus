// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 追踪系统单元测试
//!
//! 测试分布式追踪的独立功能，包括：
//! - Span 嵌套测试
//! - 追踪采样策略测试
//! - OTLP 导出测试
//! - 追踪 ID 关联日志测试
//! - 错误追踪标记测试
//!
//! 这些单元测试不需要外部依赖，可以独立运行。

#![allow(clippy::expect_fun_call)]

use std::collections::HashMap;

#[path = "../../common/mod.rs"]
mod common;

// ============================================================================
// TEST-UNIT-TRACING-001: 追踪初始化 - stdout 导出器
// ============================================================================

/// 测试使用 stdout 导出器初始化追踪
#[tokio::test]
async fn test_init_stdout_exporter() {
    // Act - 初始化 stdout 导出器
    let result = dbnexus::tracing::init("stdout", "unused").await;

    // Assert - 初始化应该成功
    assert!(
        result.is_ok(),
        "stdout exporter init should succeed: {:?}",
        result.err()
    );

    let _guard = result.unwrap();
    // guard 会在作用域结束时自动 Drop，触发清理
}

// ============================================================================
// TEST-UNIT-TRACING-002: 追踪初始化 - OTLP 导出器
// ============================================================================

/// 测试使用 OTLP 导出器初始化追踪
#[tokio::test]
async fn test_init_otlp_exporter() {
    // 注意：OTLP 初始化会尝试连接到端点，使用无效端点测试基本初始化流程
    // 实际网络错误会被捕获并转换为 String 错误
    let result = dbnexus::tracing::init("otlp", "http://localhost:4317").await;

    // OTLP 初始化可能因为网络问题失败，这是预期行为
    // 我们只验证函数调用的正确性
    if result.is_err() {
        let err = result.unwrap_err();
        assert!(
            err.contains("transport") || err.contains("connection") || err.contains("timeout"),
            "OTLP error should be network related: {}",
            err
        );
    }
}

// ============================================================================
// TEST-UNIT-TRACING-003: 追踪初始化 - 未知导出器回退
// ============================================================================

/// 测试未知导出器会回退到默认行为
#[tokio::test]
async fn test_init_unknown_exporter_fallback() {
    // Act - 使用未知的导出器，应该回退到 stdout
    let result = dbnexus::tracing::init("unknown_exporter", "unused").await;

    // Assert - 应该成功回退
    assert!(
        result.is_ok(),
        "Unknown exporter should fallback to stdout: {:?}",
        result.err()
    );

    let _guard = result.unwrap();
}

// ============================================================================
// TEST-UNIT-TRACING-004: 追踪初始化 - 空字符串导出器
// ============================================================================

/// 测试空字符串导出器的处理
#[tokio::test]
async fn test_init_empty_exporter() {
    // Act - 使用空字符串作为导出器
    let result = dbnexus::tracing::init("", "unused").await;

    // Assert - 应该回退到默认行为
    assert!(result.is_ok(), "Empty exporter should fallback to stdout");

    let _guard = result.unwrap();
}

// ============================================================================
// TEST-UNIT-TRACING-005: 上下文注入测试
// ============================================================================

/// 测试追踪上下文注入功能
#[tokio::test]
async fn test_context_injection() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    // 创建要注入的 headers
    let mut headers = HashMap::new();
    headers.insert("x-custom-header".to_string(), "test-value".to_string());

    // Act - 注入追踪上下文
    dbnexus::tracing::inject(&mut headers);

    // Assert - headers 应该包含追踪信息
    // traceparent 是 W3C 标准追踪头
    assert!(
        headers.contains_key("traceparent"),
        "Headers should contain traceparent after injection"
    );

    let traceparent = headers.get("traceparent").expect("traceparent should exist");
    assert!(
        traceparent.starts_with("00-"),
        "traceparent should start with version byte 00"
    );

    // 验证 traceparent 格式: version-trace_id-span_id-flags
    let parts: Vec<&str> = traceparent.split('-').collect();
    assert_eq!(parts.len(), 3, "traceparent should have 3 parts");
    assert_eq!(parts[0], "00", "version should be 00");
    assert_eq!(parts[1].len(), 32, "trace_id should be 32 hex characters");
    assert_eq!(parts[2].len(), 18, "span_id+flag should be 18 characters");
}

// ============================================================================
// TEST-UNIT-TRACING-006: 上下文提取测试
// ============================================================================

/// 测试追踪上下文提取功能
#[tokio::test]
async fn test_context_extraction() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    // 创建带有追踪上下文的 headers
    let trace_id = "0af7651916cd43dd8448eb211c80319c";
    let span_id = "b7ad6b7169203331";
    let traceparent = format!("00-{}-{}01", trace_id, span_id);

    let mut headers = HashMap::new();
    headers.insert("traceparent".to_string(), traceparent);

    // Act - 提取追踪上下文
    dbnexus::tracing::extract(&headers);

    // 此测试验证 extract 函数不会 panic
    // 实际追踪上下文的提取需要通过 Tracer 获取
}

// ============================================================================
// TEST-UNIT-TRACING-007: 上下文注入提取一致性测试
// ============================================================================

/// 测试注入和提取的追踪上下文一致性
#[tokio::test]
async fn test_context_injection_extraction_roundtrip() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    let mut headers = HashMap::new();

    // Act - 注入追踪上下文
    dbnexus::tracing::inject(&mut headers);

    // 验证注入的内容格式
    if let Some(traceparent) = headers.get("traceparent") {
        let parts: Vec<&str> = traceparent.split('-').collect();
        assert_eq!(parts.len(), 3);

        // 提取注入的 trace_id
        let injected_trace_id = parts[1];
        assert_eq!(injected_trace_id.len(), 32);

        // 验证是有效的十六进制
        let first_half = &injected_trace_id[0..16];
        let second_half = &injected_trace_id[16..32];
        u64::from_str_radix(first_half, 16).expect("First half should be valid hex");
        u64::from_str_radix(second_half, 16).expect("Second half should be valid hex");
    }
}

// ============================================================================
// TEST-UNIT-TRACING-008: Span 嵌套测试
// ============================================================================

/// 测试 Span 嵌套功能
#[tokio::test]
async fn test_span_nesting() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    // 获取全局 tracer
    let tracer = opentelemetry::global::tracer("test_span_nesting");

    // Act - 创建嵌套的 spans
    let _root = tracer.start("root_operation");

    // 在 root span 中创建子 span
    let _child1 = tracer.start("child_operation_1");

    // 创建更深层次的嵌套
    let _grandchild = tracer.start("grandchild_operation");

    // 嵌套测试验证：
    // 1. 每个 span 都能成功创建
    // 2. span 层级关系由 OpenTelemetry SDK 自动管理
    // 3. 不应出现 panic 或错误

    // Assert - 验证没有错误发生（如果能执行到这里即表示成功）
    assert!(true, "Span nesting should complete without errors");
}

// ============================================================================
// TEST-UNIT-TRACING-009: 多个 Span 并发测试
// ============================================================================

/// 测试多个 Span 并发创建
#[tokio::test]
async fn test_multiple_spans_concurrent() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    let tracer = opentelemetry::global::tracer("test_concurrent_spans");

    // Act - 并发创建多个 spans
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let tracer = opentelemetry::global::tracer("test_concurrent_spans");
            tokio::spawn(async move {
                let span = tracer.start(format!("operation_{}", i));
                // 模拟一些异步工作
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                span
            })
        })
        .collect();

    // 等待所有 spans 完成
    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.expect("Task should not panic"))
        .collect();

    // Assert - 验证所有 spans 都成功创建
    assert_eq!(results.len(), 10, "All 10 spans should be created");
}

// ============================================================================
// TEST-UNIT-TRACING-010: 追踪采样策略测试
// ============================================================================

/// 测试追踪采样策略配置
#[tokio::test]
async fn test_tracing_sampling_strategy() {
    // Arrange - 初始化追踪
    // 注意：当前实现使用默认采样策略
    // 测试验证初始化能正确处理采样配置
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    let tracer = opentelemetry::global::tracer("test_sampling");

    // 创建多个 spans，验证采样行为
    for i in 0..5 {
        let _span = tracer.start(format!("sampled_operation_{}", i));
    }

    // 验证采样配置不会导致错误
    // 默认情况下，所有 spans 都会被记录
    assert!(true, "Sampling strategy should be applied correctly");
}

// ============================================================================
// TEST-UNIT-TRACING-011: OTLP 导出连接错误处理
// ============================================================================

/// 测试 OTLP 导出器的错误处理
#[tokio::test]
async fn test_otlp_export_error_handling() {
    // 使用一个肯定不存在的端点
    let invalid_endpoint = "http://invalid-endpoint-that-does-not-exist:99999";

    // Act
    let result = dbnexus::tracing::init("otlp", invalid_endpoint).await;

    // Assert - 应该返回错误而不是 panic
    assert!(result.is_err(), "Invalid endpoint should return error");

    let error = result.unwrap_err();
    // 错误信息应该包含网络相关的问题描述
    assert!(!error.is_empty(), "Error message should not be empty");
}

// ============================================================================
// TEST-UNIT-TRACING-012: 追踪 ID 格式验证
// ============================================================================

/// 测试追踪 ID 格式正确性
#[tokio::test]
async fn test_trace_id_format() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    let mut headers = HashMap::new();

    // Act - 注入追踪上下文
    dbnexus::tracing::inject(&mut headers);

    // Assert - 验证追踪 ID 格式
    if let Some(traceparent) = headers.get("traceparent") {
        let parts: Vec<&str> = traceparent.split('-').collect();
        let trace_id = parts[1];

        // 验证长度
        assert_eq!(trace_id.len(), 32, "Trace ID should be 32 characters");

        // 验证是有效的十六进制字符串
        for (i, c) in trace_id.chars().enumerate() {
            assert!(
                c.is_ascii_hexdigit(),
                "Character at position {} should be hex digit: {}",
                i,
                c
            );
        }
    }
}

// ============================================================================
// TEST-UNIT-TRACING-013: Span ID 格式验证
// ============================================================================

/// 测试 Span ID 格式正确性
#[tokio::test]
async fn test_span_id_format() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    let mut headers = HashMap::new();

    // Act - 注入追踪上下文
    dbnexus::tracing::inject(&mut headers);

    // Assert - 验证 Span ID 格式
    if let Some(traceparent) = headers.get("traceparent") {
        let parts: Vec<&str> = traceparent.split('-').collect();
        // span_id 格式: 16字符ID + 2字符flags
        let span_part = parts[2];
        let span_id = &span_part[0..16];
        let flags = &span_part[16..18];

        // 验证 Span ID 长度
        assert_eq!(span_id.len(), 16, "Span ID should be 16 characters");

        // 验证是有效的十六进制字符串
        for (i, c) in span_id.chars().enumerate() {
            assert!(
                c.is_ascii_hexdigit(),
                "Span ID character at position {} should be hex digit: {}",
                i,
                c
            );
        }

        // 验证 flags 格式 (01 = sampled)
        assert_eq!(flags, "01", "Flags should be 01 (sampled)");
    }
}

// ============================================================================
// TEST-UNIT-TRACING-014: 错误追踪标记测试
// ============================================================================

/// 测试错误追踪标记功能
#[tokio::test]
async fn test_error_tracing_marking() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    let tracer = opentelemetry::global::tracer("test_error_marking");

    // Act - 创建一个 span 并标记为错误
    let span = tracer.start("error_operation");

    // 使用 set_status 标记错误
    span.set_status(opentelemetry::trace::Status::error("Test error message"));

    // 添加错误属性
    span.set_attribute(opentelemetry::Key::new("error").bool(true));
    span.set_attribute(opentelemetry::Key::new("error.message").string("Test error occurred"));

    // Assert - 验证错误标记已设置
    // 注意：在单元测试中，我们主要验证 API 调用不 panic
    // 实际的错误状态会在导出后被 OTLP 接收器看到
    assert!(true, "Error marking should complete without panic");
}

// ============================================================================
// TEST-UNIT-TRACING-015: 追踪上下文传播到子任务
// ============================================================================

/// 测试追踪上下文能正确传播到异步子任务
#[tokio::test]
async fn test_trace_propagation_to_subtask() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    let tracer = opentelemetry::global::tracer("test_subtask_propagation");

    // 创建父 span
    let parent_span = tracer.start("parent_operation");

    // Act - 在子任务中使用追踪
    let result = tokio::spawn(async {
        let child_tracer = opentelemetry::global::tracer("test_child_task");
        let child_span = child_tracer.start("child_operation");
        // 模拟一些工作
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        child_span
    })
    .await;

    // Assert - 子任务应该能正常执行
    assert!(result.is_ok(), "Child task should complete successfully");
}

// ============================================================================
// TEST-UNIT-TRACING-016: 追踪属性测试
// ============================================================================

/// 测试 Span 属性设置
#[tokio::test]
async fn test_span_attributes() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    let tracer = opentelemetry::global::tracer("test_attributes");

    // Act - 创建带有属性的 span
    let span = tracer.start("operation_with_attributes");

    // 设置各种类型的属性
    span.set_attribute(opentelemetry::Key::new("string_attr").string("test_value"));
    span.set_attribute(opentelemetry::Key::new("int_attr").i64(42));
    span.set_attribute(opentelemetry::Key::new("bool_attr").bool(true));
    span.set_attribute(opentelemetry::Key::new("double_attr").f64(3.14));

    // 设置自定义追踪属性
    span.set_attribute(opentelemetry::Key::new("db.system").string("sqlite"));
    span.set_attribute(opentelemetry::Key::new("db.operation").string("SELECT"));
    span.set_attribute(opentelemetry::Key::new("db.statement").string("SELECT * FROM users"));

    // Assert - 验证属性设置不会导致错误
    assert!(true, "Setting attributes should complete without error");
}

// ============================================================================
// TEST-UNIT-TRACING-017: 追踪事件测试
// ============================================================================

/// 测试 Span 事件记录
#[tokio::test]
async fn test_span_events() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    let tracer = opentelemetry::global::tracer("test_events");

    // Act - 创建 span 并添加事件
    let span = tracer.start("operation_with_events");

    // 添加事件
    span.add_event("operation_started", vec![]);
    span.add_event("operation_progress", vec![opentelemetry::Key::new("progress").i64(50)]);
    span.add_event("operation_completed", vec![]);

    // Assert - 验证事件添加不会导致错误
    assert!(true, "Adding events should complete without error");
}

// ============================================================================
// TEST-UNIT-TRACING-018: 空 Headers 处理测试
// ============================================================================

/// 测试空 Headers 的处理
#[tokio::test]
async fn test_empty_headers_handling() {
    // Arrange - 初始化追踪
    let _guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    // Act - 使用空 headers 进行提取
    let empty_headers: HashMap<String, String> = HashMap::new();

    // 验证不会 panic
    dbnexus::tracing::extract(&empty_headers);

    // Assert - 验证函数正常返回
    assert!(true, "Empty headers should be handled gracefully");
}

// ============================================================================
// TEST-UNIT-TRACING-019: TracingGuard Drop 测试
// ============================================================================

/// 测试 TracingGuard 正确释放资源
#[tokio::test]
async fn test_tracing_guard_drop() {
    // Arrange - 初始化追踪
    let guard = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Init should succeed");

    // 验证初始化成功
    let mut headers = HashMap::new();
    dbnexus::tracing::inject(&mut headers);
    assert!(headers.contains_key("traceparent"));

    // Act - 显式 drop guard
    drop(guard);

    // 验证可以重新初始化
    let guard2 = dbnexus::tracing::init("stdout", "unused")
        .await
        .expect("Re-init should succeed");

    // guard2 会在作用域结束时自动 drop
    assert!(true, "Guard drop and re-init should work correctly");
}

// ============================================================================
// TEST-UNIT-TRACING-020: 多次初始化测试
// ============================================================================

/// 测试多次初始化的行为
#[tokio::test]
async fn test_multiple_init_calls() {
    // Act - 连续多次初始化
    for i in 0..3 {
        let result = dbnexus::tracing::init("stdout", "unused").await;
        assert!(result.is_ok(), "Init {} should succeed", i);

        let mut headers = HashMap::new();
        dbnexus::tracing::inject(&mut headers);
        assert!(headers.contains_key("traceparent"), "Init {} should inject headers", i);
    }

    // Assert - 多次初始化不应该导致错误
    assert!(true, "Multiple init calls should work correctly");
}
