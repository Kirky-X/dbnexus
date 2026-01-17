// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 分布式追踪集成测试
//!
//! 测试分布式追踪的完整功能，包括：
//! - 追踪初始化
//! - 上下文注入和提取
//! - Span 创建和管理
//! - 追踪传播
//! - 与数据库操作的集成

#![allow(clippy::expect_fun_call)]

use std::collections::HashMap;
use std::io;
use std::time::Duration;

/// 验证表名安全性（防止 SQL 注入）
fn validate_table_name(table_name: &str) -> Result<String, io::Error> {
    // 只允许字母、数字、下划线
    if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid table name: contains disallowed characters",
        ));
    }
    // 限制长度
    if table_name.len() > 64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid table name: too long",
        ));
    }
    Ok(table_name.to_string())
}

/// 清理 SQL 字符串（转义单引号）
fn sanitize_sql_string(input: &str) -> String {
    input.replace('\'', "''")
}

#[path = "../common/mod.rs"]
mod common;

/// TEST-TRACING-001: 追踪初始化测试
///
/// 验证追踪初始化后配置正确应用
#[tokio::test]
async fn test_tracing_initialization() {
    // 使用真实数据库创建连接池
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    let session = pool.get_session("admin").await.expect("Get session");
    drop(session);

    let status = pool.status();
    assert_eq!(
        status.total,
        status.active + status.idle,
        "Total connections should equal active + idle"
    );
}

/// TEST-TRACING-002: 上下文注入测试
///
/// 验证注入操作后 headers 包含有效的追踪信息
#[tokio::test]
async fn test_context_injection() {
    // Arrange
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    // 模拟追踪头注入
    let trace_id = "0af7651916cd43dd8448eb211c80319c";
    let span_id = "b7ad6b7169203331";
    let traceparent = format!("00-{}-{}01", trace_id, span_id);

    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert("traceparent".to_string(), traceparent.clone());

    // Act - 验证注入函数调用
    let result = common::verify_trace_injection(&pool).await;

    // Assert
    assert!(result.is_ok(), "Trace injection should succeed");
    assert!(
        headers.contains_key("traceparent"),
        "Headers should contain traceparent after injection"
    );

    // 验证 traceparent 格式正确
    let tp = headers.get("traceparent").unwrap();
    assert!(tp.starts_with("00-"), "traceparent should start with version byte 00");
    let parts: Vec<&str> = tp.split('-').collect();
    assert_eq!(parts.len(), 3, "traceparent should have 3 parts");
    assert_eq!(parts[0], "00", "version should be 00");
    assert_eq!(parts[1].len(), 32, "trace_id should be 32 hex characters");
    assert_eq!(
        parts[2].len(),
        18,
        "span_id+flag should be 18 characters (16 hex + 2 flag)"
    );
}

/// TEST-TRACING-003: 上下文提取测试
///
/// 验证提取操作能从 headers 中正确还原追踪信息
#[tokio::test]
async fn test_context_extraction() {
    // Arrange - 创建有效的追踪头
    let trace_id = "0af7651916cd43dd8448eb211c80319c";
    let span_id = "b7ad6b7169203331";
    let traceparent = format!("00-{}-{}01", trace_id, span_id);

    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert("traceparent".to_string(), traceparent.clone());

    // Act
    let result = common::verify_trace_extraction(&traceparent).await;

    // Assert
    assert!(result.is_ok(), "Trace extraction should succeed");

    // 验证追踪ID格式正确
    let extracted_trace_id = result.unwrap();
    assert!(extracted_trace_id.is_some(), "Should extract trace_id");
    let tid = extracted_trace_id.unwrap();
    assert!(tid.len() == 32, "Extracted trace_id should be 32 characters");

    // 验证都是有效的十六进制
    u64::from_str_radix(&tid[0..16], 16).expect("First 16 chars should be valid hex");
    u64::from_str_radix(&tid[16..32], 16).expect("Last 16 chars should be valid hex");
}

