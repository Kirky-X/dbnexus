// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Saga 分布式事务单元测试

use dbnexus::{InMemorySagaLog, SagaError, SagaLog, SagaStatus, SagaStepLog};

// ============================================================================
// SagaStatus 测试
// ============================================================================

#[test]
fn test_saga_status_equality() {
    assert_eq!(SagaStatus::Running, SagaStatus::Running);
    assert_eq!(SagaStatus::Completed, SagaStatus::Completed);
    assert_eq!(SagaStatus::Compensating, SagaStatus::Compensating);
    assert_eq!(SagaStatus::Failed, SagaStatus::Failed);
    assert_ne!(SagaStatus::Running, SagaStatus::Completed);
    assert_ne!(SagaStatus::Failed, SagaStatus::Compensating);
}

#[test]
fn test_saga_status_clone_copy() {
    let status = SagaStatus::Running;
    let cloned = status;
    assert_eq!(status, cloned);
}

// ============================================================================
// SagaError 测试
// ============================================================================

#[test]
fn test_saga_error_execution_failed_display() {
    let err = SagaError::ExecutionFailed("connection refused".to_string());
    let msg = format!("{err}");
    assert!(msg.contains("execution failed"));
    assert!(msg.contains("connection refused"));
}

#[test]
fn test_saga_error_compensation_failed_display() {
    let err = SagaError::CompensationFailed("rollback failed".to_string());
    let msg = format!("{err}");
    assert!(msg.contains("compensation failed"));
    assert!(msg.contains("rollback failed"));
}

#[test]
fn test_saga_error_timeout_display() {
    let err = SagaError::Timeout("30s exceeded".to_string());
    let msg = format!("{err}");
    assert!(msg.contains("timeout"));
    assert!(msg.contains("30s exceeded"));
}

#[test]
fn test_saga_error_is_error_trait() {
    let err: Box<dyn std::error::Error> = Box::new(SagaError::ExecutionFailed("test".to_string()));
    assert!(err.source().is_none());
}

// ============================================================================
// SagaStepLog 测试
// ============================================================================

#[test]
fn test_saga_step_log_success() {
    let log = SagaStepLog {
        name: "create_order".to_string(),
        shard_id: 0,
        action_success: true,
        compensation_success: None,
        error: None,
    };
    assert!(log.action_success);
    assert!(log.compensation_success.is_none());
    assert!(log.error.is_none());
}

#[test]
fn test_saga_step_log_with_compensation() {
    let log = SagaStepLog {
        name: "deduct_balance".to_string(),
        shard_id: 1,
        action_success: true,
        compensation_success: Some(true),
        error: None,
    };
    assert_eq!(log.compensation_success, Some(true));
}

#[test]
fn test_saga_step_log_failed() {
    let log = SagaStepLog {
        name: "ship_order".to_string(),
        shard_id: 2,
        action_success: false,
        compensation_success: None,
        error: Some("inventory unavailable".to_string()),
    };
    assert!(!log.action_success);
    assert!(log.error.is_some());
}

// ============================================================================
// SagaLog 测试
// ============================================================================

#[test]
fn test_saga_log_creation() {
    let log = SagaLog {
        saga_id: "test-saga-123".to_string(),
        status: SagaStatus::Running,
        steps: vec![],
    };
    assert_eq!(log.saga_id, "test-saga-123");
    assert_eq!(log.status, SagaStatus::Running);
    assert!(log.steps.is_empty());
}

#[test]
fn test_saga_log_clone() {
    let log = SagaLog {
        saga_id: "abc".to_string(),
        status: SagaStatus::Completed,
        steps: vec![SagaStepLog {
            name: "step1".to_string(),
            shard_id: 0,
            action_success: true,
            compensation_success: None,
            error: None,
        }],
    };
    let cloned = log.clone();
    assert_eq!(cloned.saga_id, "abc");
    assert_eq!(cloned.steps.len(), 1);
}

// ============================================================================
// InMemorySagaLog 测试
// ============================================================================

#[test]
fn test_in_memory_saga_log_insert_and_get() {
    let store = InMemorySagaLog::new();
    let log = SagaLog {
        saga_id: "saga-1".to_string(),
        status: SagaStatus::Running,
        steps: vec![],
    };
    store.insert(log);

    let retrieved = store.get("saga-1");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().saga_id, "saga-1");
}

#[test]
fn test_in_memory_saga_log_get_nonexistent() {
    let store = InMemorySagaLog::new();
    assert!(store.get("nonexistent").is_none());
}

#[test]
fn test_in_memory_saga_log_update_status() {
    let store = InMemorySagaLog::new();
    let log = SagaLog {
        saga_id: "saga-2".to_string(),
        status: SagaStatus::Running,
        steps: vec![],
    };
    store.insert(log);

    store.update_status("saga-2", SagaStatus::Completed);
    let retrieved = store.get("saga-2").unwrap();
    assert_eq!(retrieved.status, SagaStatus::Completed);
}

#[test]
fn test_in_memory_saga_log_multiple_sagas() {
    let store = InMemorySagaLog::new();

    for i in 0..10 {
        store.insert(SagaLog {
            saga_id: format!("saga-{i}"),
            status: SagaStatus::Running,
            steps: vec![],
        });
    }

    for i in 0..10 {
        let log = store.get(&format!("saga-{i}"));
        assert!(log.is_some());
    }
}

#[test]
fn test_in_memory_saga_log_default() {
    let store = InMemorySagaLog::default();
    assert!(store.get("any").is_none());
}
