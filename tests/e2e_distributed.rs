// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分布式能力端到端集成测试
//!
//! 验证所有分布式能力 feature 编译协同 + 类型可访问性

// ============================================================================
// 重试 + 故障转移协同验证
// ============================================================================

#[cfg(feature = "retry")]
mod retry_integration {
    use dbnexus::{RetryPolicy, is_idempotent_operation};

    #[test]
    fn test_retry_policy_with_db_config() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);

        // 幂等操作判断
        assert!(is_idempotent_operation("SELECT * FROM users"));
        assert!(!is_idempotent_operation("INSERT INTO users VALUES (1)"));
    }
}

#[cfg(feature = "failover")]
mod failover_integration {
    use dbnexus::FailoverConfig;

    #[test]
    fn test_failover_config_with_multiple_urls() {
        let config = FailoverConfig {
            urls: vec![
                "postgres://primary:5432/db".to_string(),
                "postgres://replica1:5432/db".to_string(),
                "postgres://replica2:5432/db".to_string(),
            ],
            health_check_query: Some("SELECT 1".to_string()),
            failover_threshold: 3,
        };
        assert_eq!(config.urls.len(), 3);
    }
}

// ============================================================================
// 副本路由验证
// ============================================================================

#[cfg(feature = "replica-routing")]
mod replica_integration {
    use dbnexus::ReplicaConfig;

    #[test]
    fn test_replica_config_with_lag_threshold() {
        let config = ReplicaConfig {
            replica_urls: vec!["postgres://replica:5432/db".to_string()],
            max_lag_seconds: 5.0,
            lag_check_interval_secs: 10,
        };
        assert!((config.max_lag_seconds - 5.0).abs() < f64::EPSILON);
    }
}

// ============================================================================
// Scatter-Gather 验证
// ============================================================================

#[cfg(feature = "scatter-gather")]
mod scatter_integration {
    use dbnexus::{AggregateFunction, AggregateValue, PartialFailurePolicy, ScatterResult, ShardError};

    #[test]
    fn test_scatter_result_aggregation() {
        let result = ScatterResult {
            shard_row_counts: vec![(0, 100), (1, 200), (2, 150)],
            failed_shards: vec![ShardError {
                shard_id: 3,
                error: "timeout".to_string(),
            }],
            aggregated: Some(AggregateValue::Count(450)),
        };
        assert_eq!(result.shard_row_counts.len(), 3);
        assert_eq!(result.failed_shards.len(), 1);
    }

    #[test]
    fn test_partial_failure_policies() {
        assert_ne!(PartialFailurePolicy::Fail, PartialFailurePolicy::BestEffort);
    }

    #[test]
    fn test_aggregate_function_types() {
        let _count = AggregateFunction::Count;
        let _sum = AggregateFunction::Sum("amount".to_string());
        let _avg = AggregateFunction::Avg("price".to_string());
        let _min = AggregateFunction::Min("score".to_string());
        let _max = AggregateFunction::Max("score".to_string());
    }
}

// ============================================================================
// Saga 分布式事务验证
// ============================================================================

#[cfg(feature = "saga")]
mod saga_integration {
    use dbnexus::{InMemorySagaLog, SagaError, SagaLog, SagaStatus};

    #[test]
    fn test_saga_lifecycle() {
        let store = InMemorySagaLog::new();

        // 创建 saga 日志
        let log = SagaLog {
            saga_id: "e2e-test-saga".to_string(),
            status: SagaStatus::Running,
            steps: vec![],
        };
        store.insert(log);

        // 验证初始状态
        let retrieved = store.get("e2e-test-saga").unwrap();
        assert_eq!(retrieved.status, SagaStatus::Running);

        // 更新为完成
        store.update_status("e2e-test-saga", SagaStatus::Completed);
        let final_log = store.get("e2e-test-saga").unwrap();
        assert_eq!(final_log.status, SagaStatus::Completed);
    }

    #[test]
    fn test_saga_error_types() {
        let exec_err = SagaError::ExecutionFailed("test".to_string());
        let comp_err = SagaError::CompensationFailed("test".to_string());
        let timeout_err = SagaError::Timeout("test".to_string());

        assert!(format!("{exec_err}").contains("execution failed"));
        assert!(format!("{comp_err}").contains("compensation failed"));
        assert!(format!("{timeout_err}").contains("timeout"));
    }
}

// ============================================================================
// 分布式 ID 验证
// ============================================================================

#[cfg(feature = "distributed-id")]
mod distributed_id_integration {
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::thread;

    use dbnexus::{DistributedIdGenerator, SnowflakeIdGenerator};

    #[test]
    fn test_snowflake_id_e2e() {
        let id_gen = Arc::new(SnowflakeIdGenerator::new(42, 1_700_000_000_000).unwrap());

        // 多线程生成 ID
        let mut handles = Vec::new();
        for _ in 0..5 {
            let gen_clone = Arc::clone(&id_gen);
            handles.push(thread::spawn(move || {
                (0..200).map(|_| gen_clone.next_id()).collect::<Vec<_>>()
            }));
        }

        let mut all_ids = HashSet::new();
        for handle in handles {
            for id in handle.join().unwrap() {
                assert!(all_ids.insert(id), "Duplicate ID: {id}");
            }
        }
        assert_eq!(all_ids.len(), 1000);
    }

    #[test]
    fn test_id_parse_roundtrip() {
        let id_gen = SnowflakeIdGenerator::new(99, 1_700_000_000_000).unwrap();
        let id = id_gen.next_id();
        let components = id_gen.parse_id(id);
        assert_eq!(components.machine_id, 99);
        assert!(components.timestamp_ms > 0);
    }
}