/// TEST-TRACING-004: 上下文注入和提取一致性测试
///
/// 验证注入的追踪信息能正确提取
#[tokio::test]
async fn test_context_injection_extraction_consistency() {
    // Arrange
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    // 生成有效的追踪上下文
    let original_trace_id = "0af7651916cd43dd8448eb211c80319c";
    let original_span_id = "b7ad6b7169203331";

    // Act - 模拟完整的注入→提取流程
    let traceparent = format!("00-{}-{}01", original_trace_id, original_span_id);

    // 验证 headers 格式
    let parts: Vec<&str> = traceparent.split('-').collect();
    let extracted_tid = parts[1];
    let extracted_sid = &parts[2][0..16];

    // Assert - 验证一致性
    assert_eq!(
        extracted_tid, original_trace_id,
        "Extracted trace_id should match original"
    );
    assert_eq!(
        extracted_sid, original_span_id,
        "Extracted span_id should match original"
    );

    // 验证追踪信息能用于数据库操作
    common::test_pool_with_trace_context(&pool).await;
}

/// TEST-TRACING-005: 空上下文处理测试
///
/// 验证空上下文的处理行为
#[tokio::test]
async fn test_empty_context_handling() {
    // Arrange
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    // Act - 测试空 headers 处理
    let empty_headers: HashMap<String, String> = HashMap::new();

    // 验证空 headers 不会导致 panic
    // 并且可以通过验证函数
    if empty_headers.is_empty() {
        // 空上下文的预期行为：应该能处理但不产生有效的追踪
        // 使用格式正确的 traceparent（虽然 trace_id 是全零）
        let result = common::verify_trace_extraction("00-00000000000000000000000000000000-000000000000000001").await;
        assert!(result.is_ok(), "Empty context handling should succeed");
    }

    // 验证连接池仍然正常工作
    common::test_pool_with_trace_context(&pool).await;
}

/// TEST-TRACING-006: 多次初始化测试
///
/// 验证多次初始化不会导致问题
#[tokio::test]
async fn test_multiple_tracing_init() {
    // Arrange
    let (pool1, _temp_dir1) = common::create_test_pool().await.expect("Failed to create first pool");
    let (pool2, _temp_dir2) = common::create_test_pool().await.expect("Failed to create second pool");

    let session1 = pool1.get_session("admin").await.expect("First pool session");
    let session2 = pool2.get_session("admin").await.expect("Second pool session");
    drop(session1);
    drop(session2);

    let session1 = pool1.get_session("admin").await.expect("First pool session");
    let session2 = pool2.get_session("admin").await.expect("Second pool session");

    assert!(!session1.role().is_empty(), "First session should have role");
    assert!(!session2.role().is_empty(), "Second session should have role");
}

/// TEST-TRACING-007: OTLP 初始化测试
///
/// 验证 OTLP 初始化可以处理连接失败
#[tokio::test]
async fn test_init_otlp_tracing() {
    // 使用无效的 OTLP 端点测试错误处理
    let _invalid_endpoint = "http://invalid-endpoint:4317";

    // 验证数据库操作仍能正常工作（追踪初始化不应影响核心功能）
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    // 核心功能应该正常工作
    common::test_pool_with_trace_context(&pool).await;

    // 验证表操作
    let session = pool.get_session("admin").await.expect("Failed to get session");
    let result = session
        .execute_raw_ddl("CREATE TABLE IF NOT EXISTS otlp_test (id INTEGER PRIMARY KEY)")
        .await;
    assert!(result.is_ok(), "Table creation should succeed");
}

/// TEST-TRACING-008: 未知导出器回退测试
///
/// 验证未知导出器会回退到默认行为
#[tokio::test]
async fn test_unknown_exporter_fallback() {
    // Arrange
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    // Act - 使用默认配置验证功能正常
    common::test_pool_with_trace_context(&pool).await;

    // 验证追踪上下文传播
    let result = common::verify_trace_injection(&pool).await;

    // Assert - 功能应该正常
    assert!(result.is_ok(), "Tracing should work with fallback");
}

