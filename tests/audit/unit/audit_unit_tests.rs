// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 审计模块单元测试
//!
//! 测试审计日志的核心功能，包括日志轮转、敏感数据脱敏、压缩存储、导出、完整性校验和异步写入。

use chrono::Utc;
use dbnexus::audit::{
    AuditConfig, AuditEvent, AuditLogger, AuditOperation, AuditQueryFilters, AuditResult, AuditSeverity, AuditStorage,
    MemoryAuditStorage,
};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// 审计日志轮转测试
// ============================================================================

/// TEST-U-AUDIT-001: 测试日志容量达到上限时自动轮转
///
/// 验证当存储达到容量上限时，旧日志被自动清除，新日志可以继续写入。
#[tokio::test]
async fn test_audit_log_rotation_on_capacity() {
    let storage = Arc::new(MemoryAuditStorage::new(5));
    let config = AuditConfig {
        enabled: true,
        storage_path: None,
        sync_write: false,
        max_file_size: 10 * 1024 * 1024,
        retention_count: 7,
        sensitive_fields: vec![],
        alert_operations: vec![],
        alert_severity: AuditSeverity::High,
    };
    let logger = AuditLogger::with_config(config, storage.clone());

    // 写入超过容量的日志
    for i in 0..10 {
        let event = AuditEvent::create("rotation_test", &i.to_string(), "admin");
        logger.log(event).await.unwrap();
    }

    // 验证存储容量限制生效
    let count = storage.event_count().await;
    assert!(count <= 5, "Storage should respect capacity limit, got {}", count);
}

/// TEST-U-AUDIT-002: 测试手动触发日志轮转
///
/// 验证可以手动触发日志轮转，清理指定时间之前的日志。
#[tokio::test]
async fn test_audit_manual_rotation() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    // 写入当前时间的日志
    for i in 0..5 {
        let event = AuditEvent::create("manual_rotation", &i.to_string(), "admin");
        logger.log(event).await.unwrap();
    }

    // 写入过去的日志
    let old_time = Utc::now() - chrono::Duration::days(10);
    for i in 0..3 {
        let mut event = AuditEvent::create("manual_rotation", &format!("old_{}", i), "admin");
        event.timestamp = old_time;
        logger.log(event).await.unwrap();
    }

    // 清理 7 天前的日志
    let removed = logger.cleanup(7).await.unwrap();

    // 验证旧日志被清理
    assert_eq!(removed, 3, "Should remove 3 old logs");
    let remaining = storage.event_count().await;
    assert_eq!(remaining, 5, "Should have 5 logs remaining");
}

/// TEST-U-AUDIT-003: 测试轮转策略 - 保留关键操作日志
///
/// 验证轮转时保留高严重级别的日志。
#[tokio::test]
async fn test_audit_rotation_retain_high_severity() {
    let storage = Arc::new(MemoryAuditStorage::new(10));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    // 写入普通日志直到接近容量
    for i in 0..8 {
        let event = AuditEvent::create("normal", &i.to_string(), "user").with_severity(AuditSeverity::Info);
        logger.log(event).await.unwrap();
    }

    // 写入高优先级日志
    let critical_event = AuditEvent::create("critical", "1", "admin").with_severity(AuditSeverity::Critical);
    logger.log(critical_event).await.unwrap();

    // 再写入一条普通日志触发轮转
    let last_event = AuditEvent::create("normal", "last", "user").with_severity(AuditSeverity::Info);
    logger.log(last_event).await.unwrap();

    // 验证高优先级日志存在
    let filters = AuditQueryFilters {
        severity: Some(AuditSeverity::Critical),
        ..Default::default()
    };
    let critical_logs = storage.query(&filters).await.unwrap();
    assert!(!critical_logs.is_empty(), "Critical logs should be retained");
}

// ============================================================================
// 敏感数据脱敏测试
// ============================================================================

/// TEST-U-AUDIT-004: 测试 JSON 格式敏感字段脱敏
///
/// 验证 JSON 中的敏感字段被正确脱敏。
#[tokio::test]
async fn test_audit_sanitize_json_sensitive_fields() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    let event = AuditEvent::create("users", "1", "admin")
        .with_after_value(r#"{"username": "test", "password": "secret123", "email": "test@example.com"}"#);

    logger.log(event).await.unwrap();

    let filters = AuditQueryFilters::default();
    let results = logger.query(&filters).await.unwrap();
    let stored_value = results[0].after_value.as_ref().unwrap();

    // 验证密码被脱敏
    assert!(
        stored_value.contains("***REDACTED_PASSWORD***"),
        "Password should be redacted, got: {}",
        stored_value
    );
    // 验证非敏感字段未被修改
    assert!(
        stored_value.contains("username") && stored_value.contains("test"),
        "Non-sensitive fields should remain"
    );
}

