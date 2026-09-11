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
    assert_ne!(SagaStatus::Failed, SagaStatus::CompensationFailed);
    assert_ne!(SagaStatus::CompensationFailed, SagaStatus::Failed);
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

// ============================================================================
// T402：SagaLogStore 内存实现 + 补偿重放测试
// ============================================================================

#[test]
fn test_memory_store_persist_and_load_pending() {
    let store = InMemorySagaLog::new();
    use dbnexus::SagaLogStore;
    futures_now::block_on(async {
        let log = SagaLog {
            saga_id: "s-1".to_string(),
            status: SagaStatus::Running,
            steps: vec![SagaStepLog {
                name: "step1".to_string(),
                shard_id: 0,
                action_success: true,
                compensation_success: None,
                error: None,
            }],
        };
        store.persist(&log).await.unwrap();
        assert!(dbnexus::SagaLogStore::get(&store, "s-1").await.unwrap().is_some());
        let pending = dbnexus::SagaLogStore::load_pending(&store).await.unwrap();
        assert_eq!(pending.len(), 1);

        let mut done = log.clone();
        done.status = SagaStatus::Completed;
        store.persist(&done).await.unwrap();
        assert!(dbnexus::SagaLogStore::load_pending(&store).await.unwrap().is_empty());
    });
}

/// futures-now 风格的阻塞辅助（复用 tokio runtime 单线程执行）
mod futures_now {
    pub fn block_on<F: std::future::Future>(fut: F) -> <F as std::future::Future>::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(fut)
    }
}

// ============================================================================
// T402：DbSagaLog 持久化 + 启动恢复 + 补偿重放
// ============================================================================

#[cfg(feature = "sql-parser")]
mod db_saga_recovery_tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use dbnexus::{
        DbPool, DbSagaLog, SagaAction, SagaError, SagaExecutionResult, SagaLog, SagaLogStore,
        SagaOrchestrator, SagaRecovery, SagaStatus, SagaStep, SagaStepLog, ShardRouter,
    };

    struct OkAction;
    struct FailCompAction;

    #[async_trait]
    impl dbnexus::SagaAction for OkAction {
        async fn execute(&self, _session: &dbnexus::Session) -> Result<(), SagaError> {
            Ok(())
        }
        fn name(&self) -> &str {
            "ok"
        }
    }

    #[async_trait]
    impl dbnexus::SagaAction for FailCompAction {
        async fn execute(&self, _session: &dbnexus::Session) -> Result<(), SagaError> {
            Err(SagaError::CompensationFailed("comp boom".to_string()))
        }
        fn name(&self) -> &str {
            "fail-comp"
        }
    }

    fn temp_db_url(tag: &str) -> (String, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("dbnexus_saga_{}_{}.db", tag, std::process::id()));
        (format!("sqlite:{}?mode=rwc", path.display()), path)
    }

    #[tokio::test]
    async fn test_db_saga_log_persist_and_recover_and_replay() {
        let (url, path) = temp_db_url("persist");
        let pool = Arc::new(dbnexus::DbPool::new(&url).await.unwrap());

        let mut router = ShardRouter::with_strategy("hash", 1);
        router.register_shard(0, "s0".to_string(), url.clone());
        router.set_pool(0, pool.clone()).unwrap();

        let db_store = Arc::new(DbSagaLog::new(pool.clone()));
        db_store.init().await.unwrap();
        let store: Arc<dyn SagaLogStore> = db_store;

        let orchestrator = SagaOrchestrator::new_with_log_store(Arc::new(router), store.clone());

        // saga：step1 正向成功、step2 正向失败 → Failed（日志持久化）
        let steps = vec![
            SagaStep {
                name: "step1".to_string(),
                shard_id: 0,
                action: Box::new(OkAction),
                compensation: Box::new(OkAction),
            },
            SagaStep {
                name: "step2".to_string(),
                shard_id: 0,
                action: Box::new(FailCompAction2),
                compensation: Box::new(OkAction),
            },
        ];
        struct FailCompAction2;
        #[async_trait]
        impl dbnexus::SagaAction for FailCompAction2 {
            async fn execute(&self, _session: &dbnexus::Session) -> Result<(), SagaError> {
                Err(SagaError::ExecutionFailed("forward boom".to_string()))
            }
            fn name(&self) -> &str {
                "fail-forward"
            }
        }
        let result = orchestrator.execute_saga(steps).await;
        assert_eq!(result.status, SagaStatus::Failed);

        // 持久化断言：可从存储读回
        let stored = store.get(&result.saga_id).await.unwrap().unwrap();


        assert_eq!(stored.status, SagaStatus::Failed);
        assert_eq!(stored.steps.len(), 2);
        assert!(stored.steps[0].action_success);

        // 模拟"重启"：用同库新建存储，注入一条 Running 中断日志
        let interrupted = SagaLog {
            saga_id: "rec-1".to_string(),
            status: SagaStatus::Running,
            steps: vec![SagaStepLog {
                name: "step1".to_string(),
                shard_id: 0,
                action_success: true,
                compensation_success: None,
                error: None,
            }],
        };
        store.persist(&interrupted).await.unwrap();
        let pending = SagaRecovery::new(store.clone()).list_pending().await.unwrap();
        assert_eq!(pending.len(), 1, "Running 中断日志应出现在恢复列表");
        assert_eq!(pending[0].saga_id, "rec-1");

        // 重放补偿（补偿成功）→ 终态 Failed
        let replay_steps = vec![SagaStep {
            name: "step1".to_string(),
            shard_id: 0,
            action: Box::new(OkAction),
            compensation: Box::new(OkAction),
        }];
        let replay: SagaExecutionResult = orchestrator
            .compensate_recovered("rec-1", &replay_steps)
            .await;
        assert_eq!(replay.status, SagaStatus::Failed);
        assert_eq!(replay.compensated_steps, vec!["step1"]);

        // 补偿失败重放 → CompensationFailed（可再次重放）
        let failing_steps = vec![SagaStep {
            name: "step1".to_string(),
            shard_id: 0,
            action: Box::new(OkAction),
            compensation: Box::new(FailCompAction),
        }];
        store.persist(&interrupted).await.unwrap();
        let replay2 = orchestrator
            .compensate_recovered("rec-1", &failing_steps)
            .await;
        assert_eq!(replay2.status, SagaStatus::CompensationFailed);

        // 重试补偿（成功补偿覆盖上次失败）
        store.persist(&interrupted).await.unwrap();
        let replay3 = orchestrator
            .compensate_recovered("rec-1", &replay_steps)
            .await;
        assert_eq!(replay3.status, SagaStatus::Failed);

        let _ = std::fs::remove_file(&path);
    }
}