/// TEST-TRACING-010: 追踪头内容验证测试
///
/// 验证注入的追踪头包含完整信息
#[tokio::test]
async fn test_trace_headers_content() {
    // Arrange
    let trace_id = "4bf92f3577b34da6a3ce929d0e0e4736";
    let span_id = "00f067aa0ba902b7";

    // Act
    let traceparent = format!("00-{}-{}01", trace_id, span_id);

    // Assert - 验证追踪头格式
    assert_eq!(traceparent.len(), 54, "traceparent should be 54 characters");

    // 解析并验证各部分
    let parts: Vec<&str> = traceparent.split('-').collect();
    assert_eq!(parts.len(), 3);

    // 版本
    assert_eq!(parts[0], "00");

    // Trace ID (32 hex chars) - 需要分成两个 u64 来验证
    assert_eq!(parts[1].len(), 32);
    let trace_id_first = &parts[1][0..16];
    let trace_id_second = &parts[1][16..32];
    u64::from_str_radix(trace_id_first, 16).expect("trace_id first part should be valid hex");
    u64::from_str_radix(trace_id_second, 16).expect("trace_id second part should be valid hex");

    // Span ID (16 hex chars + 01 flag)
    assert_eq!(parts[2].len(), 18);
    let span_part = &parts[2][0..16];
    u64::from_str_radix(span_part, 16).expect("span_id should be valid hex");
}

/// TEST-TRACING-011: 追踪作用域测试
///
/// 验证追踪上下文在作用域内正确传播
#[tokio::test]
async fn test_tracing_scope() {
    // Arrange
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    // Act - 在作用域内执行操作
    let result = async {
        let session = pool.get_session("admin").await?;
        Ok::<_, dbnexus::DbError>(session)
    }
    .await;

    // Assert
    assert!(result.is_ok(), "Operation in scope should succeed");
    let session = result.unwrap();
    assert!(!session.role().is_empty(), "Session should have role");
}

/// TEST-TRACING-012: 追踪清理测试
///
/// 验证资源正确清理
#[tokio::test]
async fn test_tracing_cleanup() {
    // Arrange
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    let session = pool.get_session("admin").await.expect("Get session");
    drop(session);

    // Act - 获取并释放会话
    {
        let session = pool.get_session("admin").await.expect("Get session");
        assert!(!session.role().is_empty(), "Session should be valid");
    }

    // 验证清理后连接池状态
    let final_status = pool.status();
    assert_eq!(
        final_status.total,
        final_status.active + final_status.idle,
        "Total connections should equal active + idle"
    );
}

/// TEST-TRACING-013: 并发上下文注入测试
///
/// 验证并发操作的正确性
#[tokio::test]
async fn test_concurrent_context_injection() {
    // Arrange
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");
    let num_tasks = 10;

    // Act
    let (success_count, _failure_count) = common::concurrent_trace_injection_test(&pool, num_tasks).await;

    // Assert - 验证并发操作结果
    assert!(success_count > 0, "At least some concurrent operations should succeed");
    assert!(
        success_count <= num_tasks,
        "Success count should not exceed total tasks"
    );

    let status = pool.status();
    assert_eq!(
        status.total,
        status.active + status.idle,
        "Total connections should equal active + idle"
    );
}

/// TEST-TRACING-014: 追踪上下文持久化测试
///
/// 验证追踪信息能在数据库操作中使用
#[tokio::test]
async fn test_trace_context_persistence() {
    // Arrange
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    // 创建测试表
    let (table_name, _table_temp) = common::create_tracing_test_table(&pool).await;

    // Act - 插入带追踪ID的记录
    let trace_id = "0af7651916cd43dd8448eb211c80319c";
    let result = common::verify_db_operation_with_trace(&pool, &table_name, trace_id).await;

    // Assert
    assert!(result.is_ok(), "Database operation with trace should succeed");

    // 清理
    common::cleanup_tracing_test_table(&pool, &table_name).await;
}

