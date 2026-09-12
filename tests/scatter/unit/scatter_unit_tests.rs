// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Scatter-Gather 跨分片查询引擎单元测试

use dbnexus::{AggregateFunction, AggregateValue, PartialFailurePolicy, ScatterResult};

// ============================================================================
// PartialFailurePolicy 测试
// ============================================================================

#[test]
fn test_partial_failure_policy_equality() {
    assert_eq!(PartialFailurePolicy::Fail, PartialFailurePolicy::Fail);
    assert_eq!(
        PartialFailurePolicy::BestEffort,
        PartialFailurePolicy::BestEffort
    );
    assert_ne!(PartialFailurePolicy::Fail, PartialFailurePolicy::BestEffort);
}

#[test]
fn test_partial_failure_policy_clone() {
    let policy = PartialFailurePolicy::BestEffort;
    let cloned = policy;
    assert_eq!(policy, cloned);
}

// ============================================================================
// AggregateFunction 测试
// ============================================================================

#[test]
fn test_aggregate_function_count() {
    let func = AggregateFunction::Count;
    assert!(matches!(func, AggregateFunction::Count));
}

#[test]
fn test_aggregate_function_sum() {
    let func = AggregateFunction::Sum("amount".to_string());
    match func {
        AggregateFunction::Sum(col) => assert_eq!(col, "amount"),
        _ => panic!("Expected Sum variant"),
    }
}

#[test]
fn test_aggregate_function_avg() {
    let func = AggregateFunction::Avg("price".to_string());
    match func {
        AggregateFunction::Avg(col) => assert_eq!(col, "price"),
        _ => panic!("Expected Avg variant"),
    }
}

#[test]
fn test_aggregate_function_min_max() {
    let min_func = AggregateFunction::Min("score".to_string());
    let max_func = AggregateFunction::Max("score".to_string());
    match (&min_func, &max_func) {
        (AggregateFunction::Min(a), AggregateFunction::Max(b)) => {
            assert_eq!(a, "score");
            assert_eq!(b, "score");
        }
        _ => panic!("Expected Min and Max variants"),
    }
}

// ============================================================================
// AggregateValue 测试
// ============================================================================

#[test]
fn test_aggregate_value_count() {
    let val = AggregateValue::Count(42);
    match val {
        AggregateValue::Count(n) => assert_eq!(n, 42),
        _ => panic!("Expected Count variant"),
    }
}

#[test]
fn test_aggregate_value_sum_avg() {
    let sum = AggregateValue::Sum(100.5);
    let avg = AggregateValue::Avg(50.25);
    match (sum, avg) {
        (AggregateValue::Sum(s), AggregateValue::Avg(a)) => {
            assert!((s - 100.5).abs() < f64::EPSILON);
            assert!((a - 50.25).abs() < f64::EPSILON);
        }
        _ => panic!("Expected Sum and Avg variants"),
    }
}

#[test]
fn test_aggregate_value_min_max() {
    let min = AggregateValue::Min(1.0);
    let max = AggregateValue::Max(999.0);
    match (min, max) {
        (AggregateValue::Min(mn), AggregateValue::Max(mx)) => {
            assert!((mn - 1.0).abs() < f64::EPSILON);
            assert!((mx - 999.0).abs() < f64::EPSILON);
        }
        _ => panic!("Expected Min and Max variants"),
    }
}

// ============================================================================
// ScatterResult 测试
// ============================================================================

#[test]
fn test_scatter_result_empty() {
    let result = ScatterResult {
        shard_row_counts: vec![],
        failed_shards: vec![],
        aggregated: None,
        shard_rows: vec![],
    };
    assert!(result.shard_row_counts.is_empty());
    assert!(result.failed_shards.is_empty());
    assert!(result.aggregated.is_none());
}