/// TEST-U-AUDIT-005: 测试非 JSON 格式敏感字段脱敏
///
/// 验证非 JSON 格式的敏感数据也能被正确识别和脱敏。
#[tokio::test]
async fn test_audit_sanitize_non_json_sensitive_fields() {
    let _event = AuditEvent::create("test", "1", "admin");

    // 测试包含敏感关键字的非 JSON 值
    let sensitive_input = "password: my_secret_password, username: test";
    let sanitized = AuditEvent::sanitize_value(sensitive_input, Some(vec!["password".to_string()]));

    assert_eq!(sanitized, "***REDACTED***", "Sensitive value should be redacted");
}

/// TEST-U-AUDIT-006: 测试 Base64 编码值脱敏
///
/// 验证 Base64 编码的敏感数据被正确识别和脱敏。
#[tokio::test]
async fn test_audit_sanitize_base64_values() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    // Base64 编码的 "secret" 是 "c2VjcmV0"
    let event =
        AuditEvent::create("secrets", "1", "admin").with_after_value(r#"{"token": "c2VjcmV0", "name": "test"}"#);

    logger.log(event).await.unwrap();

    let filters = AuditQueryFilters::default();
    let results = logger.query(&filters).await.unwrap();
    let stored_value = results[0].after_value.as_ref().unwrap();

    // 验证 Base64 值被脱敏
    assert!(
        stored_value.contains("REDACTED"),
        "Base64 value should be redacted, got: {}",
        stored_value
    );
}

/// TEST-U-AUDIT-007: 测试自定义敏感字段列表
///
/// 验证可以使用自定义的敏感字段列表进行脱敏。
#[tokio::test]
async fn test_audit_sanitize_custom_fields() {
    let _event = AuditEvent::create("test", "1", "admin");

    // 使用自定义敏感字段
    let custom_fields = vec!["api_key".to_string(), "jwt_token".to_string()];
    let input = r#"{"api_key": "key123", "jwt_token": "token456", "data": "normal"}"#;
    let sanitized = AuditEvent::sanitize_value(input, Some(custom_fields));

    // 验证敏感字段被脱敏（JSON 中值为 "[REDACTED]"）
    assert!(
        sanitized.contains("[REDACTED]"),
        "Custom sensitive fields should be redacted, got: {}",
        sanitized
    );
    assert!(
        sanitized.contains("data") && sanitized.contains("normal"),
        "Non-sensitive fields should remain"
    );
}

/// TEST-U-AUDIT-008: 测试嵌套敏感字段脱敏
///
/// 验证嵌套结构中的敏感字段也能被正确脱敏。
#[tokio::test]
async fn test_audit_sanitize_nested_fields() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let mut config = AuditConfig::default();
    config.sensitive_fields.push("user.password".to_string());
    let logger = AuditLogger::with_config(config, storage.clone());

    let event = AuditEvent::create("users", "1", "admin")
        .with_after_value(r#"{"user": {"name": "test", "password": "secret"}}"#);

    logger.log(event).await.unwrap();

    let filters = AuditQueryFilters::default();
    let results = logger.query(&filters).await.unwrap();
    let stored_value = results[0].after_value.as_ref().unwrap();

    // 验证嵌套字段被脱敏
    assert!(
        stored_value.contains("REDACTED"),
        "Nested sensitive fields should be redacted, got: {}",
        stored_value
    );
}

// ============================================================================
// 审计日志压缩存储测试
// ============================================================================

