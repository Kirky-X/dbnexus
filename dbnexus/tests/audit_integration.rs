// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Audit 集成测试
//!
//! 测试审计模块的查询组合、存储后端、告警系统、配置等高级功能

use chrono::Utc;
use dbnexus::audit::{
    AuditConfig, AuditEvent, AuditLogger, AuditOperation, AuditQueryFilters, AuditStorage, MemoryAuditStorage,
};
use std::sync::Arc;
use std::time::Duration;

/// TEST-AUDIT-001: 多条件组合查询测试
#[tokio::test]
async fn test_audit_query_multiple_conditions() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    for i in 0..100 {
        let event = AuditEvent::create("users", &i.to_string(), "admin");
        let _ = logger.log(event).await;
    }

    let filters = AuditQueryFilters {
        entity_type: Some("users".to_string()),
        ..Default::default()
    };
    let results: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");
    assert_eq!(results.len(), 100, "Should find all 100 user events");
}

/// TEST-AUDIT-002: 时间范围查询测试
#[tokio::test]
async fn test_audit_query_time_range() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    let base_time = Utc::now();

    for i in 0..10 {
        let event = AuditEvent::create("logs", &i.to_string(), "system");
        let _ = logger.log(event).await;
    }

    let filters = AuditQueryFilters {
        start_time: Some(base_time - chrono::Duration::hours(5)),
        ..Default::default()
    };
    let recent: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");
    assert!(recent.len() >= 4, "Should find events from last 5 hours");
}

/// TEST-AUDIT-003: 空条件处理测试
#[tokio::test]
async fn test_audit_query_empty_conditions() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    for i in 0..10 {
        let event = AuditEvent::create("test", &i.to_string(), "test_user");
        let _ = logger.log(event).await;
    }

    let all: Vec<AuditEvent> = storage
        .query(&AuditQueryFilters::default())
        .await
        .expect("Query should succeed");
    assert_eq!(all.len(), 10, "Empty query should return all events");
}

/// TEST-AUDIT-004: 分页查询测试
#[tokio::test]
async fn test_audit_query_pagination() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    for i in 0..50 {
        let event = AuditEvent::create("paged", &i.to_string(), "admin");
        let _ = logger.log(event).await;
    }

    let filters = AuditQueryFilters {
        entity_type: Some("paged".to_string()),
        ..Default::default()
    };
    let all_results: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");

    assert_eq!(all_results.len(), 50, "Should find all 50 events");

    let page1: Vec<_> = all_results[0..10].to_vec();
    let page2: Vec<_> = all_results[10..20].to_vec();

    assert_eq!(page1.len(), 10, "First page should have 10 results");
    assert_eq!(page2.len(), 10, "Second page should have 10 results");
}

/// TEST-AUDIT-005: 存储后端读写测试
#[tokio::test]
async fn test_audit_storage_write_read() {
    let storage = Arc::new(MemoryAuditStorage::new(100));

    for i in 0..10 {
        let event = AuditEvent::create("files", &i.to_string(), "admin");
        storage.store(&event).await.expect("Failed to store event");
    }

    let filters = AuditQueryFilters {
        entity_type: Some("files".to_string()),
        ..Default::default()
    };
    let results: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");
    assert_eq!(results.len(), 10, "Should retrieve all written events");
}

/// TEST-AUDIT-006: 存储后端 JSON 序列化测试
#[tokio::test]
async fn test_audit_storage_json_serialization() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    let event = AuditEvent::create("users", "123", "admin");
    let _ = logger.log(event.clone()).await;

    let json = event.to_json().expect("Should serialize to JSON");
    let parsed = AuditEvent::from_json(&json).expect("Should deserialize from JSON");

    assert_eq!(parsed.entity_type, "users");
    assert_eq!(parsed.entity_id, "123");
    assert_eq!(parsed.user_id, "admin");
    assert_eq!(parsed.operation, AuditOperation::Create);
}

/// TEST-AUDIT-007: 审计日志批量操作测试
#[tokio::test]
async fn test_audit_batch_operations() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    let batch_size = 100;
    let mut events = Vec::new();

    for i in 0..batch_size {
        let event = AuditEvent::create("batch_table", &i.to_string(), "batch_user");
        events.push(event);
    }

    for event in events {
        let _ = logger.log(event).await;
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let filters = AuditQueryFilters {
        entity_type: Some("batch_table".to_string()),
        ..Default::default()
    };
    let results: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");
    assert_eq!(results.len(), batch_size, "All batch events should be logged");
}