#[test]
fn test_scatter_result_with_data() {
    use dbnexus::ShardError;
    let result = ScatterResult {
        shard_row_counts: vec![(0, 100), (1, 200), (2, 150)],
        failed_shards: vec![ShardError {
            shard_id: 3,
            error: "connection refused".to_string(),
        }],
        aggregated: Some(AggregateValue::Count(450)),
        shard_rows: vec![],
    };
    assert_eq!(result.shard_row_counts.len(), 3);
    assert_eq!(result.failed_shards.len(), 1);
    assert!(result.aggregated.is_some());
}

// ============================================================================
// ShardError 测试
// ============================================================================

#[test]
fn test_shard_error_debug() {
    use dbnexus::ShardError;
    let err = ShardError {
        shard_id: 5,
        error: "timeout".to_string(),
    };
    let debug_str = format!("{err:?}");
    assert!(debug_str.contains("5"));
    assert!(debug_str.contains("timeout"));
}

#[test]
fn test_shard_error_clone() {
    use dbnexus::ShardError;
    let err = ShardError {
        shard_id: 1,
        error: "error".to_string(),
    };
    let cloned = err.clone();
    assert_eq!(cloned.shard_id, 1);
    assert_eq!(cloned.error, "error");
}

// ============================================================================
// AggregateFunction::compute_from_rows 行级聚合测试
// ============================================================================

#[test]
fn test_compute_from_rows_count() {
    use dbnexus::AggregateFunction;
    let rows = vec![
        (0u32, vec![serde_json::json!({"id": 1}), serde_json::json!({"id": 2})]),
        (1u32, vec![serde_json::json!({"id": 3})]),
    ];
    let agg = AggregateFunction::Count.compute_from_rows(&rows);
    match agg {
        Some(AggregateValue::Count(n)) => assert_eq!(n, 3),
        other => panic!("expected Count(3), got {:?}", other),
    }
}

#[test]
fn test_compute_from_rows_sum_avg_min_max() {
    use dbnexus::AggregateFunction;
    let rows = vec![
        (0u32, vec![serde_json::json!({"val": 10.0}), serde_json::json!({"val": 20.0})]),
        (1u32, vec![serde_json::json!({"val": 30.0})]),
    ];
    match AggregateFunction::Sum("val".to_string()).compute_from_rows(&rows) {
        Some(AggregateValue::Sum(v)) => assert!((v - 60.0).abs() < 1e-9),
        other => panic!("expected Sum(60), got {:?}", other),
    }
    match AggregateFunction::Avg("val".to_string()).compute_from_rows(&rows) {
        Some(AggregateValue::Avg(v)) => assert!((v - 20.0).abs() < 1e-9),
        other => panic!("expected Avg(20), got {:?}", other),
    }
    match AggregateFunction::Min("val".to_string()).compute_from_rows(&rows) {
        Some(AggregateValue::Min(v)) => assert!((v - 10.0).abs() < 1e-9),
        other => panic!("expected Min(10), got {:?}", other),
    }
    match AggregateFunction::Max("val".to_string()).compute_from_rows(&rows) {
        Some(AggregateValue::Max(v)) => assert!((v - 30.0).abs() < 1e-9),
        other => panic!("expected Max(30), got {:?}", other),
    }
}

#[test]
fn test_compute_from_rows_empty_and_missing_column() {
    use dbnexus::AggregateFunction;
    // 无行：Sum 返回 0 中性值，Avg/Min/Max 返回 None
    let empty: Vec<(u32, Vec<serde_json::Value>)> = vec![];
    assert!(matches!(
        AggregateFunction::Sum("v".to_string()).compute_from_rows(&empty),
        Some(AggregateValue::Sum(v)) if v == 0.0
    ));
    assert!(AggregateFunction::Avg("v".to_string()).compute_from_rows(&empty).is_none());
    assert!(AggregateFunction::Min("v".to_string()).compute_from_rows(&empty).is_none());
    // 行存在但列缺失：AVG None
    let rows = vec![(0u32, vec![serde_json::json!({"other": 1.0})])];
    assert!(AggregateFunction::Avg("v".to_string()).compute_from_rows(&rows).is_none());
}