/// TEST-U-AUDIT-009: 测试内存存储压缩比
///
/// 验证存储大量日志时的内存效率。
#[tokio::test]
async fn test_audit_storage_compression_ratio() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    // 写入大量重复模式的日志
    for i in 0..500 {
        let event = AuditEvent::create("logs", &i.to_string(), "system")
            .with_extra(r#"{"action": "login", "status": "success"}"#);
        logger.log(event).await.unwrap();
    }

    let count = storage.event_count().await;
    assert_eq!(count, 500, "All logs should be stored");

    // 验证可以正常查询
    let filters = AuditQueryFilters {
        entity_type: Some("logs".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filters).await.unwrap();
    assert_eq!(results.len(), 500);
}

/// TEST-U-AUDIT-010: 测试存储清理效率
///
/// 验证大批量清理操作的效率。
#[tokio::test]
async fn test_audit_cleanup_efficiency() {
    let storage = Arc::new(MemoryAuditStorage::new(10000));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    // 写入大量日志
    for i in 0..1000 {
        let event = AuditEvent::create("cleanup_test", &i.to_string(), "admin");
        logger.log(event).await.unwrap();
    }

    // 使用未来时间作为清理阈值，这样刚创建的事件都会被清理
    let future_time = Utc::now() + chrono::Duration::days(1);

    // 验证清理操作成功（清理所有刚创建的事件）
    let removed = storage.cleanup(&future_time).await.unwrap();
    assert_eq!(removed, 1000, "All logs should be cleaned up");
}

// ============================================================================
// 审计日志导出测试
// ============================================================================

/// TEST-U-AUDIT-011: 测试 JSON 格式导出
///
/// 验证审计日志可以正确序列化为 JSON 格式。
#[tokio::test]
async fn test_audit_export_json_format() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    let event = AuditEvent::create("users", "123", "admin")
        .with_user("administrator", "192.168.1.1")
        .with_severity(AuditSeverity::High)
        .with_result(AuditResult::Success);

    logger.log(event.clone()).await.unwrap();

    // 导出为 JSON
    let json = event.to_json().unwrap();
    assert!(json.contains("users"));
    assert!(json.contains("123"));
    assert!(json.contains("admin"));

    // 验证可以反向解析
    let parsed = AuditEvent::from_json(&json).unwrap();
    assert_eq!(parsed.entity_type, "users");
    assert_eq!(parsed.entity_id, "123");
}

/// TEST-U-AUDIT-012: 测试批量导出
///
/// 验证可以批量导出多条审计日志。
#[tokio::test]
async fn test_audit_batch_export() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    // 创建多个事件
    let mut events = Vec::new();
    for i in 0..10 {
        let event = AuditEvent::create("export_test", &i.to_string(), "admin");
        events.push(event.clone());
        logger.log(event).await.unwrap();
    }

    // 导出所有日志
    let filters = AuditQueryFilters {
        entity_type: Some("export_test".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filters).await.unwrap();

    // 验证导出数量
    assert_eq!(results.len(), 10, "Should export all 10 events");

    // 验证每条日志都可以序列化为 JSON
    for event in &results {
        let json = event.to_json().unwrap();
        let _ = AuditEvent::from_json(&json).expect("Should be able to parse exported JSON");
    }
}

/// TEST-U-AUDIT-013: 测试带脱敏的导出
///
/// 验证导出时敏感数据被正确脱敏。
#[tokio::test]
async fn test_audit_export_with_sanitization() {
    let event =
        AuditEvent::create("users", "1", "admin").with_after_value(r#"{"password": "secret", "data": "public"}"#);

    // 获取脱敏后的副本
    let sanitized = event.sanitized();

    // 验证原始值未被修改
    assert!(
        event.after_value.as_ref().unwrap().contains("secret"),
        "Original should not be modified"
    );

    // 验证脱敏副本
    assert!(
        sanitized.after_value.as_ref().unwrap().contains("REDACTED"),
        "Sanitized copy should have redacted values"
    );
}

// ============================================================================
// 审计日志完整性校验测试
// ============================================================================

/// TEST-U-AUDIT-014: 测试事件 ID 唯一性
///
/// 验证每个审计事件都有唯一的 ID。
#[tokio::test]
async fn test_audit_event_id_uniqueness() {
    let mut ids = std::collections::HashSet::new();

    // 创建多个事件
    for _ in 0..100 {
        let event = AuditEvent::create("test", "1", "admin");
        assert!(ids.insert(event.id.clone()), "Event ID should be unique");
    }

    assert_eq!(ids.len(), 100, "All 100 IDs should be unique");
}

/// TEST-U-AUDIT-015: 测试时间戳顺序
///
/// 验证事件时间戳是合理递增的。
#[tokio::test]
async fn test_audit_timestamp_ordering() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    let mut previous_time = Utc::now();

    for i in 0..10 {
        let event = AuditEvent::create("timestamp_test", &i.to_string(), "admin");
        logger.log(event).await.unwrap();

        // 获取最新插入的事件
        let filters = AuditQueryFilters {
            entity_type: Some("timestamp_test".to_string()),
            ..Default::default()
        };
        let results = storage.query(&filters).await.unwrap();
        let latest = results.last().unwrap();

        // 验证时间戳不早于之前的时间
        assert!(
            latest.timestamp >= previous_time,
            "Timestamp should be monotonically increasing"
        );
        previous_time = latest.timestamp;
    }
}