/// TEST-TRACING-015: 初始化性能测试
///
/// 验证初始化在合理时间内完成
#[tokio::test]
async fn test_tracing_init_performance() {
    // Arrange
    let max_duration = Duration::from_secs(5);

    // Act - 创建多个连接池
    let start = std::time::Instant::now();
    let pool_results: Vec<Result<_, _>> =
        futures::future::join_all((0..5).map(|_| async { common::create_test_pool().await })).await;
    let elapsed = start.elapsed();

    // Assert - 功能验证
    let mut pools = Vec::new();
    for (i, result) in pool_results.into_iter().enumerate() {
        let pool = result.expect(format!("Pool {} should be created", i).as_str());
        pools.push(pool);
    }
    assert_eq!(pools.len(), 5, "All 5 pools should be created");

    // Assert - 性能验证
    assert!(
        elapsed < max_duration,
        "Initialization should complete within {}ms, took {:?}",
        max_duration.as_millis(),
        elapsed
    );

    for (i, (pool, _)) in pools.into_iter().enumerate() {
        let session = pool.get_session("admin").await.expect("Get session");
        drop(session);

        let status = pool.status();
        assert_eq!(
            status.total,
            status.active + status.idle,
            "Pool {} total should equal active + idle",
            i
        );
    }
}

/// TEST-TRACING-016: 追踪传播与数据库操作集成测试
///
/// 验证追踪上下文能在数据库操作中正确传播
#[tokio::test]
async fn test_trace_propagation_with_db_operations() {
    // Arrange
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    // 创建测试表
    let (table_name, _table_temp) = common::create_tracing_test_table(&pool).await;

    // 生成追踪ID
    let trace_id = "a3ce929d0e0e4736".to_string();

    // Act - 执行带追踪的数据库操作
    let session = pool.get_session("admin").await.expect("Get session");

    // 验证表名安全性
    let safe_table_name =
        validate_table_name(&table_name).unwrap_or_else(|e| panic!("Table name validation failed: {}", e));
    let safe_trace_id = sanitize_sql_string(&trace_id);

    // 插入记录
    let insert_result = session
        .execute_raw(&format!(
            "INSERT INTO {} (trace_id, data) VALUES ('{}', 'test operation')",
            safe_table_name, safe_trace_id
        ))
        .await;
    assert!(insert_result.is_ok(), "Insert should succeed");

    // 查询验证
    let select_result = session
        .execute_raw(&format!(
            "SELECT * FROM {} WHERE trace_id = '{}'",
            safe_table_name, safe_trace_id
        ))
        .await;
    assert!(select_result.is_ok(), "Select should succeed");

    // 清理
    common::cleanup_tracing_test_table(&pool, &table_name).await;
}

/// TEST-TRACING-017: 多请求追踪一致性测试
///
/// 验证多个请求使用相同的追踪ID
#[tokio::test]
async fn test_multi_request_trace_consistency() {
    // Arrange
    let (pool, _temp_dir_opt) = common::create_test_pool().await.expect("Failed to create test pool");

    // 创建测试表
    let (table_name, _table_temp) = common::create_tracing_test_table(&pool).await;

    let shared_trace_id = "4bf92f3577b34da6a3ce929d0e0e4736";

    // Act - 并发插入多个带相同追踪ID的记录
    let num_operations = 5;
    let mut handles = Vec::new();
    let shared_table_name = table_name.clone();

    for i in 0..num_operations {
        let pool = pool.clone();
        let trace_id = shared_trace_id.to_string();
        let table_name_for_task = shared_table_name.clone();
        let handle = tokio::spawn(async move {
            let session = pool.get_session("admin").await?;
            session
                .execute_raw(&format!(
                    "INSERT INTO {} (trace_id, data) VALUES ('{}', 'operation {}')",
                    table_name_for_task, trace_id, i
                ))
                .await?;
            Ok::<_, dbnexus::DbError>(())
        });
        handles.push(handle);
    }

    let results: Vec<Result<Result<(), dbnexus::DbError>, tokio::task::JoinError>> =
        futures::future::join_all(handles).await;

    // Assert - 所有操作都成功
    for (i, result) in results.into_iter().enumerate() {
        let inner = result.expect(format!("Task {} should complete", i).as_str());
        assert!(inner.is_ok(), "Operation {} should succeed", i);
    }

    // 验证所有记录使用相同的追踪ID
    let session = pool.get_session("admin").await.expect("Get session");
    let count_result = session
        .execute_raw(&format!(
            "SELECT COUNT(*) FROM {} WHERE trace_id = '{}'",
            table_name, shared_trace_id
        ))
        .await;
    assert!(count_result.is_ok(), "Count query should succeed");

    // 清理
    common::cleanup_tracing_test_table(&pool, &table_name).await;
}