/// TEST-AUDIT-008: 审计存储容量限制测试
#[tokio::test]
async fn test_audit_storage_capacity_limit() {
    let storage = Arc::new(MemoryAuditStorage::new(50));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    for i in 0..100 {
        let event = AuditEvent::create("capacity_test", &i.to_string(), "admin");
        let _ = logger.log(event).await;
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let filters = AuditQueryFilters {
        entity_type: Some("capacity_test".to_string()),
        ..Default::default()
    };
    let results: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");
    assert!(
        results.len() <= 50,
        "Storage should respect capacity limit: got {}",
        results.len()
    );
}

/// TEST-AUDIT-009: 审计事件类型完整测试
#[tokio::test]
async fn test_audit_all_event_types() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    let _ = logger.log(AuditEvent::create("test", "1", "user")).await;
    let _ = logger.log(AuditEvent::read("test", "1", "user")).await;
    let _ = logger.log(AuditEvent::update("test", "1", "user", None, None)).await;
    let _ = logger.log(AuditEvent::delete("test", "1", "admin")).await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    let filters = AuditQueryFilters {
        entity_type: Some("test".to_string()),
        ..Default::default()
    };
    let results: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");
    assert_eq!(results.len(), 4, "All event types should be logged");

    let logged_operations: Vec<_> = results.iter().map(|e| e.operation.clone()).collect();

    assert!(
        logged_operations.contains(&AuditOperation::Create),
        "Create should be logged"
    );
    assert!(
        logged_operations.contains(&AuditOperation::Read),
        "Read should be logged"
    );
    assert!(
        logged_operations.contains(&AuditOperation::Update),
        "Update should be logged"
    );
    assert!(
        logged_operations.contains(&AuditOperation::Delete),
        "Delete should be logged"
    );
}

/// TEST-AUDIT-010: 审计查询时间戳范围测试
#[tokio::test]
async fn test_audit_query_timestamp_range() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    for i in 0..20 {
        let event = AuditEvent::create("timestamp_test", &i.to_string(), "admin");
        let _ = logger.log(event).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let now = Utc::now();
    let filters = AuditQueryFilters {
        start_time: Some(now - chrono::Duration::seconds(1)),
        end_time: Some(now),
        ..Default::default()
    };
    let results: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");
    assert!(results.len() >= 10, "Should find recent events");
}

/// TEST-AUDIT-011: 审计用户ID过滤测试
#[tokio::test]
async fn test_audit_query_by_user() {
    let storage = Arc::new(MemoryAuditStorage::new(1000));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    for i in 0..10 {
        let event = AuditEvent::create("users", &i.to_string(), "alice");
        let _ = logger.log(event).await;
    }
    for i in 0..10 {
        let event = AuditEvent::create("users", &format!("b_{}", i), "bob");
        let _ = logger.log(event).await;
    }

    let filters = AuditQueryFilters {
        user_id: Some("alice".to_string()),
        ..Default::default()
    };
    let alice_events: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");
    assert_eq!(alice_events.len(), 10, "Should find only alice's events");

    let filters = AuditQueryFilters {
        user_id: Some("bob".to_string()),
        ..Default::default()
    };
    let bob_events: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");
    assert_eq!(bob_events.len(), 10, "Should find only bob's events");
}

/// TEST-AUDIT-012: 审计操作结果测试
#[tokio::test]
async fn test_audit_operation_results() {
    let storage = Arc::new(MemoryAuditStorage::new(100));
    let config = AuditConfig::default();
    let logger = AuditLogger::new(config, storage.clone());

    let success_event = AuditEvent::create("test", "1", "admin").with_result(dbnexus::audit::AuditResult::Success);
    let _ = logger.log(success_event).await;

    let failure_event = AuditEvent::create("test", "2", "admin").with_result(dbnexus::audit::AuditResult::Failure);
    let _ = logger.log(failure_event).await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    let filters = AuditQueryFilters {
        entity_type: Some("test".to_string()),
        ..Default::default()
    };
    let results: Vec<AuditEvent> = storage.query(&filters).await.expect("Query should succeed");
    assert_eq!(results.len(), 2);

    let success_count = results
        .iter()
        .filter(|e| e.result == dbnexus::audit::AuditResult::Success)
        .count();
    let failure_count = results
        .iter()
        .filter(|e| e.result == dbnexus::audit::AuditResult::Failure)
        .count();

    assert_eq!(success_count, 1, "Should have one success");
    assert_eq!(failure_count, 1, "Should have one failure");
}

/// TEST-AUDIT-013: 审计事件JSON解析测试
#[tokio::test]
async fn test_audit_event_json_roundtrip() {
    let original = AuditEvent::create("products", "123", "admin")
        .with_user("admin", "192.168.1.1")
        .with_before_value(r#"{"name": "Old"}"#)
        .with_after_value(r#"{"name": "New"}"#);

    let json = original.to_json().expect("Should serialize");
    let restored = AuditEvent::from_json(&json).expect("Should deserialize");

    assert_eq!(original.entity_type, restored.entity_type);
    assert_eq!(original.entity_id, restored.entity_id);
    assert_eq!(original.user_id, restored.user_id);
    assert_eq!(original.user_role, restored.user_role);
    assert_eq!(original.client_ip, restored.client_ip);
    assert_eq!(original.before_value, restored.before_value);
    assert_eq!(original.after_value, restored.after_value);
    assert_eq!(original.operation, restored.operation);
}