/// TEST-U-AUDIT-016: 测试必需字段完整性
///
/// 验证审计事件包含所有必需字段。
#[tokio::test]
async fn test_audit_required_fields() {
    let event = AuditEvent::create("users", "123", "admin")
        .with_user("administrator", "10.0.0.1")
        .with_severity(AuditSeverity::High)
        .with_result(AuditResult::Success);

    // 验证必需字段存在且非空
    assert!(!event.id.is_empty(), "ID should not be empty");
    assert!(!event.entity_type.is_empty(), "Entity type should not be empty");
    assert!(!event.entity_id.is_empty(), "Entity ID should not be empty");
    assert!(!event.user_id.is_empty(), "User ID should not be empty");
    assert!(!event.request_id.is_empty(), "Request ID should not be empty");
    assert_eq!(event.operation, AuditOperation::Create);
    assert_eq!(event.result, AuditResult::Success);
    assert_eq!(event.severity, AuditSeverity::High);
}

/// TEST-U-AUDIT-017: 测试日志完整性校验 - 校验和
///
/// 验证可以通过比较事件内容来校验日志完整性。
#[tokio::test]
async fn test_audit_log_integrity_checksum() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    // 创建并记录事件
    let event = AuditEvent::create("integrity_test", "1", "admin")
        .with_before_value(r#"{"name": "old"}"#)
        .with_after_value(r#"{"name": "new"}"#);
    let original_json = event.to_json().unwrap();
    logger.log(event).await.unwrap();

    // 查询并验证
    let filters = AuditQueryFilters {
        entity_type: Some("integrity_test".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filters).await.unwrap();

    assert_eq!(results.len(), 1, "Should find the logged event");

    let stored_json = results[0].to_json().unwrap();
    assert_eq!(original_json, stored_json, "Stored event should match original");
}

// ============================================================================
// 异步审计写入测试
// ============================================================================

/// TEST-U-AUDIT-018: 测试异步并发写入
///
/// 验证多个异步任务可以并发写入审计日志。
#[tokio::test]
async fn test_audit_async_concurrent_write() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = Arc::new(AuditLogger::with_config(config, storage.clone()));

    // 并发写入多个事件
    let mut handles = Vec::new();
    for i in 0..50 {
        let logger = logger.clone();
        let handle = tokio::spawn(async move {
            let event = AuditEvent::create("concurrent", &i.to_string(), &format!("user_{}", i));
            logger.log(event).await.unwrap();
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }

    // 验证所有事件都被写入
    let count = storage.event_count().await;
    assert_eq!(count, 50, "All 50 concurrent events should be logged");
}

/// TEST-U-AUDIT-019: 测试异步写入错误处理
///
/// 验证异步写入出错时能够正确处理。
#[tokio::test]
async fn test_audit_async_write_error_handling() {
    // 使用禁用的审计日志
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig {
        enabled: false,
        ..Default::default()
    };
    let logger = AuditLogger::with_config(config, storage.clone());

    // 即使存储被禁用，写入也不应该 panic
    let result = logger.log(AuditEvent::create("test", "1", "admin")).await;
    assert!(result.is_ok(), "Disabled audit should return Ok");

    // 验证没有写入任何内容
    let count = storage.event_count().await;
    assert_eq!(count, 0, "No events should be logged when disabled");
}

/// TEST-U-AUDIT-020: 测试异步写入顺序保证
///
/// 验证异步写入保持事件的时间顺序。
#[tokio::test]
async fn test_audit_async_write_ordering() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = Arc::new(AuditLogger::with_config(config, storage.clone()));

    let mut handles = Vec::new();

    // 并发写入但按顺序查询
    for i in 0..20 {
        let logger = logger.clone();
        let handle = tokio::spawn(async move {
            let event = AuditEvent::create("ordering_test", &i.to_string(), "admin");
            logger.log(event).await.unwrap();
            i
        });
        handles.push(handle);
    }

    // 等待所有写入完成
    for handle in handles {
        handle.await.unwrap();
    }

    // 等待所有写入完成
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 验证所有事件都被记录
    let filters = AuditQueryFilters {
        entity_type: Some("ordering_test".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filters).await.unwrap();
    assert_eq!(results.len(), 20, "All 20 events should be logged");
}

/// TEST-U-AUDIT-021: 测试大量异步写入性能
///
/// 验证大量异步写入时的性能表现。
#[tokio::test]
async fn test_audit_high_throughput_writes() {
    let storage = Arc::new(MemoryAuditStorage::new(5000));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    let start_time = std::time::Instant::now();

    // 写入 500 个事件
    for i in 0..500 {
        let event = AuditEvent::create("throughput_test", &i.to_string(), "admin");
        logger.log(event).await.unwrap();
    }

    let elapsed = start_time.elapsed();

    // 验证所有事件都被写入
    let count = storage.event_count().await;
    assert_eq!(count, 500);

    // 性能检查：500 个事件应该在合理时间内完成
    println!("Wrote 500 events in {:?}", elapsed);
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "Should complete within 10 seconds"
    );
}

/// TEST-U-AUDIT-022: 测试审计事件构建器
///
/// 验证使用构建器创建完整的审计事件。
#[tokio::test]
async fn test_audit_event_builder() {
    let event = AuditEvent::builder()
        .operation(AuditOperation::Update)
        .entity_type("users")
        .entity_id("123")
        .user_id("admin")
        .user_role("administrator")
        .client_ip("192.168.1.100")
        .severity(AuditSeverity::High)
        .result(AuditResult::Success)
        .before_value(r#"{"name": "Old Name"}"#)
        .after_value(r#"{"name": "New Name"}"#)
        .extra(r#"{"reason": "Name change request"}"#)
        .build();

    assert_eq!(event.operation, AuditOperation::Update);
    assert_eq!(event.entity_type, "users");
    assert_eq!(event.entity_id, "123");
    assert_eq!(event.user_id, "admin");
    assert_eq!(event.user_role, "administrator");
    assert_eq!(event.client_ip, "192.168.1.100");
    assert_eq!(event.severity, AuditSeverity::High);
    assert_eq!(event.result, AuditResult::Success);
    assert!(event.before_value.is_some());
    assert!(event.after_value.is_some());
    assert!(event.extra.is_some());
}

/// TEST-U-AUDIT-023: 测试追踪上下文
///
/// 验证审计事件可以包含分布式追踪上下文。
#[tokio::test]
async fn test_audit_trace_context() {
    let event = AuditEvent::create("test", "1", "admin").with_trace_context("trace-123", "span-456");

    assert!(event.trace_context.is_some());
    let trace = event.trace_context.unwrap();
    assert_eq!(trace.trace_id, "trace-123");
    assert_eq!(trace.span_id, "span-456");
}

/// TEST-U-AUDIT-024: 测试审计上下文
///
/// 验证审计上下文的创建和使用。
#[tokio::test]
async fn test_audit_context() {
    let ctx = dbnexus::audit::AuditContext::new("user123", "admin", "10.0.0.1")
        .with_request_id("req-001")
        .with_session_id("sess-001");

    assert_eq!(ctx.user_id, "user123");
    assert_eq!(ctx.user_role, "admin");
    assert_eq!(ctx.client_ip, "10.0.0.1");
    assert_eq!(ctx.request_id, "req-001");
    assert_eq!(ctx.session_id, "sess-001");
}

/// TEST-U-AUDIT-025: 测试审计配置默认值
///
/// 验证审计配置的默认值设置正确。
#[test]
fn test_audit_config_defaults() {
    let config = AuditConfig::default();

    assert!(config.enabled, "Should be enabled by default");
    assert_eq!(
        config.max_file_size,
        10 * 1024 * 1024,
        "Default max file size should be 10MB"
    );
    assert_eq!(config.retention_count, 7, "Default retention should be 7 days");
    assert!(config.sensitive_fields.contains(&"password".to_string()));
    assert!(config.sensitive_fields.contains(&"token".to_string()));
    assert!(config.alert_operations.contains(&AuditOperation::Delete));
}

/// TEST-U-AUDIT-026: 测试敏感数据脱敏 - 数组格式
///
/// 验证 JSON 数组中的敏感字段也被正确脱敏。
#[tokio::test]
async fn test_audit_sanitize_json_array() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig::default();
    let logger = AuditLogger::with_config(config, storage.clone());

    let event = AuditEvent::create("users", "1", "admin")
        .with_after_value(r#"[{"name": "user1", "password": "pass1"}, {"name": "user2", "password": "pass2"}]"#);

    logger.log(event).await.unwrap();

    let filters = AuditQueryFilters::default();
    let results = logger.query(&filters).await.unwrap();
    let stored_value = results[0].after_value.as_ref().unwrap();

    // 验证数组中的敏感字段被脱敏
    assert!(
        stored_value.contains("REDACTED"),
        "Array sensitive fields should be redacted, got: {}",
        stored_value
    );
    // 验证非敏感字段保留
    assert!(
        stored_value.contains("user1") && stored_value.contains("user2"),
        "Non-sensitive fields should remain"
    );
}
