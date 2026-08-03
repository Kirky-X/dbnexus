// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 副本路由模块单元测试

use dbnexus::ReplicaConfig;

// ============================================================================
// ReplicaConfig 测试
// ============================================================================

#[test]
fn test_replica_config_creation() {
    let config = ReplicaConfig {
        replica_urls: vec![
            "postgres://replica1:5432/db".to_string(),
            "postgres://replica2:5432/db".to_string(),
        ],
        max_lag_seconds: 5.0,
        lag_check_interval_secs: 10,
    };
    assert_eq!(config.replica_urls.len(), 2);
    assert!((config.max_lag_seconds - 5.0).abs() < f64::EPSILON);
    assert_eq!(config.lag_check_interval_secs, 10);
}

#[test]
fn test_replica_config_serde_roundtrip() {
    let config = ReplicaConfig {
        replica_urls: vec!["postgres://replica:5432/db".to_string()],
        max_lag_seconds: 3.0,
        lag_check_interval_secs: 5,
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: ReplicaConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.replica_urls.len(), 1);
    assert!((deserialized.max_lag_seconds - 3.0).abs() < f64::EPSILON);
    assert_eq!(deserialized.lag_check_interval_secs, 5);
}

#[test]
fn test_replica_config_empty_urls() {
    let config = ReplicaConfig {
        replica_urls: vec![],
        max_lag_seconds: 10.0,
        lag_check_interval_secs: 30,
    };
    assert!(config.replica_urls.is_empty());
}
