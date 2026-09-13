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

// ============================================================================
// 副本负载均衡（读写分离 + 权重/延迟选择 + 故障剔除）
// ============================================================================

#[cfg(all(
    feature = "replica-routing",
    feature = "sqlite",
    feature = "runtime-tokio-rustls"
))]
mod t411_replica_load_balancer_tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use dbnexus::database::replica::{ReplicationLag, ReplicationLagDetector};
    use dbnexus::{DbPool, ReplicaConfig, ReplicaLoadBalancer, ReplicaNode};

    fn temp_db_url(tag: &str) -> String {
        let path =
            std::env::temp_dir().join(format!("dbnexus_t411_{}_{}.db", tag, std::process::id()));
        format!("sqlite:{}?mode=rwc", path.display())
    }

    /// mock 探测器：可控的 caught_up 结果
    struct MockDetector {
        caught_up: bool,
        delay_ms: u64,
    }

    impl MockDetector {
        fn caught_up() -> Arc<Self> {
            Arc::new(Self {
                caught_up: true,
                delay_ms: 0,
            })
        }
        fn lagging() -> Arc<Self> {
            Arc::new(Self {
                caught_up: false,
                delay_ms: 0,
            })
        }
        fn slow(delay_ms: u64) -> Arc<Self> {
            Arc::new(Self {
                caught_up: true,
                delay_ms,
            })
        }
    }

    #[async_trait]
    impl ReplicationLagDetector for MockDetector {
        async fn detect_lag(&self, _pool: &DbPool) -> dbnexus::DbResult<ReplicationLag> {
            if self.delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
            }
            Ok(ReplicationLag {
                lag_bytes: None,
                lag_seconds: if self.caught_up {
                    Some(0.0)
                } else {
                    Some(999.0)
                },
                is_caught_up: self.caught_up,
            })
        }
    }

    /// mock 探测器：探测即失败（模拟副本失联）
    struct FailingDetector;

    #[async_trait]
    impl ReplicationLagDetector for FailingDetector {
        async fn detect_lag(&self, _pool: &DbPool) -> dbnexus::DbResult<ReplicationLag> {
            Err(dbnexus::DbError::Query("replica probe failed".to_string()))
        }
    }

    async fn replica_node(
        tag: &str,
        weight: u32,
        detector: Arc<dyn ReplicationLagDetector>,
    ) -> ReplicaNode {
        ReplicaNode {
            name: tag.to_string(),
            pool: Arc::new(DbPool::new(&temp_db_url(tag)).await.unwrap()),
            weight,
            lag_detector: detector,
        }
    }

    #[tokio::test]
    async fn test_t411_read_write_split_routing() {
        let primary = Arc::new(DbPool::new(&temp_db_url("primary")).await.unwrap());
        let node = replica_node("replica-a", 1, MockDetector::caught_up()).await;
        let balancer =
            ReplicaLoadBalancer::new(primary.clone(), vec![node], ReplicaConfig::default());

        // 写路由：始终主库（last_selected 不变）
        let _w = balancer.get_write_session("admin").await.unwrap();
        assert_eq!(balancer.last_selected_replica(), None);

        // 读路由：命中副本
        let _r = balancer.get_read_session("admin").await.unwrap();
        assert_eq!(
            balancer.last_selected_replica(),
            Some("replica-a".to_string()),
            "读会话应路由到健康副本"
        );
    }

    #[tokio::test]
    async fn test_t411_weight_selection_prefers_heavier_replica() {
        let primary = Arc::new(DbPool::new(&temp_db_url("primary")).await.unwrap());
        let heavy = replica_node("replica-heavy", 10, MockDetector::caught_up()).await;
        let light = replica_node("replica-light", 1, MockDetector::caught_up()).await;
        let balancer = ReplicaLoadBalancer::new(
            primary.clone(),
            vec![heavy, light],
            ReplicaConfig::default(),
        );

        for _ in 0..3 {
            balancer.get_read_session("admin").await.unwrap();
            assert_eq!(
                balancer.last_selected_replica(),
                Some("replica-heavy".to_string()),
                "高权重副本应确定性胜出"
            );
        }
    }

    #[tokio::test]
    async fn test_t411_latency_selection_prefers_fast_probe() {
        let primary = Arc::new(DbPool::new(&temp_db_url("primary")).await.unwrap());
        let slow = replica_node("replica-slow", 1, MockDetector::slow(60)).await;
        let fast = replica_node("replica-fast", 1, MockDetector::caught_up()).await;
        let balancer =
            ReplicaLoadBalancer::new(primary.clone(), vec![slow, fast], ReplicaConfig::default());

        balancer.get_read_session("admin").await.unwrap();
        assert_eq!(
            balancer.last_selected_replica(),
            Some("replica-fast".to_string()),
            "等权重下低延迟副本应胜出"
        );
    }

    #[tokio::test]
    async fn test_t411_lagging_replica_bypassed() {
        let primary = Arc::new(DbPool::new(&temp_db_url("primary")).await.unwrap());
        let lagging = replica_node("replica-lagging", 10, MockDetector::lagging()).await;
        let fresh = replica_node("replica-fresh", 1, MockDetector::caught_up()).await;
        let balancer = ReplicaLoadBalancer::new(
            primary.clone(),
            vec![lagging, fresh],
            ReplicaConfig::default(),
        );

        balancer.get_read_session("admin").await.unwrap();
        assert_eq!(
            balancer.last_selected_replica(),
            Some("replica-fresh".to_string()),
            "lag 超阈值的副本不应承接读流量"
        );
    }

    #[tokio::test]
    async fn test_t411_failure_eviction_and_recovery() {
        let primary = Arc::new(DbPool::new(&temp_db_url("primary")).await.unwrap());
        let node = replica_node("replica-bad", 1, Arc::new(FailingDetector)).await;
        let config = ReplicaConfig {
            replica_urls: vec!["replica-bad".to_string()],
            ..Default::default()
        };
        let balancer = ReplicaLoadBalancer::new(primary.clone(), vec![node], config);

        // 连续失败达阈值（默认 3）→ 剔除
        for i in 0..3 {
            let session = balancer.get_read_session("admin").await;
            assert!(session.is_ok(), "第 {} 次读应回退主库成功", i);
        }
        assert!(
            balancer.is_replica_evicted("replica-bad"),
            "连续失败达阈值应被剔除"
        );

        // 剔除后探测被跳过（仍可回退主库）
        let _ = balancer.get_read_session("admin").await.unwrap();

        // 恢复：手动复活（半开重探入口）
        balancer.revive_all();
        assert!(
            !balancer.is_replica_evicted("replica-bad"),
            "revive 应清除剔除状态"
        );
    }

    #[tokio::test]
    #[cfg(feature = "health-check")]
    async fn test_t411_snapshot_matches_health_provider_shape() {
        let primary = Arc::new(DbPool::new(&temp_db_url("primary")).await.unwrap());
        let node = replica_node("replica-snap", 1, MockDetector::caught_up()).await;
        let balancer = Arc::new(ReplicaLoadBalancer::new(
            primary.clone(),
            vec![node],
            ReplicaConfig::default(),
        ));

        // 快照：条目形态与健康语义
        let snapshot = balancer.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0]["name"], "replica-snap");
        assert_eq!(snapshot[0]["healthy"], true);

        // 与健康导出对接：快照经 ReplicaHealthProvider 进入 health_snapshot
        let provider_balancer = balancer.clone();
        primary
            .set_replica_health_provider(Some(Arc::new(move || provider_balancer.snapshot())))
            .await;
        let health = primary.health_snapshot().await;
        let replicas = health["replicas"].as_array().unwrap();
        assert_eq!(replicas.len(), 1);
        assert_eq!(replicas[0]["name"], "replica-snap");
    }
}
