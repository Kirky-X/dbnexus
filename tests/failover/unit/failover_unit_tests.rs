// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 故障转移模块单元测试

use dbnexus::FailoverConfig;

// ============================================================================
// FailoverConfig 测试
// ============================================================================

#[test]
fn test_failover_config_creation() {
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
    assert_eq!(config.failover_threshold, 3);
    assert_eq!(config.health_check_query.as_deref(), Some("SELECT 1"));
}

#[test]
fn test_failover_config_default_threshold() {
    // FailoverConfig 没有 Default impl，但我们可以验证字段类型正确
    let config = FailoverConfig {
        urls: vec!["postgres://localhost/db".to_string()],
        health_check_query: None,
        failover_threshold: 3,
    };
    assert_eq!(config.failover_threshold, 3);
}

#[test]
fn test_failover_config_serde_roundtrip() {
    let config = FailoverConfig {
        urls: vec!["postgres://host1/db".to_string(), "postgres://host2/db".to_string()],
        health_check_query: Some("SELECT 1".to_string()),
        failover_threshold: 5,
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: FailoverConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.urls.len(), 2);
    assert_eq!(deserialized.failover_threshold, 5);
    assert_eq!(deserialized.health_check_query.as_deref(), Some("SELECT 1"));
}

#[test]
fn test_failover_config_empty_urls() {
    let config = FailoverConfig {
        urls: vec![],
        health_check_query: None,
        failover_threshold: 3,
    };
    assert!(config.urls.is_empty());
}
